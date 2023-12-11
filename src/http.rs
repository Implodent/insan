use async_std::net::TcpStream;
use insan_http::{
    server::{ConnectionStatus, Server as HttpServer},
    types::{Method, StatusCode},
    Read, Write,
};
pub use insan_http::{
    types as http_types,
    types::{Request, Response},
};

use crate::{Handler, Service};

pub mod client;
pub mod server;

pub trait ResponseError: std::fmt::Display {
    fn status_code(&self) -> StatusCode;
    fn to_response(&self) -> Response {
        let mut response = Response::new(self.status_code());
        response.set_body(self.to_string());
        response
    }
}

impl ResponseError for http_types::Error {
    fn status_code(&self) -> StatusCode {
        self.status()
    }
}
