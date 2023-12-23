//! Process HTTP connections on the client.

use std::sync::Arc;

use tokio::net::TcpStream;

use async_compat::CompatExt;
use http_types::{Request, Response, StatusCode};
use tokio::io::{self, AsyncRead as Read, AsyncWrite as Write};

#[cfg(not(target_arch = "wasm32"))]
mod decode;
#[cfg(not(target_arch = "wasm32"))]
mod encode;

#[cfg(not(target_arch = "wasm32"))]
pub use decode::decode;
#[cfg(not(target_arch = "wasm32"))]
pub use encode::Encoder;
use tokio_rustls::{
    rustls::{
        pki_types::DnsName,
        ClientConfig, RootCertStore,
    },
    TlsConnector,
};

#[cfg(not(target_arch = "wasm32"))]
async fn native_connect<RW>(mut stream: RW, req: Request) -> http_types::Result<Response>
where
    RW: Read + Write + Send + Sync + Unpin + 'static,
{
    let mut req = Encoder::new(req);
    log::trace!("> {:?}", &req);

    io::copy(&mut req.compat_mut(), &mut stream).await?;

    let res = decode(stream.compat()).await?;
    log::trace!("< {:?}", &res);

    Ok(res)
}

fn make_connector() -> TlsConnector {
    let mut root_cert_store = RootCertStore::empty();
    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

/// Opens an HTTP/1.1 connection to a remote host.
pub async fn connect(req: Request) -> http_types::Result<Response> {
    #[cfg(target_arch = "wasm32")]
    {
        use futures::prelude::*;
        use send_wrapper::SendWrapper;
        SendWrapper::new(async move {
            let req: fetch::Request = fetch::Request::new(req).await?;
            let mut res = req.send().await?;

            let body = res.body_bytes();
            let mut response =
                Response::new(http_types::StatusCode::try_from(res.status()).unwrap());
            response.set_body(Body::from(body));
            for (name, value) in res.headers() {
                let name: http_types::headers::HeaderName = name.parse().unwrap();
                response.append_header(&name, value);
            }

            Ok(response)
        })
        .await
    }

    #[cfg(not(target_arch = "wasm32"))]
    if req.url().scheme() == "https" {
        let stream = TcpStream::connect(format!(
            "{}:{}",
            req.url().host_str().ok_or_else(|| {
                http_types::Error::from_str(
                    StatusCode::UnprocessableEntity,
                    "No host in request URL",
                )
            })?,
            req.url().port_or_known_default().ok_or_else(|| {
                http_types::Error::from_str(
                    StatusCode::UnprocessableEntity,
                    "No port in request URL",
                )
            })?
        ))
        .await?;
        let connector = make_connector();

        native_connect(
            connector
                .connect(
                    tokio_rustls::rustls::pki_types::ServerName::DnsName(
                        DnsName::try_from(req.host().ok_or_else(|| {
                            http_types::Error::from_str(
                                StatusCode::UnprocessableEntity,
                                "No host in request URL",
                            )
                        })?)
                        .unwrap().to_owned(),
                    ),
                    stream,
                )
                .await?,
            req,
        )
        .await
    } else {
        native_connect(
            TcpStream::connect(format!(
                "{}:{}",
                req.url().host_str().ok_or_else(|| {
                    http_types::Error::from_str(
                        StatusCode::UnprocessableEntity,
                        "No host in request URL",
                    )
                })?,
                req.url().port_or_known_default().ok_or_else(|| {
                    http_types::Error::from_str(
                        StatusCode::UnprocessableEntity,
                        "No port in request URL",
                    )
                })?
            ))
            .await?,
            req,
        )
        .await
    }
}

#[cfg(target_arch = "wasm32")]
mod fetch {
    use js_sys::{Array, ArrayBuffer, Reflect, Uint8Array};
    use wasm_bindgen::{prelude::*, JsCast};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{RequestInit, Window, WorkerGlobalScope};

    use std::iter::{IntoIterator, Iterator};
    use std::pin::Pin;

    use http_types::StatusCode;

    use http_types::Error;

    enum WindowOrWorker {
        Window(Window),
        Worker(WorkerGlobalScope),
    }

    impl WindowOrWorker {
        fn new() -> Self {
            #[wasm_bindgen]
            extern "C" {
                type Global;

                #[wasm_bindgen(method, getter, js_name = Window)]
                fn window(this: &Global) -> JsValue;

                #[wasm_bindgen(method, getter, js_name = WorkerGlobalScope)]
                fn worker(this: &Global) -> JsValue;
            }

            let global: Global = js_sys::global().unchecked_into();

            if !global.window().is_undefined() {
                Self::Window(global.unchecked_into())
            } else if !global.worker().is_undefined() {
                Self::Worker(global.unchecked_into())
            } else {
                panic!("Only supported in a browser or web worker");
            }
        }
    }

    /// Create a new fetch request.

    /// An HTTP Fetch Request.
    pub(crate) struct Request {
        request: web_sys::Request,
        /// This field stores the body of the request to ensure it stays allocated as long as the request needs it.
        #[allow(dead_code)]
        body_buf: Pin<Vec<u8>>,
    }

    impl Request {
        /// Create a new instance.
        pub(crate) async fn new(mut req: super::Request) -> Result<Self, Error> {
            // create a fetch request initaliser
            let mut init = RequestInit::new();

            // set the fetch method
            init.method(req.method().as_ref());

            let uri = req.url().to_string();
            let body = req.take_body();

            // convert the body into a uint8 array
            // needs to be pinned and retained inside the Request because the Uint8Array passed to
            // js is just a portal into WASM linear memory, and if the underlying data is moved the
            // js ref will become silently invalid
            let body_buf = body.into_bytes().await.map_err(|_| {
                Error::from_str(StatusCode::BadRequest, "could not read body into a buffer")
            })?;
            let body_pinned = Pin::new(body_buf);
            if body_pinned.len() > 0 {
                let uint_8_array = unsafe { js_sys::Uint8Array::view(&body_pinned) };
                init.body(Some(&uint_8_array));
            }

            let request = web_sys::Request::new_with_str_and_init(&uri, &init).map_err(|e| {
                Error::from_str(
                    StatusCode::BadRequest,
                    format!("failed to create request: {:?}", e),
                )
            })?;

            // add any fetch headers
            let headers: &mut super::Headers = req.as_mut();
            for (name, value) in headers.iter() {
                let name = name.as_str();
                let value = value.as_str();

                request.headers().set(name, value).map_err(|_| {
                    Error::from_str(
                        StatusCode::BadRequest,
                        format!("could not add header: {} = {}", name, value),
                    )
                })?;
            }

            Ok(Self {
                request,
                body_buf: body_pinned,
            })
        }

        /// Submit a request
        // TODO(yoshuawuyts): turn this into a `Future` impl on `Request` instead.
        pub(crate) async fn send(self) -> Result<Response, Error> {
            // Send the request.
            let scope = WindowOrWorker::new();
            let promise = match scope {
                WindowOrWorker::Window(window) => window.fetch_with_request(&self.request),
                WindowOrWorker::Worker(worker) => worker.fetch_with_request(&self.request),
            };
            let resp = JsFuture::from(promise)
                .await
                .map_err(|e| Error::from_str(StatusCode::BadRequest, format!("{:?}", e)))?;

            debug_assert!(resp.is_instance_of::<web_sys::Response>());
            let res: web_sys::Response = resp.dyn_into().unwrap();

            // Get the response body.
            let promise = res.array_buffer().unwrap();
            let resp = JsFuture::from(promise).await.unwrap();
            debug_assert!(resp.is_instance_of::<js_sys::ArrayBuffer>());
            let buf: ArrayBuffer = resp.dyn_into().unwrap();
            let slice = Uint8Array::new(&buf);
            let mut body: Vec<u8> = vec![0; slice.length() as usize];
            slice.copy_to(&mut body);

            Ok(Response::new(res, body))
        }
    }

    /// An HTTP Fetch Response.
    pub(crate) struct Response {
        res: web_sys::Response,
        body: Option<Vec<u8>>,
    }

    impl Response {
        fn new(res: web_sys::Response, body: Vec<u8>) -> Self {
            Self {
                res,
                body: Some(body),
            }
        }

        /// Access the HTTP headers.
        pub(crate) fn headers(&self) -> Headers {
            Headers {
                headers: self.res.headers(),
            }
        }

        /// Get the request body as a byte vector.
        ///
        /// Returns an empty vector if the body has already been consumed.
        pub(crate) fn body_bytes(&mut self) -> Vec<u8> {
            self.body.take().unwrap_or_else(|| vec![])
        }

        /// Get the HTTP return status code.
        pub(crate) fn status(&self) -> u16 {
            self.res.status()
        }
    }

    /// HTTP Headers.
    pub(crate) struct Headers {
        headers: web_sys::Headers,
    }

    impl IntoIterator for Headers {
        type Item = (String, String);
        type IntoIter = HeadersIter;

        fn into_iter(self) -> Self::IntoIter {
            HeadersIter {
                iter: js_sys::try_iter(&self.headers).unwrap().unwrap(),
            }
        }
    }

    /// HTTP Headers Iterator.
    pub(crate) struct HeadersIter {
        iter: js_sys::IntoIter,
    }

    impl Iterator for HeadersIter {
        type Item = (String, String);

        fn next(&mut self) -> Option<Self::Item> {
            let pair = self.iter.next()?;

            let array: Array = pair.unwrap().into();
            let vals = array.values();

            let prop = String::from("value").into();
            let key = Reflect::get(&vals.next().unwrap(), &prop).unwrap();
            let value = Reflect::get(&vals.next().unwrap(), &prop).unwrap();

            Some((
                key.as_string().to_owned().unwrap(),
                value.as_string().to_owned().unwrap(),
            ))
        }
    }
}
