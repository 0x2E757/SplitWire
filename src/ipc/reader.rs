use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{Frame, WireError};
use crate::net::Ipv4Net;

/// Reads the fields of one frame body, the counterpart of `Writer`. Every length and count is
/// checked against the bytes left, so garbage fails with an error and never allocates past the
/// frame.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    const LEB128_MAX: usize = 10;

    /// The tag of a frame body and a reader positioned on its first field.
    pub fn open(body: &'a [u8]) -> Result<(u8, Self), WireError> {
        let (magic, rest) = body.split_first_chunk::<3>().ok_or(WireError::Truncated)?;
        if magic != Frame::MAGIC {
            return Err(WireError::BadMagic);
        }
        let (tag, _) = rest.split_first().ok_or(WireError::Truncated)?;
        Ok((
            *tag,
            Self {
                bytes: body,
                at: Frame::HEADER_LEN,
            },
        ))
    }

    /// Ends the message: no bytes may be left.
    pub fn finish(self) -> Result<(), WireError> {
        match self.remaining() {
            0 => Ok(()),
            count => Err(WireError::Trailing(count)),
        }
    }

    pub fn u64(&mut self) -> Result<u64, WireError> {
        let mut value = 0u64;
        for index in 0..Self::LEB128_MAX {
            let byte = self.byte()?;
            if index == Self::LEB128_MAX - 1 && byte > 1 {
                break;
            }
            value |= u64::from(byte & 0x7f) << (7 * index);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(WireError::Invalid("an integer longer than 64 bits"))
    }

    pub fn usize(&mut self) -> Result<usize, WireError> {
        usize::try_from(self.u64()?).map_err(|_| Self::out_of_range())
    }

    pub fn u16(&mut self) -> Result<u16, WireError> {
        u16::try_from(self.u64()?).map_err(|_| Self::out_of_range())
    }

    pub fn u8(&mut self) -> Result<u8, WireError> {
        u8::try_from(self.u64()?).map_err(|_| Self::out_of_range())
    }

    pub fn bool(&mut self) -> Result<bool, WireError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(WireError::Invalid("a flag other than 0 or 1")),
        }
    }

    /// A string borrowed from the frame, for a caller that keeps it somewhere special.
    pub fn str(&mut self) -> Result<&'a str, WireError> {
        let len = self.count()?;
        std::str::from_utf8(self.take(len)?).map_err(|_| WireError::Invalid("a string not UTF-8"))
    }

    pub fn string(&mut self) -> Result<String, WireError> {
        self.str().map(str::to_owned)
    }

    pub fn time(&mut self) -> Result<SystemTime, WireError> {
        UNIX_EPOCH
            .checked_add(Duration::from_millis(self.u64()?))
            .ok_or(WireError::Invalid("a time out of range"))
    }

    pub fn ipv4(&mut self) -> Result<Ipv4Addr, WireError> {
        let octets: [u8; 4] = self.take(4)?.try_into().expect("4 bytes were taken");
        Ok(Ipv4Addr::from(octets))
    }

    pub fn socket(&mut self) -> Result<SocketAddrV4, WireError> {
        Ok(SocketAddrV4::new(self.ipv4()?, self.u16()?))
    }

    pub fn net(&mut self) -> Result<Ipv4Net, WireError> {
        let addr = self.ipv4()?;
        let prefix = self.u8()?;
        Ipv4Net::new(addr, prefix).ok_or(WireError::Invalid("a network with a bad prefix"))
    }

    /// The variant at the index read, out of `all`.
    pub fn choice<T: Copy>(&mut self, all: &[T]) -> Result<T, WireError> {
        all.get(self.usize()?)
            .copied()
            .ok_or(WireError::Invalid("an unknown code"))
    }

    pub fn option<T>(
        &mut self,
        each: impl FnOnce(&mut Self) -> Result<T, WireError>,
    ) -> Result<Option<T>, WireError> {
        if self.bool()? {
            each(self).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn list<T>(
        &mut self,
        mut each: impl FnMut(&mut Self) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        // Every item takes at least one byte; the vector grows only as items are read.
        let count = self.count()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(each(self)?);
        }
        Ok(items)
    }

    pub fn strings(&mut self) -> Result<Vec<String>, WireError> {
        self.list(Self::string)
    }

    pub fn opt_string(&mut self) -> Result<Option<String>, WireError> {
        self.option(Self::string)
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    /// A length or count, which cannot be more than the bytes left.
    fn count(&mut self) -> Result<usize, WireError> {
        let count = self.u64()?;
        if count > self.remaining() as u64 {
            return Err(WireError::Truncated);
        }
        Ok(count as usize)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], WireError> {
        if len > self.remaining() {
            return Err(WireError::Truncated);
        }
        let taken = &self.bytes[self.at..self.at + len];
        self.at += len;
        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8, WireError> {
        self.take(1).map(|bytes| bytes[0])
    }

    fn out_of_range() -> WireError {
        WireError::Invalid("a number out of range")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::Writer;

    /// The body of a frame holding what `fill` writes.
    fn body(fill: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut writer = Writer::frame(9);
        fill(&mut writer);
        Frame::body(&writer.finish().unwrap()).unwrap().to_vec()
    }

    fn reader(body: &[u8]) -> Reader<'_> {
        let (tag, reader) = Reader::open(body).unwrap();
        assert_eq!(tag, 9);
        reader
    }

    #[test]
    fn integers_are_leb128() {
        let numbers = [0, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX];
        let bytes = body(|w| numbers.iter().for_each(|n| w.u64(*n)));
        assert_eq!(&bytes[4..9], [0, 1, 127, 0x80, 1]);
        let mut r = reader(&bytes);
        for number in numbers {
            assert_eq!(r.u64().unwrap(), number);
        }
        r.finish().unwrap();
        // u64::MAX takes ten bytes, the last one being 1.
        assert_eq!(
            &bytes[bytes.len() - 10..],
            [0xff; 9].iter().chain(&[1]).copied().collect::<Vec<_>>()
        );
    }

    #[test]
    fn integers_past_64_bits_or_their_type_are_invalid() {
        let mut too_long = b"SW1\x09".to_vec();
        too_long.extend([0xff; 9]);
        too_long.push(2);
        assert_eq!(
            reader(&too_long).u64(),
            Err(WireError::Invalid("an integer longer than 64 bits"))
        );
        let mut endless = b"SW1\x09".to_vec();
        endless.extend([0x80; 12]);
        assert!(matches!(reader(&endless).u64(), Err(WireError::Invalid(_))));
        let bytes = body(|w| w.u64(70_000));
        assert_eq!(reader(&bytes).u16(), Err(Reader::out_of_range()));
        let bytes = body(|w| w.u64(256));
        assert_eq!(reader(&bytes).u8(), Err(Reader::out_of_range()));
    }

    #[test]
    fn values_round_trip() {
        let time = UNIX_EPOCH + Duration::from_millis(1_792_423_315_123);
        let net: Ipv4Net = "10.88.0.0/16".parse().unwrap();
        let socket: SocketAddrV4 = "10.66.0.53:5353".parse().unwrap();
        let bytes = body(|w| {
            w.bool(true);
            w.str("tést");
            w.time(time);
            w.time(UNIX_EPOCH - Duration::from_secs(5));
            w.socket(socket);
            w.net(net);
            w.option(Some(&7u64), |n, w| w.u64(*n));
            w.option(None::<&u64>, |n, w| w.u64(*n));
            w.strings(&["a".into(), String::new()]);
            w.choice(&'b', &['a', 'b', 'c']);
        });
        let mut r = reader(&bytes);
        assert!(r.bool().unwrap());
        assert_eq!(r.string().unwrap(), "tést");
        assert_eq!(r.time().unwrap(), time);
        assert_eq!(r.time().unwrap(), UNIX_EPOCH);
        assert_eq!(r.socket().unwrap(), socket);
        assert_eq!(r.net().unwrap(), net);
        assert_eq!(r.option(Reader::u64).unwrap(), Some(7));
        assert_eq!(r.option(Reader::u64).unwrap(), None);
        assert_eq!(r.strings().unwrap(), ["a", ""]);
        assert_eq!(r.choice(&['a', 'b', 'c']).unwrap(), 'b');
        r.finish().unwrap();
    }

    #[test]
    fn bad_values_are_errors() {
        let flag = b"SW1\x09\x02";
        assert!(matches!(reader(flag).bool(), Err(WireError::Invalid(_))));
        let option = b"SW1\x09\x07";
        assert!(reader(option).option(Reader::u64).is_err());
        let not_utf8 = b"SW1\x09\x02\xc3\x28";
        assert!(matches!(
            reader(not_utf8).string(),
            Err(WireError::Invalid(_))
        ));
        let long_string = b"SW1\x09\x05abc";
        assert_eq!(reader(long_string).string(), Err(WireError::Truncated));
        let huge_count = body(|w| w.u64(u64::MAX));
        assert_eq!(reader(&huge_count).strings(), Err(WireError::Truncated));
        let code = body(|w| w.u64(3));
        assert!(matches!(
            reader(&code).choice(&[1, 2, 3]),
            Err(WireError::Invalid(_))
        ));
        let net = body(|w| {
            w.ipv4(Ipv4Addr::new(10, 0, 0, 1));
            w.u64(33);
        });
        assert!(matches!(reader(&net).net(), Err(WireError::Invalid(_))));
        assert_eq!(reader(b"SW1\x09\x01\x02").ipv4(), Err(WireError::Truncated));
        let mut r = reader(b"SW1\x09\x01");
        assert_eq!(r.u64().unwrap(), 1);
        assert_eq!(r.u64(), Err(WireError::Truncated));
    }

    #[test]
    fn a_frame_needs_the_magic_the_tag_and_nothing_after_the_message() {
        assert_eq!(Reader::open(b"SW2\x01").err(), Some(WireError::BadMagic));
        assert_eq!(Reader::open(b"SW1").err(), Some(WireError::Truncated));
        assert_eq!(Reader::open(b"S").err(), Some(WireError::Truncated));
        let bytes = body(|w| w.u64(1));
        assert_eq!(reader(&bytes).finish(), Err(WireError::Trailing(1)));
    }

    #[test]
    fn a_frame_past_the_limit_is_not_made() {
        let mut writer = Writer::frame(1);
        writer.str(&"x".repeat(Frame::MAX_LEN));
        assert!(matches!(writer.finish(), Err(WireError::TooLarge(_))));
    }
}
