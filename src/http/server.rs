use super::*;

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
    H::Error: From<std::io::Error> + From<acril_http::types::Error> + ResponseError,
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
