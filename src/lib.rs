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
//! [`StreamExt::flatten`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.flatten
//! [`Iterator::flatten`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flatten
//! [`StreamExt::buffered`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.buffered

#![deny(missing_docs)]

use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_utils::unsafe_pinned;

impl<T: ?Sized> StreamExt for T where T: Stream {}

/// An extension trait for Streams that provides a variety of convenient combinator functions.
pub trait StreamExt: Stream {
    /// Flattens a stream of iterators into one continuous stream.
    fn flatten_iters(self) -> FlattenIters<Self>
    where
        Self::Item: IntoIterator,
        Self: Sized,
    {
        FlattenIters::new(self)
    }
}

/// Stream for the [`flatten_iters`](StreamExt::flatten_iters) method.
// #[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct FlattenIters<St>
where
    St: Stream,
    St::Item: IntoIterator,
{
    stream: St,
    next: Option<<St::Item as IntoIterator>::IntoIter>,
}

impl<St> Unpin for FlattenIters<St>
where
    St: Stream + Unpin,
    St::Item: IntoIterator,
    <St::Item as IntoIterator>::IntoIter: Unpin,
{
}

impl<St> FlattenIters<St>
where
    St: Stream,
    St::Item: IntoIterator,
{
    unsafe_pinned!(stream: St);
    unsafe_pinned!(next: Option<<St::Item as IntoIterator>::IntoIter>);
}

impl<St> FlattenIters<St>
where
    St: Stream,
    St::Item: IntoIterator,
{
    pub(crate) fn new(stream: St) -> Self {
        Self { stream, next: None }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.stream()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St> FusedStream for FlattenIters<St>
where
    St: FusedStream,
    St::Item: IntoIterator,
    <St::Item as IntoIterator>::IntoIter: Unpin,
{
    fn is_terminated(&self) -> bool {
        self.next.is_none() && self.stream.is_terminated()
    }
}

impl<St> Stream for FlattenIters<St>
where
    St: Stream,
    St::Item: IntoIterator,
    <St::Item as IntoIterator>::IntoIter: Unpin,
{
    type Item = <St::Item as IntoIterator>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.next.is_none() {
                match ready!(self.as_mut().stream().poll_next(cx)) {
                    Some(e) => self.as_mut().next().set(Some(e.into_iter())),
                    None => return Poll::Ready(None),
                }
            }

            if let Some(item) = Option::as_mut(&mut self.as_mut().next()).unwrap().next() {
                return Poll::Ready(Some(item));
            } else {
                self.as_mut().next().set(None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StreamExt as _;
    use futures::stream::{iter, StreamExt};

    #[tokio::test]
    async fn test_basic() {
        let mut stream =
            iter(vec![vec![0_usize, 1, 2], vec![3, 4], vec![], vec![5, 6, 7]]).flatten_iters();

        assert_eq!(stream.next().await, Some(0));
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, Some(2));
        assert_eq!(stream.next().await, Some(3));
        assert_eq!(stream.next().await, Some(4));
        assert_eq!(stream.next().await, Some(5));
        assert_eq!(stream.next().await, Some(6));
        assert_eq!(stream.next().await, Some(7));
        assert_eq!(stream.next().await, None);
    }

    #[tokio::test]
    async fn test_empty() {
        let mut stream = iter(Vec::<Vec<String>>::new()).flatten_iters();

        assert_eq!(stream.next().await, None);
    }
}
