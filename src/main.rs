//! async-await-test
//!
//! This code implements a very basic HTTP fileserver, using
//! tokio 0.1, futures 0.3-alpha, and async/await.
//!
//! We use the 0.1 compatibility methods from futures@0.3 to
//! map the new futures from/to the old ones that tokio uses.
//!
#![feature(async_await, await_macro, futures_api)]

use std::fs;
use std::io;
use std::io::Read;

// Explicitly import stuff from futures@0.1 and futures@03
// with a 01 or 03 suffix so they are distinctive.
use futures::Future as Future01;
use futures03::channel::mpsc::channel as mpsc_channel03;

// futures@0.3 have a bunch of functionality in "extension traits"
// that need to be imported seperately.
use futures03::stream::TryStreamExt;
use futures03::sink::SinkExt;
use futures03::future::{FutureExt, TryFutureExt};
use futures03::compat::Future01CompatExt;

use hyper::{Body, Request, Response, Server};

use tokio_threadpool;
use tokio;

// This macro turns a blocking closure into a future, and then await!s
// on it. It makes using blocking code in async blocks/functions
// reasonably ergonomic. Just call:
//
// let result = blocking_io!({ foo_io(); return wait_bar(); });
//
macro_rules! blocking_io {
    ($expression:expr) => (
        await!(futures::future::poll_fn(|| {
            tokio_threadpool::blocking(|| -> std::io::Result<_> {
                $expression
            })
            .map_err(|_| panic!("the threadpool shut down"))
        }).then(|r| r.unwrap()).compat())
    )
}

// This is the request handler. It returns a Response future. When that
// future resolves into an actual Response, the Hyper library will write
// the response status and headers. It will then send out the response
// stream as the body.
fn handler(req: Request<Body>) -> impl Future01<Item = Response<hyper::Body>, Error = http::Error> + Send
{
    // An async block returns a 0.3 Future. Refer to it as `fut`.
    let fut = async move {
        let mut response = Response::builder();

        // Open the file and check if it's a regular file. If that fails,
        // return an Err(io::Error).
        //
        // Note the use of the question mark operator.
        //
        let path = req.uri().path();
        let mut file = blocking_io!({
            let f = fs::File::open(path)?;
            if f.metadata()?.is_dir() {
                return Err(io::Error::new(io::ErrorKind::Other, "is a directory"));
            }
            Ok(f)
        })?;

        // Create a futures 0.3 channel.
        // We will return the `rx` side as the body stream in the response.
        let (mut tx, rx) = mpsc_channel03::<Result<hyper::Chunk, io::Error>>(1);

        // The `tx` side of the channel is passed into this async closure,
        // which we will spawn as a task on the tokio runtime. It will read
        // the file we opened and write its contents to the channel.
        //
        // This future returns a Result<(), io::Error>. That result
        // is in this implementation not used for anything, except to bail
        // out of the task on errors. Note again the use of the ? operator.
        //
        let file_task = async move {
            let mut buffer = [0u8; 65536];
            loop {
                let n = blocking_io!(file.read(&mut buffer[..]))?;
                if n == 0 {
                    break;
                }
                let chunk = hyper::Chunk::from(Vec::from(&buffer[0..n]));
                await!(tx.send(Ok(chunk)))
                       .map_err(|_| io::ErrorKind::UnexpectedEof)?;
            }

            // We have to hint at the return type here.
            Ok::<_, io::Error>(())
        };

        // spawn the task onto the tokio runtime, using a .compat() adapter to
        // transform the 0.3 Future to a 0.1 Future that tokio understands.
        tokio::spawn(file_task.map_err(|_| ()).boxed().compat());

        // as Hyper is also still based on 0.1 Futures and Streams, we need
        // to convert the 0.3 Stream to a 0.1 Stream that we can
        // returns as the response body with a similar .compat() adapter.
        let body_stream01 = Box::new(rx.compat());
        let hyper_body = hyper::Body::wrap_stream(body_stream01);

        // we now have a response, with a streaming body.
        response
            .body(hyper_body)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    };

    // The async block below first resolves the `fut` future we got from
    // the async block above, then it maps any io::Error's to HTTP
    // responses so that the client gets a correct error response,
    // instead of a 500 Internal Server Error or an empty reply.
    let res = async move {
        match await!(fut) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let (code, msg) = match e.kind() {
                    io::ErrorKind::NotFound         => (404, "Not Found"),
                    io::ErrorKind::PermissionDenied => (403, "Permission Denied"),
                    _                               => (500, "Something went wrong"),
                };
                let error_msg = format!("<h2>{} {}</h2>\n", code, msg);
                Response::builder().status(code).body(hyper::Body::from(error_msg))
            }
        }
    };

    // Now `res` comes from an `async` block, so it is a 0.3 Future.
    // Convert it to a 0.1 Future and return it.
    return res.boxed().compat();
}

fn main() {

    // address to listen on.
    let addr = ([127, 0, 0, 1], 3000).into();

    // this is a Hyper construct that makes sure that incoming
    // requests get served by the `handler` function.
    let make_service = move || {
        hyper::service::service_fn(move |req| {
            handler(req)
        })
    };

    // create the server.
    let server = Server::bind(&addr)
        .serve(make_service)
        .map_err(|e| eprintln!("server error: {}", e));

    // and starts it.
    println!("Listening on http://{}, serving '/'", addr);
    hyper::rt::run(server);
}

