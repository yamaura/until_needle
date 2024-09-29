use crate::Needle;

// The trait to extend BufRead for until_needle functionality
pub trait UntilNeedleRead {
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
    fn read_until_needle(
        &mut self,
        needle: impl Needle,
        before: &mut Vec<u8>,
        matched: &mut Vec<u8>,
    ) -> std::io::Result<usize>;
}

impl<T: std::io::BufRead> UntilNeedleRead for T {
    fn read_until_needle(
        &mut self,
        needle: impl Needle,
        before: &mut Vec<u8>,
        matched: &mut Vec<u8>,
    ) -> std::io::Result<usize> {
        let mut total_buffered = 0;

        loop {
            let (done, used, buffered) = {
                let available = match self.fill_buf() {
                    Ok(n) => n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };

                let buffered = available.len() - total_buffered;

                if let Some(range) = needle.findin(available) {
                    before.extend_from_slice(&available[..range.start]);
                    matched.extend_from_slice(&available[range.clone()]);
                    (true, range.end, available.len() - range.end)
                } else if buffered > 0 {
                    (false, 0, buffered)
                } else {
                    // EOF
                    before.extend_from_slice(available);
                    (true, available.len(), 0)
                }
            };

            self.consume(used);
            if done {
                return Ok(used);
            }
            total_buffered += buffered;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Cursor};

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
        assert_eq!(cur.fill_buf().unwrap(), b"hello world!!!");
    }

    #[test]
    fn test_read_until_needle() {
        let data = b"hello world";
        let mut cur = Cursor::new(data);
        let mut before = Vec::new();
        let mut matched = Vec::new();
        assert_eq!(
            cur.read_until_needle(b"hello", &mut before, &mut matched)
                .unwrap(),
            5
        );
        assert_eq!(before, b"");
        assert_eq!(matched, b"hello");
        before.clear();
        matched.clear();
        assert_eq!(
            cur.read_until_needle(b"world", &mut before, &mut matched)
                .unwrap(),
            6
        );
        assert_eq!(before, b" ");
        assert_eq!(matched, b"world");
        before.clear();
        matched.clear();
        assert_eq!(
            cur.read_until_needle(b"foo", &mut before, &mut matched)
                .unwrap(),
            0
        );
        assert_eq!(before, b"");
        assert_eq!(matched, b"");
        cur.set_position(0);
        before.clear();
        matched.clear();
        assert_eq!(
            cur.read_until_needle(b"world", &mut before, &mut matched)
                .unwrap(),
            11
        );
        assert_eq!(before, b"hello ");
        assert_eq!(matched, b"world");
    }
}
