
# async-await-test

Toying around with tokio 0.1, futures 0.3-alpha, and async/wait.

- `src/asyncstream.rs` gives you a way of `yield`ing items to a stream.
- `src/main.rs` uses that, and async/await, and `tokio_threadpool::blocking`
  to implement a very basic fileserver that is async and non-blocking.

## Docs used

For futures 0.3, 0.1 compat, tokio, async/await:

- [Futures 0.1 Compatibility Layer blog post (WIP)](https://github.com/rust-lang-nursery/futures-rs/blob/26dec35971e378f0b8702304b3941e3cdfcb69b8/_posts/2018-12-04-compatibility-layer.md)
- [Futures 0.3-alpha futures::compat docs](https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.12/futures/compat/index.html)
- [Rust: Getting started with nightly async/await support blog post](https://jsdw.me/posts/rust-asyncawait-preview/)
- [rust-lang async/await RFC](https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md)

For working with `tokio_threadpool::blocking`:

- [Function tokio_threadpool::blocking](https://docs.rs/tokio-threadpool/0.1.11/tokio_threadpool/fn.blocking.html)
- [Function futures::future::poll_fn](https://docs.rs/futures/0.1.25/futures/future/fn.poll_fn.html)
- [A blocking aware version of poll_fn for use with tokio](https://www.reddit.com/r/rust/comments/am0nef/a_blocking_aware_version_of_poll_fn_for_use_with/)

