#![feature(async_fn_in_trait)]

use acril::prelude::http::*;

endpoint_error!(http_types::Error);

pub struct MyClient;

impl HttpClientContext for MyClient {
    type Error = http_types::Error;
    fn new_request(&self, _method: Method, _url: &str) -> Request {
        todo!()
    }
    async fn run_request(&self, _request: Request) -> Result<Response, Self::Error> {
        todo!()
    }
}

#[with_builder(coolness)]
#[derive(ClientEndpoint, Debug)]
#[endpoint(Post(empty, empty) "/test" in MyClient)]
pub struct MyCoolEndpoint {
    #[required]
    pub required: String,
    pub not_required: Vec<String>,
}

fn main() {
    let builder = MyClient
        .coolness(String::from("YEAH IM COOL ;)"))
        .not_required(vec![String::from("absolutely"), String::from("insanely")]);

    println!("{:#?}", builder.1);
}
