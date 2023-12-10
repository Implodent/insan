use insan_http::{
    server::{ConnectionStatus, Server as HttpServer},
    Read, Write,
};
pub use insan_http::{types as http_types, types::{Request, Response}};

use crate::{Handler, Service};

pub struct Route<S> {
    route: String,
    service: S,
}

#[derive(Debug)]
pub enum RouteError<E> {
    Service(E),
    DidNotMatch(String),
}

impl<E> From<E> for RouteError<E> {
    fn from(value: E) -> Self {
        Self::Service(value)
    }
}

impl<S: Service<Context = HttpContext>> Service for Route<S> {
    type Error = RouteError<S::Error>;
    type Context = HttpContext;

    fn started(
        &mut self,
        cx: &mut Self::Context,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> {
        async move { Ok(self.service.started(cx).await?) }
    }
    fn stopping(
        &mut self,
        cx: &mut Self::Context,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> {
        async move { Ok(self.service.stopping(cx).await?) }
    }
}

impl<S: Handler<Request, Response, Context = HttpContext>> Handler<Request, Response> for Route<S> {
    fn call(
        &mut self,
        request: Request,
        cx: &mut Self::Context,
    ) -> impl std::future::Future<Output = Result<Response, Self::Error>> {
        async move {
            if request.url().path() == self.route {
                Ok(self.service.call(request, cx).await?)
            } else {
                Err(RouteError::DidNotMatch(request.url().path().to_owned()))
            }
        }
    }
}

pub struct Server<H, RW> {
    root: H,
    server: HttpServer<RW>,
}

pub struct HttpContext {}
impl HttpContext {
    fn make() -> Self {
        Self {}
    }
}

impl<H: Service<Context = HttpContext> + Handler<Request, Response>, RW> Server<H, RW>
where
    RW: Read + Write + Clone + Send + Sync + Unpin + 'static,
    H::Error: From<std::io::Error> + From<insan_http::types::Error>,
{
    pub fn new(root: H, io: RW) -> Self {
        Self {
            root,
            server: HttpServer::new(io),
        }
    }

    pub async fn run(&mut self) -> Result<(), H::Error> {
        let mut cx = HttpContext::make();

        self.root.started(&mut cx).await?;

        while let ConnectionStatus::KeepAlive = self
            .server
            .accept_one(|req| self.root.call(req, &mut cx))
            .await?
        {}

        self.root.stopping(&mut cx).await?;

        Ok(())
    }
}
