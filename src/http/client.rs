use crate::Handler;

use super::*;
pub use acril_macros::{with_builder, ClientEndpoint};
use http_types::Url;

pub struct DefaultMiddleware;

impl Service for DefaultMiddleware {
    type Context = ();
    type Error = http_types::Error;
}

impl Handler<Request> for DefaultMiddleware {
    type Response = Response;

    async fn call(
        &mut self,
        request: Request,
        _cx: &mut Self::Context,
    ) -> Result<Self::Response, Self::Error> {
        acril_http::client::connect(request).await
    }
}

pub trait Middleware: Handler<Request, Response = Response, Context = ()> {}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct HttpClient<M = DefaultMiddleware> {
    middleware: M,
    base_url: Option<Url>,
}

impl<M: Middleware> HttpClient<M> {
    pub fn new_with(middleware: M) -> Self {
        Self {
            middleware,
            base_url: None,
        }
    }

    pub fn with_base_url(mut self, base_url: Url) -> Self {
        self.base_url = Some(base_url);
        self
    }

    pub fn get_middleware(&self) -> &M {
        &self.middleware
    }
    pub fn base_url(&self) -> Option<&Url> {
        self.base_url.as_ref()
    }

    pub async fn call<E: Service<Context = Self> + ClientEndpoint>(
        &mut self,
        endpoint: E,
    ) -> Result<E::Output, E::Error> {
        endpoint.run(self).await
    }

    pub async fn execute(&mut self, request: Request) -> Result<Response, M::Error> {
        self.middleware.call(request, &mut ()).await
    }
}

impl HttpClient<DefaultMiddleware> {
    pub fn new() -> Self {
        Self {
            middleware: DefaultMiddleware,
            base_url: None,
        }
    }
}

pub trait HttpClientContext {
    type Error;

    fn new_request(&self, method: Method, url: &str) -> Request;
    async fn run_request(&mut self, request: Request) -> Result<Response, Self::Error>;
}

impl<M: Middleware> HttpClientContext for HttpClient<M> {
    type Error = <M as Service>::Error;

    fn new_request(&self, method: Method, url: &str) -> Request {
        Request::new(
            method,
            if let Some(base) = self.base_url.as_ref() {
                Url::options()
                    .base_url(Some(base))
                    .parse(url)
                    .expect("errors reparsing a perfectly good url")
            } else {
                Url::parse(url).unwrap()
            },
        )
    }

    async fn run_request(&mut self, request: Request) -> Result<Response, M::Error> {
        self.execute(request).await
    }
}

pub trait ClientEndpoint: Service {
    type Output;

    async fn run(&self, context: &mut Self::Context) -> Result<Self::Output, Self::Error>;
}
