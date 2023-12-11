#![cfg_attr(
    any(not(feature = "beta"), feature = "nightly"),
    feature(async_fn_in_trait)
)]
pub use std::future::Future;
pub use serde_urlencoded;

pub trait Handler<Request, Response>: Service {
    async fn call(
        &mut self,
        request: Request,
        cx: &mut Self::Context,
    ) -> Result<Response, Self::Error>;
}

pub trait Service {
    type Context;
    type Error;

    async fn started(&mut self, _cx: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn stopping(
        &mut self,
        _cx: &Self::Context,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(feature = "http")]
pub mod http;

pub mod prelude {
    pub use crate::{Service, Handler, serde_urlencoded};
    #[cfg(feature = "http")]
    pub mod http {
        pub use super::*;
        pub use crate::http::{*, client::*, server::*, http_types::{self, Response, Request, Method, Url, StatusCode}};
    }
}
