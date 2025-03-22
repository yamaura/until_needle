#![doc = include_str!("../README.md")]
/// Implementation for futures
#[cfg(feature = "futures")]
pub mod futures;
/// Implementation for std::io
pub mod io;
pub mod needle;
pub use crate::io::UntilNeedleRead;
pub use crate::needle::Needle;

/// `Captures` represents the result of reading data from a stream until a specified needle is found.
///
/// This structure holds the complete buffer of bytes read and a split index (`match_start`)
/// indicating where the matching segment (needle) begins. It allows easy access to:
/// - The data before the match (via [`Captures::before`])
/// - The matching segment (via [`Captures::matched`])
/// - The entire buffer (via [`Captures::as_bytes`] or [`Captures::into_bytes`])
///
/// # Examples
///
/// ```rust
/// use std::io::{BufReader, Cursor};
/// use until_needle::{Captures, Needle, UntilNeedleRead};
///
/// let mut data = BufReader::new(Cursor::new(b"hello world"));
/// let captures = data.read_until_needle("world").unwrap();
///
/// assert_eq!(captures.before(), b"hello ");
/// assert_eq!(captures.matched(), b"world");
/// assert_eq!(captures.total_bytes_read(), b"hello world".len());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Captures {
    buf: Vec<u8>,
    // The range of bytes before the match.
    match_start: usize,
}

impl Captures {
    fn new(buf: Vec<u8>, match_start: usize) -> Self {
        Self { buf, match_start }
    }

    /// Returns a slice of the data before the match.
    ///
    /// This method provides access to the portion of the buffer preceding the matching segment.
    pub fn before(&self) -> &[u8] {
        &self.buf[..self.match_start]
    }

    /// Returns a slice of the matched data.
    ///
    /// This method provides access to the matching segment (needle) within the buffer.
    pub fn matched(&self) -> &[u8] {
        &self.buf[self.match_start..]
    }

    /// Consumes the `Captures` instance and returns the entire buffer.
    ///
    /// This method transfers ownership of the complete buffer, which contains both the data before the match
    /// and the matched segment.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Splits the captured data into two vectors.
    ///
    /// Returns a tuple where:
    /// - The first element contains the bytes before the match.
    /// - The second element contains the matched segment.
    pub fn split(self) -> (Vec<u8>, Vec<u8>) {
        let mut buf = self.buf;
        let matched = buf.split_off(self.match_start);
        (buf, matched)
    }

    /// Returns a slice of the entire captured buffer.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Returns the total number of bytes read.
    ///
    /// This is the length of the entire buffer, representing both the data before the match and the match itself.
    pub fn total_bytes_read(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures() {
        let buf = b"hello world";
        let match_start = "world".findin(buf).unwrap().start;
        let captures = Captures::new(buf.to_vec(), match_start);
        assert_eq!(captures.before(), b"hello ");
        assert_eq!(captures.matched(), b"world");
        assert_eq!(captures.as_bytes(), buf);
        assert_eq!(captures.total_bytes_read(), buf.len());
        let (before, matched) = captures.split();
        assert_eq!(before, b"hello ");
        assert_eq!(matched, b"world");
    }
}
