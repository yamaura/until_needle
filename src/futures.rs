use crate::Needle;
use futures_core::ready;
use futures_util::io::AsyncBufRead;
use std::future::Future;
use std::io::{self};
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

/// The trait to extend `AsyncBufRead` for `read_until_needle` functionality.
pub trait AsyncUntilNeedleRead: futures_util::io::AsyncBufRead {
    /// Asynchronously reads data from the underlying reader until the specified `needle` is found or EOF is reached.
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
    fn read_until_needle<'a, N>(
        &'a mut self,
        needle: N,
        before: &'a mut Vec<u8>,
        matched: &'a mut Vec<u8>,
    ) -> ReadUntilNeedle<'a, Self, N>
    where
        Self: Unpin + Sized,
        N: Needle + 'a;
}

impl<R> AsyncUntilNeedleRead for R
where
    R: AsyncBufRead + Unpin,
{
    fn read_until_needle<'a, N>(
        &'a mut self,
        needle: N,
        before: &'a mut Vec<u8>,
        matched: &'a mut Vec<u8>,
    ) -> ReadUntilNeedle<'a, Self, N>
    where
        Self: Unpin + Sized,
        N: Needle + 'a,
    {
        ReadUntilNeedle {
            reader: self,
            needle,
            buf: Vec::new(),
            before,
            matched,
            total_bytes_read: 0,
        }
    }
}

/// A future that reads data until the specified needle is found.
pub struct ReadUntilNeedle<'a, R, N>
where
    R: Unpin + ?Sized,
{
    reader: &'a mut R,
    needle: N,
    buf: Vec<u8>,
    before: &'a mut Vec<u8>,
    matched: &'a mut Vec<u8>,
    total_bytes_read: usize,
}

impl<R: ?Sized + Unpin, N> Unpin for ReadUntilNeedle<'_, R, N> {}

impl<'a, R, N> Future for ReadUntilNeedle<'a, R, N>
where
    R: AsyncBufRead + Unpin + ?Sized,
    N: Needle,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let ReadUntilNeedle {
            reader,
            needle,
            buf,
            before,
            matched,
            total_bytes_read,
        } = &mut *self;
        let reader = Pin::new(reader);
        read_until_needle_internal(reader, cx, needle, buf, before, matched, total_bytes_read)
    }
}

/// Internal function to read until the needle is found.
fn read_until_needle_internal<R, N>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    needle: &N,
    buf: &mut Vec<u8>,
    before: &mut Vec<u8>,
    matched: &mut Vec<u8>,
    total_bytes_read: &mut usize,
) -> Poll<io::Result<usize>>
where
    R: AsyncBufRead + ?Sized,
    N: Needle,
{
    loop {
        let (done, used) = {
            let available = ready!(reader.as_mut().poll_fill_buf(cx))?;
            buf.extend_from_slice(available);
            // must consume to detect EOF

            if available.is_empty() {
                // EOF reached
                before.extend_from_slice(buf);
                (true, available.len())
            } else if let Some(range) = needle.findin(buf) {
                // Needle found
                before.extend_from_slice(&buf[..range.start]);
                matched.extend_from_slice(&buf[range.clone()]);
                (true, available.len() - (buf.len() - range.end))
            } else {
                (false, available.len())
            }
        };

        reader.as_mut().consume(used);
        *total_bytes_read += used;

        if done {
            return Poll::Ready(Ok(mem::replace(total_bytes_read, 0)));
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
    async fn test_read_until_needle() {
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
                .read_until_needle(b"world", &mut before, &mut matched)
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
