#![feature(async_fn_in_trait)]
use std::{error::Error, str::FromStr};

use async_std::net::TcpListener;
use acril::{
    http::{http_types, server::*, *},
    *,
};
use acril_http::types::{Method, Mime, StatusCode};
use tracing::*;

struct HelloService;

impl Service for HelloService {
    type Error = http_types::Error;
    type Context = HttpContext<Self>;

    async fn started(&mut self, _cx: &Self::Context) -> Result<(), Self::Error> {
        info!("LIFTOFF BABYYY");
        Ok(())
    }

    async fn stopping(&mut self, _cx: &Self::Context) -> Result<(), Self::Error> {
        info!("Served requests, shutting down ;)");
        Ok(())
    }
}

impl Handler<Request, Response> for HelloService {
    async fn call(
        &mut self,
        mut req: Request,
        _cx: &mut Self::Context,
    ) -> Result<Response, Self::Error> {
        let mut resp = Response::new(StatusCode::Ok);
        debug!("Serving request {req:?}");
        match (req.method(), req.url().path()) {
            (Method::Get, "/") => {
                resp.set_content_type(Mime::from_str("text/html").unwrap());
                resp.set_body("<!doctype html><html><head><title>THIS FUCKING WORKS YEEEAHH</title></head><body><h1>GG</h1><h2>THIS FUCKING WORKS YEEEEEE</h2></body></html>");
            }
            (Method::Post, "/say_hello") => {
                resp.set_content_type(Mime::from_str("text/html").unwrap());
                resp.set_body(format!("<!doctype html><html><head><title>You said something</title></head><body><h1>You said:</h1><h2>{}</h2></body></html>", req.body_string().await?));
            }
            (method, path @ ("/" | "/say_hello")) => {
                return Err(http_types::Error::from_str(
                    StatusCode::MethodNotAllowed,
                    format!("Invalid method {method} on route {path}"),
                ))
            }
            (_, path) => {
                return Err(http_types::Error::from_str(
                    StatusCode::NotFound,
                    format!("Invalid route {path}"),
                ))
            }
        }

        Ok(resp)
    }
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    info!("Starting!");

    let lis = TcpListener::bind("0.0.0.0:3000").await?;
    loop {
        let (stream, addr) = lis.accept().await?;

        async_std::task::spawn(async move {
            if let Err(e) = Server::new(HelloService, stream).run().await {
                if let Some(std::io::ErrorKind::ConnectionReset) =
                    e.downcast_ref::<std::io::Error>().map(|x| x.kind())
                {
                    // nothing
                } else {
                    tracing::error!("while handling {addr}: {e}");
                }
            }
        });
    }
}
