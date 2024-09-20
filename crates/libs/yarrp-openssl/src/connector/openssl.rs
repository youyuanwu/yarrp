use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use hyper::Uri;
use hyper_util::rt::TokioIo;

use crate::OpensslStream;

// use crate::accept_stream::OpensslStream;

// openssl stream for connecting/proxying to tls endpoints for sending http requests
#[derive(Debug, Clone)]
pub struct OpensslConnector {
    inner: openssl::ssl::SslConnector,
    addr: SocketAddr,
}

impl OpensslConnector {
    pub fn new(inner: openssl::ssl::SslConnector, addr: SocketAddr) -> Self {
        Self { inner, addr }
    }

    pub async fn connect(&self) -> Result<TokioIo<crate::OpensslStream>, yarrp::Error> {
        // Connect and get ssl stream
        let ssl = self
            .inner
            .configure()
            .unwrap()
            .into_ssl("localhost")
            .unwrap();
        let io = tokio::net::TcpStream::connect(&self.addr).await?;
        let mut stream = crate::accept_stream::SslStream::new(ssl, io)?;
        std::pin::Pin::new(&mut stream).connect().await?;
        Ok::<_, yarrp::Error>(TokioIo::new(OpensslStream::new(stream)))
    }
}

impl Unpin for OpensslConnector {}

impl tower::Service<hyper::Uri> for OpensslConnector {
    type Response = TokioIo<crate::OpensslStream>;
    type Error = yarrp::Error;
    #[allow(clippy::type_complexity)]
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn call(&mut self, _req: Uri) -> Self::Future {
        let conn = self.clone();
        let fut = async move { conn.connect().await };
        Box::pin(fut)
    }

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
