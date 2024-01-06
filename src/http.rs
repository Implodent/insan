pub use http_types::{self, Method, Request, Response, StatusCode};

use crate::Service;

pub mod client;
pub use acril_macros::endpoint_error;
