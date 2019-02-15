#![feature(async_await, await_macro, futures_api)]

use std::fs;
use std::io;
use std::io::Read;

use futures::prelude::*;
use futures03::compat::Future01CompatExt;
use futures03::future::{FutureExt, TryFutureExt};

use bytes::Bytes;
use hyper::service::service_fn;
use hyper::{Body, Request, Response, Server, StatusCode};

mod asyncstream;
use asyncstream::AsyncStream;

macro_rules! blocking_io {
    ($expression:expr) => (
		futures::future::poll_fn(|| {
            tokio_threadpool::blocking(|| -> std::io::Result<_> {
                $expression
            })
            .map_err(|_| panic!("the threadpool shut down"))
        }).then(|r| r.unwrap()).compat()
    )
}

#[allow(dead_code)]
type RespData = AsyncStream<Bytes, io::Error>;

/// This is our service handler.
fn handler(req: Request<Body>) -> impl Future<Item = Response<RespData>, Error = http::Error> + Send
{

    let res = async move {
        let mut response = Response::builder();

        let path = req.uri().path();
        let mut file = await!(blocking_io!({
            let f = fs::File::open(path)?;
            if f.metadata()?.is_dir() {
                return Err(io::Error::new(io::ErrorKind::Other, "is a directory"));
            }
            Ok(f)
        }))?;

        response.status(StatusCode::OK);

        let body_stream = AsyncStream::stream(async move |mut yield_item| {
            let mut buffer = [0u8; 65536];
            loop {
                let n = await!(blocking_io!{ file.read(&mut buffer[..]) })?;
                if n == 0 {
                    break;
                }
                await!(yield_item(buffer[0..n].into()));
            }
            Ok(())
        });

        response.body(body_stream).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    };

    // now map any errors to a HTTP response.
    async move {
        use io::ErrorKind::*;
        match await!(res) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let (code, msg) = match e.kind() {
                    NotFound => (404, "Not Found"),
                    PermissionDenied => (403, "Permission Denied"),
                    _ => (500, "Something went wrong"),
                };
                let body = AsyncStream::oneshot(format!("<h2>{} {}</h2>\n", code, msg));
                Response::builder().status(code).body(body)
            }
        }
    }.boxed().compat()
}

fn main() {
    env_logger::init();
    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn(handler))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);
}

