pub use http_types::{self, Method, Request, Response, StatusCode};

use crate::Service;

pub mod client;
#[cfg(not(target_arch = "wasm32"))]
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
