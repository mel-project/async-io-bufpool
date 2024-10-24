use std::future::Future;

use bytes::Bytes;
use crossbeam_queue::SegQueue;
use futures_util::AsyncRead;

static POOL: SegQueue<Vec<u8>> = SegQueue::new();

/// Read an async reader into a buffer, while not consuming any memory before the read unblocks.
pub async fn pooled_read(rdr: impl AsyncRead + Unpin) -> Result<Bytes, std::io::Error> {
    PooledOnceReader(rdr).await
}

struct PooledOnceReader<T: AsyncRead>(T);

impl<T: AsyncRead> Future for PooledOnceReader<T> {
    type Output = Result<Bytes, std::io::Error>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut free_buf = POOL.pop().unwrap_or_else(|| vec![0u8; 8192]);
        let pinned = unsafe { std::pin::Pin::map_unchecked_mut(self, |s| &mut s.0) };
        match pinned.poll_read(cx, &mut free_buf) {
            std::task::Poll::Ready(Ok(n)) => {
                free_buf.truncate(n);
                std::task::Poll::Ready(Ok(free_buf.into()))
            }
            std::task::Poll::Ready(Err(err)) => {
                POOL.push(free_buf);
                std::task::Poll::Ready(Err(err))
            }
            std::task::Poll::Pending => todo!(),
        }
    }
}
