use crate::{Captures, Needle};
use std::io;

/// The `UntilNeedleRead` trait extends `BufRead` with functionality to read from an input stream
/// until a specified pattern (needle) is encountered or EOF is reached.
///
/// This trait provides two methods:
/// - [`read_until_needle`](#method.read_until_needle) returns a [`Captures`] struct with the
///   data before the needle, the needle (if found), and the total bytes read.
/// - [`split_read_until_needle`](#method.split_read_until_needle) splits the read data into two
///   buffers: one for the data before the needle and one for the needle itself.
///
/// # Examples
///
/// Reading until a needle is encountered:
///
/// ```rust
/// use std::io::Cursor;
/// use until_needle::{UntilNeedleRead, Needle, Captures};
///
/// let data = b"hello world!!";
/// let mut cur = Cursor::new(data);
/// let captures = cur.read_until_needle(b"world").unwrap();
///
/// assert_eq!(captures.before(), b"hello ");
/// assert_eq!(captures.matched(), b"world");
/// assert_eq!(captures.total_bytes_read(), cur.position() as usize);
/// ```
///
/// Splitting the read data into two buffers:
///
/// ```rust
/// use std::io::Cursor;
/// use until_needle::{UntilNeedleRead, Needle};
///
/// let data = b"hello world";
/// let mut cur = Cursor::new(data);
/// let mut before = Vec::new();
/// let mut matched = Vec::new();
///
/// let bytes_read = cur.split_read_until_needle(b"hello", &mut before, &mut matched).unwrap();
/// assert_eq!(bytes_read, 5);
/// assert_eq!(before, b"");
/// assert_eq!(matched, b"hello");
/// ```
pub trait UntilNeedleRead {
    /// Reads data from the underlying reader until the specified `needle` is encountered or EOF is reached.
    ///
    /// # Type Parameters
    ///
    /// - `N`: A type that implements the [`Needle`] trait, representing the search pattern.
    ///
    /// # Arguments
    ///
    /// - `needle`: The search pattern. The reading continues until this pattern is found in the input stream.
    ///
    /// # Returns
    ///
    /// - On success, returns a [`Captures`] instance containing:
    ///   - the bytes read before the needle (accessible via [`Captures::before`]),
    ///   - the needle itself if found (accessible via [`Captures::matched`]),
    ///   - and the total number of bytes read (accessible via [`Captures::total_bytes_read`]).
    ///
    /// - If the needle is not found before reaching EOF, [`Captures::matched`] will be empty.
    /// - Returns an `io::Error` if an I/O error occurs.
    fn read_until_needle<N>(&mut self, needle: N) -> io::Result<Captures>
    where
        N: Needle;

    /// Reads data from the underlying reader until the specified `needle` is found or EOF is reached.
    ///
    /// # Arguments
    /// - `needle`: An object implementing the `Needle` trait, which defines the search pattern.
    /// - `before`: A mutable buffer to store the data read before the `needle` is found.
    /// - `matched`: A mutable buffer to store the `needle` itself, if found. If EOF is reached without finding
    ///              the needle, this buffer will not be modified.
    ///
    /// # Returns
    /// - On success, it returns the total number of bytes read, including the needle.
    /// - If EOF is reached before the needle is found, `matched` will remain untouched.
    fn split_read_until_needle<N>(
        &mut self,
        needle: N,
        before: &mut Vec<u8>,
        matched: &mut Vec<u8>,
    ) -> io::Result<usize>
    where
        N: Needle,
    {
        let captures = self.read_until_needle(needle)?;
        let total_bytes_read = captures.total_bytes_read();
        let (b, m) = captures.split();
        before.extend_from_slice(&b);
        matched.extend_from_slice(&m);
        Ok(total_bytes_read)
    }
}

impl<T: std::io::BufRead> UntilNeedleRead for T {
    fn read_until_needle<N>(&mut self, needle: N) -> io::Result<Captures>
    where
        N: Needle,
    {
        let mut buf = Vec::new();
        loop {
            let available = self.fill_buf()?;
            let len = available.len();

            if len == 0 {
                let len = buf.len();
                return Ok(Captures::new(buf, len));
            }

            buf.extend_from_slice(available);

            if let Some(range) = needle.findin(&buf) {
                // Needle found
                let used = len.saturating_sub(buf.len().saturating_sub(range.end));
                self.consume(used);
                buf.truncate(range.end);
                return Ok(Captures::new(buf, range.start));
            } else {
                self.consume(len);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Cursor, Read};

    #[test]
    fn test_bufread() {
        let data = b"hello world";
        let mut cur = Cursor::new(data);
        assert_eq!(cur.fill_buf().unwrap(), data);
        assert_eq!(cur.fill_buf().unwrap(), data);

        let data = Vec::from(b"hello world");
        let mut cur = Cursor::new(data);
        assert_eq!(cur.fill_buf().unwrap(), b"hello world");
        assert_eq!(cur.fill_buf().unwrap(), b"hello world");
        cur.get_mut().extend_from_slice(b"!!!");
    }

    #[test]
    fn test_read_until_needle() {
        let data = b"hello world!!";
        let mut cur = Cursor::new(data);

        let cap = cur.read_until_needle(b"world").unwrap();

        assert_eq!(cap.before(), b"hello ");
        assert_eq!(cap.matched(), b"world");
        assert_eq!(cap.total_bytes_read(), cur.position() as usize);

        let mut remain = Vec::new();
        cur.read_to_end(&mut remain).unwrap();
        assert_eq!(remain, b"!!");
    }

    #[test]
    fn test_split_read_until_needle() {
        let data = b"hello world";
        let mut cur = Cursor::new(data);
        let mut before = Vec::new();
        let mut matched = Vec::new();
        assert_eq!(
            cur.split_read_until_needle(b"hello", &mut before, &mut matched)
                .unwrap(),
            5
        );
        assert_eq!(before, b"");
        assert_eq!(matched, b"hello");
        before.clear();
        matched.clear();
        assert_eq!(
            cur.split_read_until_needle(b"world", &mut before, &mut matched)
                .unwrap(),
            6
        );
        assert_eq!(before, b" ");
        assert_eq!(matched, b"world");
        before.clear();
        matched.clear();
        assert_eq!(
            cur.split_read_until_needle(b"foo", &mut before, &mut matched)
                .unwrap(),
            0
        );
        assert_eq!(before, b"");
        assert_eq!(matched, b"");
        cur.set_position(0);
        before.clear();
        matched.clear();
        assert_eq!(
            cur.split_read_until_needle(b"world", &mut before, &mut matched)
                .unwrap(),
            11
        );
        assert_eq!(before, b"hello ");
        assert_eq!(matched, b"world");
    }
}
