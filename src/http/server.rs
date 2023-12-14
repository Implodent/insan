use std::time::{Duration, SystemTime};

use crate::Handler;

use super::*;
use acril_http::{
    server::{ConnectionStatus, Server as HttpServer},
    Read, Write,
};

pub struct Server<H, RW> {
    root: H,
    server: HttpServer<RW>,
}

pub struct HttpContext<S: Service<Context = Self>> {
    stop_reason: Option<S::Error>,
}
impl<S: Service<Context = Self>> HttpContext<S> {
    fn make() -> Self {
        Self { stop_reason: None }
    }

    pub fn fatal(&mut self, error: S::Error) {
        self.stop_reason = Some(error);
    }
}

impl<H: Service<Context = HttpContext<H>> + Handler<Request, Response>, RW> Server<H, RW>
where
    RW: Read + Write + Clone + Send + Sync + Unpin + 'static,
    H::Error: From<std::io::Error> + From<http_types::Error> + ResponseError,
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
            .accept_one(|req| async {
                Ok::<_, http_types::Error>(match self.root.call(req, &mut cx).await {
                    Ok(ok) => ok,
                    Err(e) => e.to_response(),
                })
            })
            .await?
        {
            if let Some(reason) = cx.stop_reason.take() {
                return Err(reason);
            }
        }

        self.root.stopping(&mut cx).await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RateLimit {
    pub requests: usize,
    pub per: Duration,
}

/// An adapter that adds a rate limit to `handler`.
pub struct RateLimiter<H> {
    handler: H,
    pub limit: RateLimit,
    remaining: usize,
    until: Option<std::time::SystemTime>,
}

impl<H> RateLimiter<H> {
    pub fn new(handler: H, limit: RateLimit) -> Self {
        Self {
            handler,
            limit,
            remaining: limit.requests,
            until: None,
        }
    }
}

impl<H: Service> Service for RateLimiter<H> {
    type Error = H::Error;
    type Context = H::Context;
}

impl<H: Handler<Request, Response>> Handler<Request, Response> for RateLimiter<H> {
    async fn call(
        &mut self,
        request: Request,
        cx: &mut Self::Context,
    ) -> Result<Response, Self::Error> {
        let now = SystemTime::now();

        if let Some(until) = self.until.filter(|until| now < *until) {
            if self.remaining < 1 {
                let mut response = Response::new(StatusCode::TooManyRequests);
                response.append_header("x-ratelimit-limit", self.limit.requests.to_string());
                response.append_header("x-ratelimit-remaining", 0.to_string());
                response.append_header(
                    "x-ratelimit-reset",
                    until
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .to_string(),
                );
                Ok(response)
            } else {
                self.remaining -= 1;

                self.handler.call(request, cx).await
            }
        } else {
            self.remaining -= 1;
            self.until = Some(now + self.limit.per);

            self.handler.call(request, cx).await
        }
    }
}
