use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::AsyncRead as Read;
use http_types::Body;
use pin_project::pin_project;
use tokio::io::ReadBuf;

use crate::chunked::ChunkedEncoder;

#[pin_project(project=BodyEncoderProjection)]
#[derive(Debug)]
pub(crate) enum BodyEncoder {
    Chunked(#[pin] ChunkedEncoder<Body>),
    Fixed(#[pin] Body),
}

impl BodyEncoder {
    pub(crate) fn new(body: Body) -> Self {
        match body.len() {
            Some(_) => Self::Fixed(body),
            None => Self::Chunked(ChunkedEncoder::new(body)),
        }
    }
}

impl Read for BodyEncoder {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.project() {
            BodyEncoderProjection::Chunked(encoder) => encoder.poll_read(cx, buf),
            BodyEncoderProjection::Fixed(body) => body.poll_read(cx, buf),
        }
    }
}
