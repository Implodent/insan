#![allow(async_fn_in_trait, incomplete_features)]
#![feature(associated_type_bounds, return_type_notation)]

pub use serde_urlencoded;
pub use std::future::Future;

pub trait Handler<Request>: Service {
    type Response;

    async fn call(
        &mut self,
        request: Request,
        cx: &mut Self::Context,
    ) -> Result<Self::Response, Self::Error>;
}

pub trait Service {
    type Context;
    type Error;

    async fn started(&mut self, _cx: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn stopping(&mut self, _cx: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub trait Layer<S> {
    type Service;

    fn wrap(&self, inner: S) -> Self::Service;
}

pub struct Stack<A, B>(A, B);

impl<S, A: Layer<S>, B: Layer<A::Service>> Layer<S> for Stack<A, B> {
    type Service = B::Service;

    fn wrap(&self, inner: S) -> Self::Service {
        self.1.wrap(self.0.wrap(inner))
    }
}

pub struct Identity;

impl<S> Layer<S> for Identity {
    type Service = S;
    fn wrap(&self, inner: S) -> Self::Service {
        inner
    }
}

pub struct Builder<L>(L);

impl Builder<Identity> {
    pub fn new() -> Self {
        Self(Identity)
    }
}

impl<L> Builder<L> {
    pub fn into_inner(self) -> L {
        self.0
    }

    pub fn layer<T>(self, layer: T) -> Builder<Stack<T, L>> {
        Builder(Stack(layer, self.0))
    }

    pub fn service<S>(&self, service: S) -> L::Service
    where
        L: Layer<S>,
    {
        self.0.wrap(service)
    }
}

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "websocket")]
pub mod websocket;

pub mod prelude {
    pub use crate::{serde_urlencoded, Handler, Service};
    #[cfg(feature = "http")]
    pub mod http {
        pub use super::*;
        #[cfg(not(target_arch = "wasm32"))]
        pub use crate::http::server::*;
        pub use crate::http::{
            client::*,
            http_types::{self, Method, Mime, Request, Response, StatusCode, Url},
            *,
        };
    }
}
