#![doc = include_str!("../README.md")]
#![allow(async_fn_in_trait, incomplete_features)]
#![feature(associated_type_bounds, return_type_notation)]

#[cfg(feature = "macros")]
#[doc(hidden)]
pub use serde_urlencoded;
#[doc(hidden)]
pub use std::future::Future;

/// This trait includes the most important method of this library - [`call`](Handler::call).
///
/// It allows actors to handle messages and return responses ([`Response`](Handler::Response)).
pub trait Handler<Request>: Service {
    /// The response of this handler.
    type Response;

    /// Handle `Request`.
    async fn call(
        &mut self,
        request: Request,
        cx: &mut Self::Context,
    ) -> Result<Self::Response, Self::Error>;
}

/// This trait is a marker trait for any actor, setting the context and error types.
pub trait Service {
    type Context;
    type Error;

    async fn started(&mut self, _cx: &mut Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn stopping(&mut self, _cx: &mut Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// A layer is some type which returns a service ([`Layer::Service`]) which wraps the given service (`S`)
pub trait Layer<S> {
    /// The service that wraps `S`.
    type Service;

    /// Wrap `S`.
    fn wrap(&self, inner: S) -> Self::Service;
}

/// Stack the layer `B` on top of `A`.
pub struct Stack<A, B>(A, B);

impl<S, A: Layer<S>, B: Layer<A::Service>> Layer<S> for Stack<A, B> {
    type Service = B::Service;

    fn wrap(&self, inner: S) -> Self::Service {
        self.1.wrap(self.0.wrap(inner))
    }
}

/// A layer which returns the given service as-is.
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identity;

impl<S> Layer<S> for Identity {
    type Service = S;
    fn wrap(&self, inner: S) -> Self::Service {
        inner
    }
}

/// A builder for services, allowing users to declaratively construct a service by adding
/// [`Layer`]s on top.
pub struct Builder<L>(L);

impl Builder<Identity> {
    /// Create an empty builder.
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

/// `use acril::prelude::*;` to import commonly used types and traits.
pub mod prelude {
    #[cfg(feature = "macros")]
    #[doc(hidden)]
    pub use serde_urlencoded;
    pub use crate::{Handler, Service};
    #[cfg(feature = "http")]
    pub mod http {
        pub use super::*;
        pub use crate::http::{
            client::*,
            http_types::{self, Method, Mime, Request, Response, StatusCode, Url},
            *,
        };
    }
}
