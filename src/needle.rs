use std::ops::Range;

pub trait Needle {
    /// Finds the first occurrence of the pattern in the given haystack (as &[u8]).
    /// Returns a `Range<usize>` if found, otherwise returns `None`.
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>>;
}

impl Needle for [u8] {
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>> {
        haystack
            .windows(self.len())
            .position(|window| window == self)
            .map(|pos| pos..pos + self.len())
    }
}

impl Needle for &[u8] {
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>> {
        haystack
            .windows(self.len())
            .position(|window| window == *self)
            .map(|pos| pos..pos + self.len())
    }
}

impl<const N: usize> Needle for &[u8; N] {
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>> {
        self[..].findin(haystack)
    }
}

impl Needle for Vec<u8> {
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>> {
        self.as_slice().findin(haystack)
    }
}

impl Needle for &str {
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>> {
        self.as_bytes().findin(haystack)
    }
}

impl Needle for String {
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>> {
        self.as_str().findin(haystack)
    }
}

#[cfg(feature = "regex")]
impl Needle for regex::bytes::Regex {
    fn findin(&self, haystack: &[u8]) -> Option<Range<usize>> {
        self.find(haystack).map(|m| m.start()..m.end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_findin() {
        let haystack = b"hello world";
        assert_eq!(b"hello".findin(haystack), Some(0..5));
        assert_eq!(b"world".findin(haystack), Some(6..11));
        assert_eq!(b"foo".findin(haystack), None);

        let haystack = "hello world".as_bytes();
        assert_eq!("hello".findin(haystack), Some(0..5));
        assert_eq!("world".findin(haystack), Some(6..11));
        assert_eq!("foo".findin(haystack), None);

    }

    #[cfg(feature = "regex")]
    #[test]
    fn test_regex_findin() {
        let haystack = b" hello world";
        let regex = regex::bytes::Regex::new(r"\b\w+\b").unwrap();
        assert_eq!(regex.findin(haystack), Some(1..6));
    }
}
