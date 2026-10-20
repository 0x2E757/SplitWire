use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{Frame, WireError};
use crate::net::Ipv4Net;

/// Builds one frame. Integers are unsigned LEB128, strings a length plus UTF-8, lists a count plus
/// the items, options a 0/1 byte plus the value, enums the index of the variant, times Unix
/// milliseconds and IPv4 addresses their 4 bytes.
pub struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    /// A frame with the message `tag`; its length is filled in by `finish`.
    pub fn frame(tag: u8) -> Self {
        Self::frame_with_capacity(tag, 0)
    }

    /// A frame whose buffer holds `capacity` bytes of fields without growing: a frame that carries
    /// a `.conf` leaves no copy of it behind in freed memory.
    pub fn frame_with_capacity(tag: u8, capacity: usize) -> Self {
        let mut bytes = Vec::with_capacity(4 + Frame::HEADER_LEN + capacity);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(Frame::MAGIC);
        bytes.push(tag);
        Self { bytes }
    }

    /// The whole frame, length first, or `TooLarge` past `Frame::MAX_LEN`.
    pub fn finish(mut self) -> Result<Vec<u8>, WireError> {
        let len = self.bytes.len() - 4;
        if len > Frame::MAX_LEN {
            return Err(WireError::TooLarge(len as u64));
        }
        self.bytes[..4].copy_from_slice(&(len as u32).to_le_bytes());
        Ok(self.bytes)
    }

    pub fn u64(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.bytes.push(value as u8 | 0x80);
            value >>= 7;
        }
        self.bytes.push(value as u8);
    }

    pub fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    pub fn u16(&mut self, value: u16) {
        self.u64(value.into());
    }

    pub fn bool(&mut self, value: bool) {
        self.bytes.push(value.into());
    }

    pub fn str(&mut self, text: &str) {
        self.usize(text.len());
        self.bytes.extend_from_slice(text.as_bytes());
    }

    /// Milliseconds since 1970; an earlier time is written as 1970.
    pub fn time(&mut self, time: SystemTime) {
        let millis = time.duration_since(UNIX_EPOCH).map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        });
        self.u64(millis);
    }

    pub fn ipv4(&mut self, address: Ipv4Addr) {
        self.bytes.extend_from_slice(&address.octets());
    }

    pub fn socket(&mut self, socket: SocketAddrV4) {
        self.ipv4(*socket.ip());
        self.u16(socket.port());
    }

    pub fn net(&mut self, net: Ipv4Net) {
        self.ipv4(net.addr());
        self.u64(net.prefix().into());
    }

    /// The index of `value` in `all`, the list of every variant of a fieldless enum.
    pub fn choice<T: PartialEq>(&mut self, value: &T, all: &[T]) {
        let index = all
            .iter()
            .position(|item| item == value)
            .expect("every variant is listed");
        self.usize(index);
    }

    pub fn option<T>(&mut self, value: Option<&T>, each: impl FnOnce(&T, &mut Self)) {
        self.bool(value.is_some());
        if let Some(value) = value {
            each(value, self);
        }
    }

    pub fn list<T>(&mut self, items: &[T], mut each: impl FnMut(&T, &mut Self)) {
        self.usize(items.len());
        for item in items {
            each(item, self);
        }
    }

    /// A list of strings, the most common list.
    pub fn strings(&mut self, items: &[String]) {
        self.list(items, |item, w| w.str(item));
    }

    /// An optional string.
    pub fn opt_str(&mut self, value: Option<&String>) {
        self.option(value, |text, w| w.str(text));
    }
}
