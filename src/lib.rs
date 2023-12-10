#![cfg_attr(any(not(feature = "beta"), feature = "nightly"), feature(return_position_impl_trait_in_trait))]
pub use std::future::Future;

pub trait Handler<Request, Response>: Service {
    fn call(&mut self, request: Request, cx: &mut Self::Context) -> impl Future<Output = Result<Response, Self::Error>>;
}

pub trait Service {
    type Context;
    type Error;

    fn started(&mut self, _cx: &mut Self::Context) -> impl Future<Output = Result<(), Self::Error>> {
        async move { Ok(()) }
    }
    fn stopping(&mut self, _cx: &mut Self::Context) -> impl Future<Output = Result<(), Self::Error>> {
        async move { Ok(()) }
    }
}

#[cfg(feature = "http")]
pub mod http;
