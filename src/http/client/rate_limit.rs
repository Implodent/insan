use std::{time::Duration, pin::Pin};

use super::*;
use tokio::time::{Sleep, Instant};

struct RateLimit<S> {
    inner: S,
    sleep: Pin<Box<Sleep>>,
}

impl<S: Service> Service for RateLimit<S> {
    type Context = S::Context;
    type Error = S::Error;
}
impl<S: Handler<Request, Response = Response>> Handler<Request> for RateLimit<S>
where
    S::Error: From<http_types::Error>,
{
    type Response = S::Response;

    async fn call(
        &mut self,
        request: Request,
        cx: &mut Self::Context,
    ) -> Result<Self::Response, Self::Error> {
        (&mut self.sleep).await;

        let response = self.inner.call(request, cx).await?;
        if response.status() == StatusCode::TooManyRequests {
            if let Some(reset) = response.header("x-ratelimit-reset") {
                use std::str::FromStr;

                let when = Duration::from_secs(
                    u64::from_str(&reset.to_string()).unwrap()
                        - std::time::SystemTime::UNIX_EPOCH
                            .elapsed()
                            .unwrap()
                            .as_secs(),
                );

                let mut min = when.as_secs() / 60;
                let hrs = {
                    let hrs = min / 24;
                    min = min - (hrs * 24);
                    hrs
                };
                let sec = when.as_secs() - min * 60;
                tracing::debug!("got rate limited, waiting for {hrs}h {min}m {sec}s");

                self.sleep.as_mut().reset(Instant::now() + when);
            }
        }

        Ok(response)
    }
}
