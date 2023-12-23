use std::{future::IntoFuture, marker::PhantomData};

use async_tungstenite::{tokio::TokioAdapter, tungstenite::Message, WebSocketStream};
use futures::{Sink, SinkExt, StreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    task::JoinHandle,
};

use crate::{Handler, Service};

pub struct WebSocket<S, H: Service> {
    handle: JoinHandle<Result<(), H::Error>>,
    phantom: PhantomData<S>,
}

impl<
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        H: Service<Context = WebSocketStream<TokioAdapter<S>>>,
    > WebSocket<S, H>
where
    H::Error: From<async_tungstenite::tungstenite::Error>
        + From<<WebSocketStream<TokioAdapter<S>> as Sink<Message>>::Error>
        + Send
        + 'static,
    H: Handler<Message, call(): Send> + Send + 'static,
    H::Response: Into<Message>,
{
    pub async fn new(stream: WebSocketStream<TokioAdapter<S>>, handler: H) -> Self {
        Self {
            handle: tokio::task::spawn(Self::lifecycle(stream, handler)),
            phantom: PhantomData,
        }
    }

    pub async fn lifecycle(
        mut stream: WebSocketStream<TokioAdapter<S>>,
        mut handler: H,
    ) -> Result<(), H::Error> {
        while let Some(item) = stream.next().await {
            let response = handler.call(item?, &mut stream).await?;
            stream.send(response.into()).await?;
        }

        Ok(())
    }

    pub fn abort(self) {
        self.handle.abort();
    }
}

impl<S, H: Service> IntoFuture for WebSocket<S, H> {
    type Output = Result<Result<(), H::Error>, tokio::task::JoinError>;
    type IntoFuture = JoinHandle<Result<(), H::Error>>;

    fn into_future(self) -> Self::IntoFuture {
        self.handle
    }
}
