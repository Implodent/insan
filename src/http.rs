use async_std::net::TcpStream;
use acril_http::{
    server::{ConnectionStatus, Server as HttpServer},
    types::{Method, StatusCode},
    Read, Write,
};
pub use acril_http::{
    types as http_types,
    types::{Request, Response},
};

use crate::{Handler, Service};

pub mod client;
pub mod server;

pub use acril_macros::endpoint_error;

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
