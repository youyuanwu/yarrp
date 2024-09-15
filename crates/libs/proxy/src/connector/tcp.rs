use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use http::Uri;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioIo};

/// Connector for tcp.
/// Fixed address.
/// TODO: Replace the inner with custom impl.
#[derive(Debug, Clone)]
pub struct TcpConnector {
    inner: hyper_util::client::legacy::connect::HttpConnector,
    addr: SocketAddr,
}

impl TcpConnector {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            inner: HttpConnector::new(),
            addr,
        }
    }
}

impl Unpin for TcpConnector {}

impl tower::Service<hyper::Uri> for TcpConnector {
    type Response = TokioIo<tokio::net::TcpStream>;
    type Error = std::io::Error;
    #[allow(clippy::type_complexity)]
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn call(&mut self, req: Uri) -> Self::Future {
        // fix req to have fixed uri
        let req_target = http::uri::Builder::from(req)
            .authority(self.addr.to_string())
            .build()
            .unwrap();
        // .map_err(|e| {
        //     std::io::Error::new(std::io::ErrorKind::ConnectionAborted, e.to_string())
        // }).unwrap();
        let mut conn = self.inner.clone();
        // let fut = async move { conn. };
        let fut = async move {
            conn.call(req_target).await.map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::ConnectionAborted, e.to_string())
            })
        };

        Box::pin(fut)
    }

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
