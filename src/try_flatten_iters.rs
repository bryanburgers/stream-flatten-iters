use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

impl<S: ?Sized + TryStream> TryStreamExt for S {}

/// An extension trait for Streams that provides a variety of convenient combinator functions.
pub trait TryStreamExt: TryStream {
    /// Flattens a stream of iterators into one continuous stream.
    fn try_flatten_iters(self) -> TryFlattenIters<Self>
    where
        Self::Ok: IntoIterator,
        <Self::Ok as IntoIterator>::IntoIter: Unpin,
        Self: Sized,
    {
        TryFlattenIters::new(self)
    }
}

/// Stream for the [`try_flatten`](super::TryStreamExt::try_flatten) method.
#[pin_project]
// #[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryFlattenIters<St>
where
    St: TryStream,
    St::Ok: IntoIterator,
{
    #[pin]
    stream: St,
    #[pin]
    next: Option<<St::Ok as IntoIterator>::IntoIter>,
}

impl<St> TryFlattenIters<St>
where
    St: TryStream,
    St::Ok: IntoIterator,
    <St::Ok as IntoIterator>::IntoIter: Unpin,
{
    pub(crate) fn new(stream: St) -> Self {
        Self { stream, next: None }
    }

    /// Acquires a reference to the underlying sink or stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying sink or stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// sink or stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Acquires a pinned mutable reference to the underlying sink or stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// sink or stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: core::pin::Pin<&mut Self>) -> core::pin::Pin<&mut St> {
        self.project().stream
    }

    /// Consumes this combinator, returning the underlying sink or stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St> FusedStream for TryFlattenIters<St>
where
    St: TryStream + FusedStream,
    St::Ok: IntoIterator,
    <St::Ok as IntoIterator>::IntoIter: Unpin,
{
    fn is_terminated(&self) -> bool {
        self.next.is_none() && self.stream.is_terminated()
    }
}

impl<St> Stream for TryFlattenIters<St>
where
    St: TryStream,
    St::Ok: IntoIterator,
    <St::Ok as IntoIterator>::IntoIter: Unpin,
{
    type Item = Result<<St::Ok as IntoIterator>::Item, St::Error>;

    #[project]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        #[project]
        let TryFlattenIters {
            mut stream,
            mut next,
        } = self.project();
        Poll::Ready(loop {
            if let Some(mut s) = next.as_mut().as_pin_mut() {
                if let Some(item) = s.next() {
                    break Some(Ok(item));
                } else {
                    next.set(None);
                }
            } else if let Some(s) = ready!(stream.as_mut().try_poll_next(cx)?) {
                next.set(Some(s.into_iter()));
            } else {
                break None;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::TryStreamExt as _;
    use futures::stream::{iter, StreamExt, TryStreamExt};

    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    struct CustomError;

    #[tokio::test]
    async fn test_basic() {
        let input: Vec<Result<Vec<usize>, CustomError>> = vec![
            Ok(vec![0_usize, 1, 2]),
            Ok(vec![3, 4]),
            Ok(vec![]),
            Ok(vec![5, 6, 7]),
        ];
        let mut stream = iter(input).try_flatten_iters();

        assert_eq!(stream.next().await, Some(Ok(0)));
        assert_eq!(stream.next().await, Some(Ok(1)));
        assert_eq!(stream.next().await, Some(Ok(2)));
        assert_eq!(stream.next().await, Some(Ok(3)));
        assert_eq!(stream.next().await, Some(Ok(4)));
        assert_eq!(stream.next().await, Some(Ok(5)));
        assert_eq!(stream.next().await, Some(Ok(6)));
        assert_eq!(stream.next().await, Some(Ok(7)));
        assert_eq!(stream.next().await, None);
    }

    #[tokio::test]
    async fn test_error() {
        let input: Vec<Result<Vec<usize>, CustomError>> = vec![
            Ok(vec![0_usize, 1, 2]),
            Err(CustomError),
            Ok(vec![]),
            Ok(vec![5, 6, 7]),
        ];
        let mut stream = iter(input).try_flatten_iters();

        assert_eq!(stream.next().await, Some(Ok(0)));
        assert_eq!(stream.next().await, Some(Ok(1)));
        assert_eq!(stream.next().await, Some(Ok(2)));
        assert_eq!(stream.next().await, Some(Err(CustomError)));
        assert_eq!(stream.next().await, Some(Ok(5)));
        assert_eq!(stream.next().await, Some(Ok(6)));
        assert_eq!(stream.next().await, Some(Ok(7)));
        assert_eq!(stream.next().await, None);
    }

    #[tokio::test]
    async fn test_error_with_collect() {
        let input: Vec<Result<Vec<usize>, CustomError>> = vec![
            Ok(vec![0_usize, 1, 2]),
            Ok(vec![3, 4]),
            Ok(vec![]),
            Ok(vec![5, 6, 7]),
        ];
        let result: Result<Vec<usize>, CustomError> =
            iter(input).try_flatten_iters().try_collect().await;
        assert_eq!(result, Ok(vec![0, 1, 2, 3, 4, 5, 6, 7]));

        let input: Vec<Result<Vec<usize>, CustomError>> = vec![
            Ok(vec![0_usize, 1, 2]),
            Err(CustomError),
            Ok(vec![]),
            Ok(vec![5, 6, 7]),
        ];
        let result: Result<Vec<usize>, CustomError> =
            iter(input).try_flatten_iters().try_collect().await;
        assert_eq!(result, Err(CustomError));
    }

    #[tokio::test]
    async fn test_empty() {
        let mut stream = iter(Vec::<Result<Vec<String>, CustomError>>::new()).try_flatten_iters();

        assert_eq!(stream.next().await, None);
    }
}
