use super::*;
use acril_http::types::Url;
pub use acril_macros::ClientEndpoint;

pub struct NoMiddleware;

impl Service for NoMiddleware {
    type Context = ();
    type Error = http_types::Error;
}

pub trait Middleware: Service<Context = ()> {
    async fn call(&self, request: Request) -> Result<Response, <Self as Service>::Error>;
}

impl Middleware for NoMiddleware {
    async fn call(&self, request: Request) -> Result<Response, Self::Error> {
        acril_http::client::connect(
            TcpStream::connect(format!(
                "{}:{}",
                request.url().host_str().ok_or_else(|| {
                    http_types::Error::from_str(
                        StatusCode::UnprocessableEntity,
                        "No host in request URL",
                    )
                })?,
                request.url().port_or_known_default().ok_or_else(|| {
                    http_types::Error::from_str(
                        StatusCode::UnprocessableEntity,
                        "No port in request URL",
                    )
                })?
            ))
            .await?,
            request,
        )
        .await
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct HttpClient<M = NoMiddleware> {
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

    pub async fn call<E: Service<Context = Self> + ClientEndpoint>(
        &self,
        endpoint: E,
    ) -> Result<E::Output, E::Error> {
        endpoint.run(self).await
    }

    pub async fn execute(&self, request: Request) -> Result<Response, M::Error> {
        self.middleware.call(request).await
    }
}

impl HttpClient<NoMiddleware> {
    pub fn new() -> Self {
        Self {
            middleware: NoMiddleware,
            base_url: None,
        }
    }
}

pub trait HttpClientContext {
    type Error;

    fn new_request(&self, method: Method, url: &str) -> Request;
    async fn run_request(&self, request: Request) -> Result<Response, Self::Error>;
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

    async fn run_request(&self, request: Request) -> Result<Response, M::Error> {
        self.execute(request).await
    }
}

pub trait ClientEndpoint: Service
where
    Self::Context: HttpClientContext,
{
    type Output;

    async fn run(&self, context: &Self::Context) -> Result<Self::Output, Self::Error>;
}
