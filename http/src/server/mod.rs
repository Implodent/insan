//! Process HTTP connections on the server.

use futures::io::{self, AsyncRead as Read, AsyncWrite as Write};
use http_types::headers::{CONNECTION, UPGRADE};
use http_types::upgrade::Connection;
use http_types::{Request, Response, StatusCode};
use std::{future::Future, time::Duration};
mod body_reader;
mod decode;
mod encode;

pub use decode::decode;
pub use encode::Encoder;

/// Configure the server.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// Timeout to handle headers. Defaults to 60s.
    headers_timeout: Option<Duration>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            headers_timeout: Some(Duration::from_secs(60)),
        }
    }
}

/// struct for server
#[derive(Debug)]
pub struct Server<RW> {
    io: RW,
    opts: ServerOptions,
}

/// An enum that represents whether the server should accept a subsequent request
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConnectionStatus {
    /// The server should not accept another request
    Close,

    /// The server may accept another request
    KeepAlive,
}

impl<RW> Server<RW>
where
    RW: Read + Write + Clone + Send + Sync + Unpin + 'static,
{
    /// builds a new server
    pub fn new(io: RW) -> Self {
        Self {
            io,
            opts: Default::default(),
        }
    }

    /// with opts
    pub fn with_opts(mut self, opts: ServerOptions) -> Self {
        self.opts = opts;
        self
    }

    /// accept one request
    pub async fn accept_one<
        F: FnOnce(Request) -> Fut,
        Fut: Future<Output = Result<Response, Error>>,
        Error: From<http_types::Error> + From<std::io::Error>,
    >(
        &mut self,
        callback: F,
    ) -> Result<ConnectionStatus, Error> {
        // Decode a new request, timing out if this takes longer than the timeout duration.
        let fut = decode(self.io.clone());

        let (req, mut body) = if let Some(timeout_duration) = self.opts.headers_timeout {
            match tokio::time::timeout(timeout_duration, fut).await {
                Ok(Ok(Some(r))) => r,
                Err(_) | Ok(Ok(None)) => return Ok(ConnectionStatus::Close), /* EOF or timeout */
                Ok(Err(e)) => return Err(e.into()),
            }
        } else {
            match fut.await? {
                Some(r) => r,
                None => return Ok(ConnectionStatus::Close), /* EOF */
            }
        };

        let has_upgrade_header = req.header(UPGRADE).is_some();
        let connection_header_as_str = req
            .header(CONNECTION)
            .map(|connection| connection.as_str())
            .unwrap_or("");

        let connection_header_is_upgrade = connection_header_as_str
            .split(',')
            .any(|s| s.trim().eq_ignore_ascii_case("upgrade"));
        let mut close_connection = connection_header_as_str.eq_ignore_ascii_case("close");

        let upgrade_requested = has_upgrade_header && connection_header_is_upgrade;

        let method = req.method();

        // Pass the request to the endpoint and encode the response.
        let mut res = (callback)(req).await?;

        close_connection |= res
            .header(CONNECTION)
            .map(|c| c.as_str().eq_ignore_ascii_case("close"))
            .unwrap_or(false);

        let upgrade_provided = res.status() == StatusCode::SwitchingProtocols && res.has_upgrade();

        let upgrade_sender = if upgrade_requested && upgrade_provided {
            Some(res.send_upgrade())
        } else {
            None
        };

        let mut encoder = Encoder::new(res, method);

        let bytes_written = io::copy(&mut encoder, &mut self.io).await?;
        log::trace!("wrote {} response bytes", bytes_written);

        let body_bytes_discarded = io::copy(&mut body, &mut io::sink()).await?;
        log::trace!(
            "discarded {} unread request body bytes",
            body_bytes_discarded
        );

        if let Some(upgrade_sender) = upgrade_sender {
            upgrade_sender.send(Connection::new(self.io.clone())).await;
            Ok(ConnectionStatus::Close)
        } else if close_connection {
            Ok(ConnectionStatus::Close)
        } else {
            Ok(ConnectionStatus::KeepAlive)
        }
    }
}
