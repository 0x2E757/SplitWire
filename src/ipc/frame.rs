use std::io::{self, Read, Write};

use super::WireError;

/// One message on the pipe: a `u32` little-endian length of everything after it, then `SW1`, a tag
/// byte and the fields. One request gets one response.
pub struct Frame;

impl Frame {
    pub const MAGIC: &'static [u8; 3] = b"SW1";
    /// The largest length a frame may announce: 16 MiB.
    pub const MAX_LEN: usize = 16 << 20;
    /// The magic and the tag: the shortest body.
    pub const HEADER_LEN: usize = 4;

    /// Reads one frame and returns its body (from `SW1` on). The length is checked before anything
    /// is allocated, so a peer cannot make the reader allocate more than the limit.
    pub fn read(input: &mut impl Read) -> io::Result<Vec<u8>> {
        let mut len = [0; 4];
        input.read_exact(&mut len)?;
        let len = Self::checked_len(u32::from_le_bytes(len))?;
        let mut body = vec![0; len];
        input.read_exact(&mut body)?;
        Ok(body)
    }

    /// Writes a frame made by `Writer::finish`.
    pub fn write(output: &mut impl Write, frame: &[u8]) -> io::Result<()> {
        output.write_all(frame)?;
        output.flush()
    }

    /// The body of a whole frame held in memory, its length checked against the bytes.
    pub fn body(frame: &[u8]) -> Result<&[u8], WireError> {
        let (len, body) = frame.split_first_chunk::<4>().ok_or(WireError::Truncated)?;
        let len = Self::checked_len(u32::from_le_bytes(*len))?;
        match body.len().cmp(&len) {
            std::cmp::Ordering::Less => Err(WireError::Truncated),
            std::cmp::Ordering::Greater => Err(WireError::Trailing(body.len() - len)),
            std::cmp::Ordering::Equal => Ok(body),
        }
    }

    fn checked_len(len: u32) -> Result<usize, WireError> {
        let len = len as usize;
        if len > Self::MAX_LEN {
            return Err(WireError::TooLarge(len as u64));
        }
        if len < Self::HEADER_LEN {
            return Err(WireError::Truncated);
        }
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::Writer;

    #[test]
    fn a_frame_reads_back_from_a_stream() {
        let mut writer = Writer::frame(7);
        writer.str("hello");
        let frame = writer.finish().unwrap();
        let mut stream = Vec::new();
        Frame::write(&mut stream, &frame).unwrap();
        Frame::write(&mut stream, &frame).unwrap();
        let mut input = stream.as_slice();
        for _ in 0..2 {
            assert_eq!(
                Frame::read(&mut input).unwrap(),
                Frame::body(&frame).unwrap()
            );
        }
        assert_eq!(
            Frame::read(&mut input).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn an_oversized_length_is_refused_before_reading_the_body() {
        let len = (Frame::MAX_LEN as u32 + 1).to_le_bytes();
        let error = Frame::read(&mut len.as_slice()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.into_inner().unwrap().to_string(),
            WireError::TooLarge(Frame::MAX_LEN as u64 + 1).to_string()
        );
        assert_eq!(
            Frame::body(&u32::MAX.to_le_bytes()),
            Err(WireError::TooLarge(u64::from(u32::MAX)))
        );
    }

    #[test]
    fn short_and_mismatched_lengths_are_refused() {
        assert_eq!(Frame::body(&[1, 0]), Err(WireError::Truncated));
        assert_eq!(
            Frame::body(&[2, 0, 0, 0, b'S', b'W']),
            Err(WireError::Truncated)
        );
        let mut frame = Writer::frame(1).finish().unwrap();
        assert!(Frame::body(&frame).is_ok());
        frame.push(0);
        assert_eq!(Frame::body(&frame), Err(WireError::Trailing(1)));
        frame.truncate(frame.len() - 2);
        assert_eq!(Frame::body(&frame), Err(WireError::Truncated));
        let stream = [5u8, 0, 0, 0, b'S', b'W'];
        assert_eq!(
            Frame::read(&mut stream.as_slice()).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
