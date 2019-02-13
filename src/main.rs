#![feature(async_await, await_macro, futures_api)]

use std::fs;
use std::io::Read;

use futures::Future;
use futures03::compat::Future01CompatExt;

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
type ReadFileResponse = Response<AsyncStream<Vec<u8>, std::io::Error>>;

/// This is our service handler.
fn read_file(req: Request<Body>) -> impl Future<Item = ReadFileResponse, Error = hyper::Error> + Send {
    let mut response = Response::builder();
    response.status(StatusCode::OK);

    let body_stream = AsyncStream::new(async move |mut yield_item| {
        let mut file = await!(blocking_io!{
            //fs::File::open("/usr/share/dict/dutch")
            fs::File::open("/home/mikevs/VirtualBox VMs/ISO/debian-buster-DI-alpha5-amd64-netinst.iso")
        })?;
        let mut buffer = [0u8; 65536];
        loop {
            let n = await!(blocking_io!{ file.read(&mut buffer[..]) })?;
            if n == 0 {
                break;
            }
            await!(yield_item(buffer[0..n].to_vec()));
        }
		Ok(())
    });

    futures::future::ok(response.body(body_stream).unwrap())
}

fn main() {
    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn(read_file))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);
}

