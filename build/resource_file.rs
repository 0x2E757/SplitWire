//! A compiled Win32 resource file (`.res`) the MSVC linker takes as an input (TaskbarMetrics'
//! approach): icon groups, each with one 32-bit image per size.

pub struct ResourceFile {
    bytes: Vec<u8>,
    /// The `RT_ICON` ids taken so far; every group's images get ids of their own.
    images: u16,
}

impl ResourceFile {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    const ENGLISH: u16 = 0x0409;

    /// A `.res` file opens with an empty entry that marks its format.
    pub fn new() -> Self {
        let mut file = Self {
            bytes: Vec::new(),
            images: 0,
        };
        file.entry(0, 0, 0, 0, &[]);
        file
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn entry(&mut self, kind: u16, name: u16, flags: u16, language: u16, data: &[u8]) {
        let out = &mut self.bytes;
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(32u32.to_le_bytes());
        for value in [0xFFFF, kind, 0xFFFF, name] {
            out.extend(value.to_le_bytes());
        }
        out.extend(0u32.to_le_bytes());
        out.extend(flags.to_le_bytes());
        out.extend(language.to_le_bytes());
        out.extend([0u8; 8]);
        out.extend(data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }

    /// Icon group `group` with one image per `(size, pixels)`; `LoadImageW` picks the size.
    pub fn icon_group(mut self, group: u16, images: &[(u32, Vec<u32>)]) -> Self {
        let mut directory = Vec::new();
        for value in [0u16, 1, images.len() as u16] {
            directory.extend(value.to_le_bytes());
        }
        for (size, pixels) in images {
            self.images += 1;
            let id = self.images;
            let image = Self::bitmap(*size, pixels);
            let side = (*size % 256) as u8;
            directory.extend([side, side, 0, 0]);
            directory.extend(1u16.to_le_bytes());
            directory.extend(32u16.to_le_bytes());
            directory.extend((image.len() as u32).to_le_bytes());
            directory.extend(id.to_le_bytes());
            self.entry(Self::RT_ICON, id, 0x1010, Self::ENGLISH, &image);
        }
        self.entry(
            Self::RT_GROUP_ICON,
            group,
            0x1030,
            Self::ENGLISH,
            &directory,
        );
        self
    }

    /// A DIB icon image: the header with a doubled height, the colours bottom-up, then the 1-bit
    /// mask, set where the image is fully transparent.
    fn bitmap(size: u32, pixels: &[u32]) -> Vec<u8> {
        let stride = size.div_ceil(32) * 4;
        let colors = size * size * 4;
        let mut out = Vec::new();
        for value in [40u32, size, size * 2] {
            out.extend(value.to_le_bytes());
        }
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend((colors + stride * size).to_le_bytes());
        out.extend([0u8; 16]);
        for row in (0..size).rev() {
            for pixel in &pixels[(row * size) as usize..((row + 1) * size) as usize] {
                out.extend(pixel.to_le_bytes());
            }
        }
        for row in (0..size).rev() {
            let mut mask = vec![0u8; stride as usize];
            for column in 0..size {
                if pixels[(row * size + column) as usize] >> 24 == 0 {
                    mask[(column / 8) as usize] |= 0x80 >> (column % 8);
                }
            }
            out.extend(mask);
        }
        out
    }
}
