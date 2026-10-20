use std::{fmt, io};

/// Why bytes are not a valid frame or message. The service answers any of them with `bad_request`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireError {
    /// The bytes end before the value does, or a length or count runs past them.
    Truncated,
    /// The frame announces more than `Frame::MAX_LEN` bytes.
    TooLarge(u64),
    /// The frame does not start with `SW1`.
    BadMagic,
    UnknownTag(u8),
    /// A value that is not allowed here: bad UTF-8, an unknown code, a number out of range.
    Invalid(&'static str),
    /// Bytes left over after the message.
    Trailing(usize),
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::Truncated => f.write_str("the message is cut short"),
            WireError::TooLarge(len) => write!(f, "a frame of {len} bytes is over the limit"),
            WireError::BadMagic => f.write_str("not a SplitWire frame"),
            WireError::UnknownTag(tag) => write!(f, "unknown message tag {tag}"),
            WireError::Invalid(what) => write!(f, "invalid message: {what}"),
            WireError::Trailing(count) => write!(f, "{count} bytes after the message"),
        }
    }
}

impl std::error::Error for WireError {}

impl From<WireError> for io::Error {
    fn from(error: WireError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}
