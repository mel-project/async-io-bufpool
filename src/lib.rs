#![doc = pretty_readme::docify!("README.md", "https://docs.rs/super-cool-crate/latest/super-cool-crate/", "./")]

use std::{cell::RefCell, future::Future};

use bytes::Bytes;

use crossbeam_queue::SegQueue;
use futures_util::{AsyncRead, AsyncWrite};
use pin_project_lite::pin_project;

thread_local! {
    static BUFFER: RefCell<[u8; 65536]> = const { RefCell::new([0u8; 65536]) }
}

/// Read an async reader into a buffer. This is done in a memory-efficient way, avoiding consuming any memory before the read unblocks.
///
/// An empty return value indicates EOF.
pub async fn pooled_read(
    rdr: impl AsyncRead,
    limit: usize,
) -> Result<Option<Bytes>, std::io::Error> {
    PooledOnceReader {
        rdr,
        resolve: |b: &[u8]| Bytes::copy_from_slice(b),
        limit,
    }
    .await
}

/// Read an async reader into a buffer, but instead of allocating memory, call a callback.
///
/// An empty return value indicates EOF.
pub async fn pooled_read_callback<T>(
    rdr: impl AsyncRead,
    limit: usize,
    resolve: impl FnMut(&[u8]) -> T,
) -> Result<Option<T>, std::io::Error> {
    PooledOnceReader {
        rdr,
        resolve,
        limit,
    }
    .await
}

/// Copy data from an async reader to an async writer using a thread-local buffer.
/// Returns the total number of bytes copied.
pub async fn pooled_copy<R, W>(mut reader: R, mut writer: W) -> std::io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut total_bytes = 0u64;

    static BUFFS: SegQueue<Box<[u8; 8192]>> = SegQueue::new();

    loop {
        let (buff, n) = match pooled_read_callback(&mut reader, 8192, |bts| {
            let mut buff = BUFFS.pop().unwrap_or_else(|| Box::new([0u8; 8192]));
            buff[..bts.len()].copy_from_slice(bts);
            (buff, bts.len())
        })
        .await?
        {
            Some(x) => x,
            None => break, // End of file
        };

        let bytes_read = n as u64;
        futures_util::AsyncWriteExt::write_all(&mut writer, &buff[..n]).await?;
        total_bytes += bytes_read;
    }

    Ok(total_bytes)
}

pin_project! {
struct PooledOnceReader<T, F>{
    #[pin]
    rdr: T,
    resolve: F,
    limit: usize
}
}
impl<T: AsyncRead, U, F: FnMut(&[u8]) -> U> Future for PooledOnceReader<T, F> {
    type Output = Result<Option<U>, std::io::Error>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        BUFFER.with(|buf| {
            let mut buf = buf.borrow_mut();
            let this = self.project();
            let limit = (*this.limit).min(buf.len());
            match this.rdr.poll_read(cx, &mut buf[..limit]) {
                std::task::Poll::Ready(Ok(n)) => {
                    if n == 0 {
                        std::task::Poll::Ready(Ok(None))
                    } else {
                        std::task::Poll::Ready(Ok(Some((this.resolve)(&buf[..n]))))
                    }
                }
                std::task::Poll::Ready(Err(err)) => std::task::Poll::Ready(Err(err)),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pollster::FutureExt;

    #[test]
    fn test_pooled_read() {
        // Create test data
        let test_data = b"Hello, World!";

        // Run the pooled_read function
        let result = pooled_read(&test_data[..], 10000).block_on();

        // Verify the result
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(bytes, Some(Bytes::from_static(test_data)));
        assert_eq!(bytes.unwrap().len(), test_data.len());
    }
}
