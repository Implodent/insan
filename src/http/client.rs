use super::*;

pub struct NoMiddleware;

impl Service for NoMiddleware {
    type Context = ();
    type Error = http_types::Error;
}

pub trait Middleware: Service<Context = (), Error = http_types::Error> {
    async fn call(&self, request: Request) -> Result<Response, Self::Error>;
}

impl Middleware for NoMiddleware {
    async fn call(&self, request: Request) -> Result<Response, Self::Error> {
        insan_http::client::connect(
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
}

impl<M: Middleware> HttpClient<M> {
    pub fn new_with(middleware: M) -> Self {
        Self { middleware }
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
        }
    }
}

pub trait HttpClientContext {
    fn new_request(&self, method: Method, url: http_types::Url) -> Request;

    async fn run_request(&self, request: Request) -> Result<Response, http_types::Error>;
}

impl<M: Middleware> HttpClientContext for HttpClient<M> {
    fn new_request(&self, method: Method, url: http_types::Url) -> Request {
        Request::new(method, url)
    }
    async fn run_request(&self, request: Request) -> Result<Response, http_types::Error> {
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
