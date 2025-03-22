use futures_core::ready;
use futures_util::io::AsyncBufRead;
use std::future::Future;
use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use crate::{Captures, Needle};

/// The `AsyncUntilNeedleRead` trait extends [`AsyncBufRead`] with asynchronous functionality
/// to read from a stream until a specified pattern (needle) is encountered or EOF is reached.
///
/// The asynchronous method returns a [`Captures`] instance containing:
/// - The complete buffer of bytes read from the stream,
/// - A split index indicating the boundary between the data read before the needle and the needle itself.
///
/// # Examples
///
/// ```rust
/// #[cfg(feature = "futures")]
/// use futures_util::io::{AsyncBufRead, BufReader, Cursor};
/// use until_needle::futures::{AsyncUntilNeedleRead, Needle, Captures};
/// use futures::executor::block_on;
///
/// let data = b"hello world!!";
/// let mut reader = BufReader::new(Cursor::new(data));
///  
/// // Asynchronously read until the needle "world" is found.
/// // When successful, `captures.before()` returns `b"hello "`
/// // and `captures.matched()` returns `b"world"`.
/// let captures: Captures = block_on(reader.read_until_needle(b"world"))
///     .expect("Read should succeed");
///
/// assert_eq!(captures.before(), b"hello ");
/// assert_eq!(captures.matched(), b"world");
/// ```
pub trait AsyncUntilNeedleRead: AsyncBufRead {
    /// Asynchronously reads data from the underlying reader until the specified `needle` is encountered or EOF is reached.
    ///
    /// # Arguments
    ///
    /// - `needle`: An object implementing the [`Needle`] trait that defines the search pattern.
    ///
    /// # Returns
    ///
    /// On success, returns a future that resolves to a [`Captures`] instance containing:
    /// - The bytes read before the needle (accessible via [`Captures::before`]),
    /// - The needle itself (accessible via [`Captures::matched`]),
    /// - And the total number of bytes read (accessible via [`Captures::total_bytes_read`]).
    ///
    /// If EOF is reached without finding the needle, the `matched` part will be empty.
    fn read_until_needle<'a, N>(&'a mut self, needle: N) -> ReadUntilNeedle<'a, Self, N>
    where
        Self: Unpin + Sized,
        N: Needle + 'a;

    /// Asynchronously reads data from the underlying reader until the specified `needle` is encountered or EOF is reached,
    /// and splits the read data into two parts: one for the data before the needle and one for the needle itself.
    ///
    /// # Arguments
    ///
    /// - `needle`: An object implementing the [`Needle`] trait that defines the search pattern.
    /// - `before`: A mutable vector that will be extended with the data read before the needle is found.
    /// - `matched`: A mutable vector that will be extended with the needle itself if found. If the needle is not found,
    ///              this vector remains unmodified.
    ///
    /// # Returns
    ///
    /// On success, returns a future that resolves to the total number of bytes read (including the needle if found).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "futures")]
    /// use futures_util::io::{AsyncBufRead, BufReader, Cursor};
    /// use until_needle::futures::{AsyncUntilNeedleRead, Needle, Captures};
    /// use futures::executor::block_on;
    ///
    /// let data = b"hello world";
    /// let mut reader = BufReader::new(Cursor::new(data));
    /// let mut before = Vec::new();
    /// let mut matched = Vec::new();
    ///
    /// let bytes_read = block_on(reader.split_read_until_needle(b"world", &mut before, &mut matched))
    ///     .expect("Read should succeed");
    ///
    /// assert_eq!(before, b"hello ");
    /// assert_eq!(matched, b"world");
    /// ```
    fn split_read_until_needle<'a, N>(
        &'a mut self,
        needle: N,
        before: &'a mut Vec<u8>,
        matched: &'a mut Vec<u8>,
    ) -> impl Future<Output = io::Result<usize>> + 'a
    where
        Self: Unpin + Sized,
        N: Needle + 'a,
    {
        async move {
            let captures = self.read_until_needle(needle).await?;
            let total_bytes_read = captures.total_bytes_read();
            let (b, m) = captures.split();
            before.extend_from_slice(&b);
            matched.extend_from_slice(&m);
            Ok(total_bytes_read)
        }
    }
}

impl<R> AsyncUntilNeedleRead for R
where
    R: AsyncBufRead + Unpin,
{
    fn read_until_needle<'a, N>(&'a mut self, needle: N) -> ReadUntilNeedle<'a, Self, N>
    where
        Self: Unpin + Sized,
        N: Needle + 'a,
    {
        ReadUntilNeedle {
            reader: self,
            needle,
            buf: Vec::new(),
        }
    }
}

/// A future that resolves when data has been read from an asynchronous reader until the specified needle is found.
///
/// When the future resolves, it returns a [`Captures`] instance which contains the
/// full buffer of data read and a split index indicating where the needle begins.
pub struct ReadUntilNeedle<'a, R, N>
where
    R: Unpin + ?Sized,
{
    reader: &'a mut R,
    needle: N,
    buf: Vec<u8>,
}

impl<R: ?Sized + Unpin, N> Unpin for ReadUntilNeedle<'_, R, N> {}

impl<'a, R, N> Future for ReadUntilNeedle<'a, R, N>
where
    R: AsyncBufRead + Unpin + ?Sized,
    N: Needle,
{
    type Output = io::Result<Captures>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Destructure self to obtain mutable references to our fields.
        let ReadUntilNeedle {
            reader,
            needle,
            buf,
        } = &mut *self;
        let reader = Pin::new(reader);
        read_until_needle_internal(reader, cx, needle, buf)
    }
}

/// Internal function to asynchronously read data until the needle is found.
///
/// This function repeatedly polls the underlying reader, appending available data into an internal buffer.
/// Once the needle is found or EOF is reached, it returns a [`Captures`] instance constructed from the buffer.
/// The split index provided to [`Captures::new`] indicates the boundary between the bytes read before the needle
/// and the needle itself.
///
/// # Arguments
///
/// - `reader`: A pinned mutable reference to the asynchronous reader.
/// - `cx`: The asynchronous task context.
/// - `needle`: A reference to the needle used for pattern matching.
/// - `buf`: The internal buffer that accumulates data from the reader.
///
/// # Returns
///
/// On success, returns `Poll::Ready(Ok(captures))` where `captures` is a [`Captures`] instance.
/// If an I/O error occurs, returns `Poll::Ready(Err(e))`.
fn read_until_needle_internal<R, N>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    needle: &N,
    buf: &mut Vec<u8>,
) -> Poll<io::Result<Captures>>
where
    R: AsyncBufRead + ?Sized,
    N: Needle,
{
    loop {
        let available = ready!(reader.as_mut().poll_fill_buf(cx))?;
        let len = available.len();

        if len == 0 {
            // EOF
            return Poll::Ready(Ok(Captures::new(mem::take(buf), buf.len())));
        }

        // Append available bytes to the internal buffer.
        buf.extend_from_slice(available);

        if let Some(range) = needle.findin(buf) {
            // Needle found.
            // Compute how many bytes from the current available slice should be consumed.
            let used = len.saturating_sub(buf.len().saturating_sub(range.end));
            reader.as_mut().consume(used);
            buf.truncate(range.end);
            // Construct a Captures instance:
            // - The entire buffered data is moved into the Captures.
            // - The split index is `range.start`, indicating that:
            //   - `captures.before()` is `&buf[..range.start]`
            //   - `captures.matched()` is `&buf[range.start..range.end]`
            return Poll::Ready(Ok(Captures::new(mem::take(buf), range.start)));
        } else {
            // No needle found in the current buffer; consume all available bytes.
            reader.as_mut().consume(len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{
        stream::{iter, TryStreamExt as _},
        AsyncReadExt as _,
    };

    #[tokio::test]
    async fn test_async_read() {
        let mut stream = iter(vec![
            Ok(b"hello".to_vec()),
            Ok(b" wo".to_vec()),
            Ok(b"rld!".to_vec()),
        ])
        .into_async_read();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"hello world!");
    }

    #[tokio::test]
    async fn test_split_read_until_needle() {
        let mut stream = iter(vec![
            Ok(b"hello".to_vec()),
            Ok(b" wo".to_vec()),
            Ok(b"rld!!".to_vec()),
        ])
        .into_async_read();

        let mut before = Vec::new();
        let mut matched = Vec::new();
        let mut buf = Vec::new();

        assert_eq!(
            stream
                .split_read_until_needle(b"world", &mut before, &mut matched)
                .await
                .unwrap(),
            11
        );
        assert_eq!(before, b"hello ");
        assert_eq!(matched, b"world");
        assert_eq!(stream.read_to_end(&mut buf).await.unwrap(), 2);
        assert_eq!(buf, b"!!");
    }
}
