# Memory-efficient async reads

**Efficient asynchronous I/O operations with minimal idle memory overhead**

---

## Overview

`async_io_bufpool` provides asynchronous reading and copying utilities optimized for low memory overhead during pending I/O operations. This crate ensures that memory is only allocated when actual data is ready to be read, preventing memory from being wasted during idle polling periods. It achieves this by leveraging thread-local buffers and deferred allocation strategies.

This crate uses `futures` types. Use a crate like `async-compat` for `tokio` compatibility.

**Note**: This is done at the cost of one extra copy per read --- make sure the tradeoff is worthwhile in your app!

---

## Motivation

Typical asynchronous read operations in Rust involve providing a async task providing a buffer upfront and polling the `AsyncRead` until completion with the same buffer. This which can lead to unnecessary memory consumption when multiple async reads are pending simultaneously.

For instance, in an a high-concurrency server where millions of tasks are idle and waiting for clients to send data, there will be millions of empty byte buffers allocated, one for each pending `AsyncReadExt::read()` call.

`async_io_bufpool` addresses this problem by deferring buffer allocations until data is actually available, borrowing thread-local buffers during polling and allocating fresh buffers only when necessary.

---

## Usage Examples

### Reading from an async source:

```rust
use async_io_bufpool::pooled_read;
use futures_util::AsyncReadExt;

async fn example_read<R: AsyncReadExt + Unpin>(reader: &mut R) -> std::io::Result<()> {
    if let Some(data) = pooled_read(reader, 8192).await? {
        println!("Read {} bytes", data.len());
    } else {
        println!("Reached EOF");
    }
    Ok(())
}
```

### Copying from an async reader to an async writer:

```rust
use async_io_bufpool::pooled_copy;
use futures_util::AsyncWriteExt;

async fn example_copy<R, W>(reader: R, writer: W) -> std::io::Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let bytes_copied = pooled_copy(reader, writer).await?;
    println!("Copied {} bytes", bytes_copied);
    Ok(())
}
```
