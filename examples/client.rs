#![feature(async_fn_in_trait)]

endpoint_error!(http_types::Error);

use acril::prelude::http::*;
use std::fmt::Display;
use tracing::*;

#[derive(ClientEndpoint)]
#[endpoint(Post(display) "/say_hello" in HttpClient -> String)]
struct SayToServer(String);

impl Display for SayToServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[async_std::main]
async fn main() {
    tracing_subscriber::fmt().init();
    info!("Starting helloer program.");

    if let Err(e) = async move {
        let client = HttpClient::new().with_base_url(Url::parse("http://localhost:3000")?);

        info!("Trying to say hello to server...");
        let output = client
            .call(SayToServer(String::from("Hello, server!")))
            .await?;
        info!(%output, "Successfully said hello to server!");

        Ok::<_, http_types::Error>(())
    }
    .await
    {
        error!("{e}");
    }
}
