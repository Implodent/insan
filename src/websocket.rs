use async_tungstenite::{tungstenite::Message, WebSocketStream};
use futures::{Sink, SinkExt, StreamExt, io::{AsyncRead, AsyncWrite}};

use crate::{Handler, Service};

pub async fn lifecycle<
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: Service<Context = WebSocketStream<S>>,
>(
    mut stream: WebSocketStream<S>,
    mut handler: H,
) -> Result<(), H::Error>
where
    H::Error: From<async_tungstenite::tungstenite::Error>
        + From<<WebSocketStream<S> as Sink<Message>>::Error>
        + Send
        + 'static,
    H: Handler<Message, call(): Send> + Send + 'static,
    H::Response: Into<Message>,
{
    while let Some(item) = stream.next().await {
        let response = handler.call(item?, &mut stream).await?;
        stream.send(response.into()).await?;
    }

    Ok(())
}
