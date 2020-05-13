//! `flatten_iters` flattens a stream of iterators into one continuous stream.
//!
//! This is useful when you have a producer that is paging through a resource (like a REST endpoint
//! with pages or a next URL, an ElasticSearch query with a scroll parameter, etc.)
//!
//! This code is taken *almost* verbatim from [`StreamExt::flatten`] and is similar
//! in spirit to [`Iterator::flatten`].
//!
//! ```
//! use stream_flatten_iters::StreamExt as _;
//! use futures::stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let (mut tx, mut rx) = tokio::sync::mpsc::channel(3);
//!
//!     tokio::spawn(async move {
//!         tx.send(vec![0, 1, 2, 3]).await.unwrap();
//!         tx.send(vec![4, 5, 6]).await.unwrap();
//!         tx.send(vec![7, 8, 9]).await.unwrap();
//!     });
//!
//!     let mut stream = rx.flatten_iters();
//!
//!     while let Some(res) = stream.next().await {
//!         println!("got = {}", res);
//!     }
//! }
//!
//! // Output:
//! // got = 0
//! // got = 1
//! // got = 2
//! // got = 3
//! // got = 4
//! // got = 5
//! // got = 6
//! // got = 7
//! // got = 8
//! // got = 9
//! ```
//!
//! This is especially useful when combined with [`StreamExt::buffered`] to keep a buffer of promises going
//! throughout a long promise.
//!
//! ```
//! use stream_flatten_iters::StreamExt as _;
//! use futures::stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (mut tx, mut rx) = tokio::sync::mpsc::channel(3);
//!
//!     tokio::spawn(async move {
//!         for i in 0_usize..100 {
//!             let start = i * 10;
//!             let end = start + 10;
//!             tx.send(start..end).await.unwrap();
//!         }
//!     });
//!
//!     let mut stream = rx.flatten_iters().map(|i| long_process(i)).buffered(10);
//!
//!     let mut total = 0_usize;
//!     while let Some(res) = stream.next().await {
//!         let _ = res?;
//!         total += 1;
//!         println!("Completed {} tasks", total);
//!     }
//!
//!     Ok(())
//! }
//!
//! async fn long_process(i: usize) -> Result<(), Box<dyn std::error::Error>> {
//!     // Do something that takes a long time
//!     Ok(())
//! }
//! ```
//!
//! Similarly, `try_flatten_iters` is analogous to [`TryStreamExt::try_flatten`], and is useful for
//! flatting results of iterators.
//!
//! ```
//! use stream_flatten_iters::TryStreamExt as _;
//! use futures::stream::{StreamExt, TryStreamExt};
//!
//! #[tokio::main]
//! async fn main() {
//!     let (mut tx, mut rx) = tokio::sync::mpsc::channel(3);
//!
//!     tokio::spawn(async move {
//!         tx.send(Ok(vec![0_usize, 1, 2, 3])).await.unwrap();
//!         tx.send(Err(())).await.unwrap();
//!         tx.send(Ok(vec![4, 5, 6])).await.unwrap();
//!         tx.send(Ok(vec![7, 8, 9])).await.unwrap();
//!     });
//!
//!     let mut stream = rx.try_flatten_iters();
//!
//!     while let Some(res) = stream.next().await {
//!         println!("got = {:?}", res);
//!     }
//! }
//!
//! // Output:
//! // got = Ok(0)
//! // got = Ok(1)
//! // got = Ok(2)
//! // got = Ok(3)
//! // got = Err(())
//! // got = Ok(4)
//! // got = Ok(5)
//! // got = Ok(6)
//! // got = Ok(7)
//! // got = Ok(8)
//! // got = Ok(9)
//! ```
//!
//! [`StreamExt::flatten`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.flatten
//! [`TryStreamExt::try_flatten`]: https://docs.rs/futures/0.3/futures/stream/trait.TryStreamExt.html#method.try_flatten
//! [`Iterator::flatten`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flatten
//! [`StreamExt::buffered`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.buffered

#![deny(missing_docs)]

mod flatten_iters;
mod try_flatten_iters;

pub use flatten_iters::{FlattenIters, StreamExt};
pub use try_flatten_iters::{TryFlattenIters, TryStreamExt};
