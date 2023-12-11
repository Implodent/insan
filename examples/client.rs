#![feature(async_fn_in_trait)]

use tracing::*;

use insan::{
    http::{
        client::{ClientEndpoint, HttpClient, HttpClientContext},
        http_types,
    },
    Service,
};
use insan_http::types::{Method, StatusCode, Url};

struct SayToServer(String);

impl Service for SayToServer {
    type Context = HttpClient;
    type Error = http_types::Error;
}

impl ClientEndpoint for SayToServer {
    type Output = String;

    async fn run(&self, context: &Self::Context) -> Result<Self::Output, Self::Error> {
        let mut request =
            context.new_request(Method::Post, Url::parse("http://127.0.0.1:3000/say_hello")?);
        request.set_body(self.0.as_str());

        let mut response = context.run_request(request).await?;
        match response.status() {
            StatusCode::Ok => response.body_string().await,
            other => Err(http_types::Error::from_str(
                other,
                format!("Expected status 200 OK, got {other}"),
            )),
        }
    }
}

#[async_std::main]
async fn main() -> Result<(), http_types::Error> {
    tracing_subscriber::fmt().init();
    info!("Starting helloer program.");

    let client = HttpClient::new();

    info!("Trying to say hello to server...");
    let output = client.call(SayToServer(String::from("Hello, server!"))).await?;
    info!(%output, "Successfully said hello to server!");

    Ok(())
}
