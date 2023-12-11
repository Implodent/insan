#![endpoint_error = "http_types::Error"]

#[derive(ClientEndpoint)]
#[endpoint(Get "/" in TradingClient -> Account)]
pub struct GetAccount;

// generates:

type __Endpoint_Error = http_types::Error;

impl Service for GetAccount {
    type Context = TradingClient;
    type Error = __Endpoint_Error;
}

impl ClientEndpoint for GetAccount {
    type Output = Account;

    async fn call(&self, cx: &Self::Context) -> Result<Self::Output, Self::Error> {
        #[allow(unused_mut)]
        let mut request = cx.new_request(Method::Get, Url::parse("/")?);

        // unit struct - no request init logic

        Ok(cx.run_request(request).await?.body_json::<Self::Output>().await?)
    }
}
