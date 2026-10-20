//! A PNG file from BGRA pixels, with the standard library only: each row unfiltered, compressed
//! by deflate with fixed Huffman codes, where a run repeats the pixel before it or the row above.
//! That is all a picture of a page needs to stay small.

pub struct Png;

/// Bits written least significant first, as deflate packs them.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    buffer: u32,
    count: u32,
}

impl Bits {
    fn put(&mut self, value: u32, count: u32) {
        self.buffer |= value << self.count;
        self.count += count;
        while self.count >= 8 {
            self.bytes.push(self.buffer as u8);
            self.buffer >>= 8;
            self.count -= 8;
        }
    }

    /// A Huffman code, written most significant bit first.
    fn code(&mut self, code: u32, length: u32) {
        let reversed = code.reverse_bits() >> (32 - length);
        self.put(reversed, length);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.bytes.push(self.buffer as u8);
        }
        self.bytes
    }
}

impl Png {
    const LENGTH_BASE: [u32; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LENGTH_EXTRA: [u32; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DISTANCE_BASE: [u32; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DISTANCE_EXTRA: [u32; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    /// The file of a `width` × `height` picture of premultiplied BGRA pixels, made opaque.
    pub fn encode(width: u32, height: u32, bgra: &[u8]) -> Vec<u8> {
        let stride = width as usize * 4;
        let mut raw = Vec::with_capacity((stride + 1) * height as usize);
        for row in bgra.chunks_exact(stride).take(height as usize) {
            raw.push(0);
            for [b, g, r, _] in row.as_chunks::<4>().0 {
                raw.extend_from_slice(&[*r, *g, *b, 255]);
            }
        }
        let mut header = Vec::new();
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut file = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        Self::chunk(&mut file, b"IHDR", &header);
        Self::chunk(&mut file, b"IDAT", &Self::zlib(&raw, &[4, stride + 1]));
        Self::chunk(&mut file, b"IEND", &[]);
        file
    }

    fn chunk(file: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        file.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = file.len();
        file.extend_from_slice(kind);
        file.extend_from_slice(data);
        let crc = Self::crc32(&file[start..]);
        file.extend_from_slice(&crc.to_be_bytes());
    }

    /// A zlib stream of one fixed-Huffman block; runs are looked for only at `distances`.
    fn zlib(data: &[u8], distances: &[usize]) -> Vec<u8> {
        let mut bits = Bits::default();
        bits.put(1, 1);
        bits.put(1, 2);
        let mut at = 0;
        while at < data.len() {
            let best = distances
                .iter()
                .filter(|&&d| d <= at && d <= 32768)
                .map(|&d| {
                    let length = (0..258.min(data.len() - at))
                        .take_while(|&i| data[at + i] == data[at + i - d])
                        .count();
                    (length, d)
                })
                .max();
            match best {
                Some((length, distance)) if length >= 3 => {
                    Self::pair(&mut bits, length as u32, distance as u32);
                    at += length;
                }
                _ => {
                    Self::literal(&mut bits, u32::from(data[at]));
                    at += 1;
                }
            }
        }
        Self::literal(&mut bits, 256);
        let mut stream = vec![0x78, 0x01];
        stream.extend(bits.finish());
        stream.extend_from_slice(&Self::adler32(data).to_be_bytes());
        stream
    }

    fn literal(bits: &mut Bits, value: u32) {
        match value {
            0..=143 => bits.code(0x30 + value, 8),
            144..=255 => bits.code(0x190 + value - 144, 9),
            256..=279 => bits.code(value - 256, 7),
            _ => bits.code(0xC0 + value - 280, 8),
        }
    }

    fn pair(bits: &mut Bits, length: u32, distance: u32) {
        let index = Self::LENGTH_BASE
            .iter()
            .rposition(|&b| b <= length)
            .unwrap_or(0);
        Self::literal(bits, 257 + index as u32);
        bits.put(length - Self::LENGTH_BASE[index], Self::LENGTH_EXTRA[index]);
        let index = Self::DISTANCE_BASE
            .iter()
            .rposition(|&b| b <= distance)
            .unwrap_or(0);
        bits.code(index as u32, 5);
        bits.put(
            distance - Self::DISTANCE_BASE[index],
            Self::DISTANCE_EXTRA[index],
        );
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + u32::from(byte)) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_are_the_standard_ones() {
        assert_eq!(Png::crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(Png::adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn a_picture_has_its_header_and_chunks() {
        let pixels = [0x10, 0x20, 0x30, 0xFF].repeat(3 * 2);
        let file = Png::encode(3, 2, &pixels);
        assert_eq!(&file[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&file[12..16], b"IHDR");
        assert_eq!(&file[16..20], &3u32.to_be_bytes());
        assert!(file.ends_with(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]));
    }

    #[test]
    fn runs_shrink_a_flat_picture() {
        let pixels = [1, 2, 3, 255].repeat(200 * 100);
        assert!(Png::encode(200, 100, &pixels).len() < 3000);
    }
}
