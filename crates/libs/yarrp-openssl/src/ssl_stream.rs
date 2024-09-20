// TcpStream implements hyper-util Connection trait in hyper-util.
// SslStream does not impl such.
// Need to impl Connection for SslStream here, but due to this is external mod, it requires boiler plate.

use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};

// For legacy client
pin_project! {
    pub struct OpensslStream{
        #[pin]
        inner:  tokio_openssl::SslStream<tokio::net::TcpStream>
    }
}

impl OpensslStream {
    pub fn new(inner: tokio_openssl::SslStream<tokio::net::TcpStream>) -> Self {
        Self { inner }
    }
}

/// impl hyper legacy client requirement
impl hyper_util::client::legacy::connect::Connection for OpensslStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        let tcp = self.inner.get_ref();
        tcp.connected()
    }
}

/// TODO: fill this
#[derive(Clone)]
pub struct TonicConnInfo {}

/// impl tonic requirement for the stream to be used in tonic
impl tonic::transport::server::Connected for OpensslStream {
    type ConnectInfo = TonicConnInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        TonicConnInfo {}
    }
}

impl AsyncRead for OpensslStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().inner.poll_read(cx, buf)
    }
}

impl AsyncWrite for OpensslStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.project().inner.poll_shutdown(cx)
    }
}

// TODO: impl traits

// expose inner tratis of the ssl stream
// impl std::ops::Deref for OpensslStream {
//     type Target = tokio_openssl::SslStream<TcpStream>;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }
