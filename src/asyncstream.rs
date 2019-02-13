use std::cell::Cell;
use std::pin::Pin;
use std::rc::Rc;

use futures::{Async, Future, Stream};

use futures03;
use futures03::task::{ Poll as Poll03, LocalWaker};
use futures03::future::FutureExt as FutureExt03;
use futures03::future::Future as Future03;
use futures03::compat::Compat as Compat0301;

use bytes;
use hyper;

/// Future returned by the yield function. All it does is to return
/// the "Pending" state once. The next time it is polled it will
/// return "Ready".
pub struct OncePending {
    state:  bool,
}

impl Future03 for OncePending {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _lw: &LocalWaker) -> Poll03<Self::Output> {
        if self.state {
            Poll03::Ready(())
        } else {
            self.state = true;
            Poll03::Pending
        }
    }
}

// Only internally used by one AsyncStream and never shared
// in any other way, so we don't have to use Arc<Mutex<..>>.
struct InternalItem<I>(Rc<Cell<Option<I>>>);
unsafe impl<I> Sync for InternalItem<I> {}
unsafe impl<I> Send for InternalItem<I> {}

/// An AsyncStream is an abstraction around a future, where the
/// future can internally loop and yield items.
///
/// For now it only accepts Future@0.3 and implements Stream@0.1,
/// because it's main use-case is to generate a body stream for
/// a hyper service function.
pub struct AsyncStream<Item, Error> {
    item:   InternalItem<Item>,
    fut:    Box<Future<Item=(), Error=Error> + 'static + Send>,
}

impl<Item, Error> AsyncStream<Item, Error> {
    /// Create a new stream from a closure returning a Future 0.3,
    /// or an "async closure" (which is the same).
    ///
    /// The closure is passed one argument, the "yielder", which is
    /// a function that can be called to send a item to the stream.
    pub fn new<F, R>(mut f: F) -> Self
        where F: FnMut(Box<FnMut(Item) -> OncePending + Send>) -> R,
              R: Future03<Output=Result<(), Error>> + Send + 'static,
              Item: 'static,
              Error: 'static,
    {
        let item = InternalItem(Rc::new(Cell::new(None)));
        let item2 = InternalItem(item.0.clone());
        let yielder = Box::new(move |yield_item| {
            item.0.set(Some(yield_item));
            OncePending{ state: false }
        });
        AsyncStream::<Item, Error> {
            item:   item2,
            fut:    Box::new(Compat0301::new(f(yielder).boxed())),
        }
    }
}

/// Stream implementation for Futures 0.1.
impl<I, E> Stream for AsyncStream<I, E> {
    type Item = I;
    type Error = E;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        match self.fut.poll() {
            // If the future returned Async::Ready, that signals the end of the stream.
            Ok(Async::Ready(_)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => {
                // Async::NotReady means that there might be new item.
                let mut item = self.item.0.replace(None);
                if item.is_none() {
                    Ok(Async::NotReady)
                } else {
                    Ok(Async::Ready(item.take()))
                }
            },
            Err(e) => Err(e),
        }
    }
}

/// hyper::body::Payload trait implementation.
///
/// This implementation allows you to use anything that implements
/// IntoBuf as a Payload item.
impl<Item, Error> hyper::body::Payload for AsyncStream<Item, Error>
    where Item: bytes::buf::IntoBuf + Send + Sync + 'static,
          Item::Buf: Send,
          Error: std::error::Error + Send + Sync + 'static,
{
    type Data = Item::Buf;
    type Error = Error;

    fn poll_data(&mut self) -> futures::Poll<Option<Self::Data>, Self::Error> {
        match self.poll() {
            Ok(Async::Ready(Some(item))) => Ok(Async::Ready(Some(item.into_buf()))),
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}
