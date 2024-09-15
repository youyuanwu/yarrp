// pub trait Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin {}

// TODO: these abstraction might not workout

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::future::BoxFuture;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::Stream;

type RustlsStream = tokio_rustls::server::TlsStream<TcpStream>;

pub struct RustlsAcceptStream {
    inner: TcpListener,
    tls: tokio_rustls::TlsAcceptor,
    fu: Option<BoxFuture<'static, std::io::Result<RustlsStream>>>,
}

impl RustlsAcceptStream {
    pub fn new(tcp: TcpListener, tls: tokio_rustls::TlsAcceptor) -> Self {
        Self {
            inner: tcp,
            tls,
            fu: None,
        }
    }
}

impl Stream for RustlsAcceptStream {
    type Item = std::io::Result<tokio_rustls::server::TlsStream<TcpStream>>;

    // This is not efficient since tls and accept does not happen in parallel
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // if has pending slot poll it.
        if let Some(f) = self.fu.as_mut() {
            let pl = f.as_mut().poll(cx).map(Some);
            // clean this and next time tcp accept is called.
            if pl.is_ready() {
                self.fu.take();
            }
            return pl;
        }

        // poll tcp and construct next f
        let tcp_s = self.inner.poll_accept(cx);
        let s = match tcp_s {
            Poll::Ready(res) => match res {
                Ok((s, _)) => s,
                Err(e) => return Poll::Ready(Some(Err(e))),
            },
            Poll::Pending => return Poll::Pending,
        };

        assert!(self.fu.is_none());
        let tls_cp = self.tls.clone();
        self.fu = Some(Box::pin(async move { tls_cp.accept(s).await }));
        let out = self.fu.as_mut().unwrap().as_mut().poll(cx).map(Some);
        if matches!(out, Poll::Ready(_)) {
            // clear an prepare next
            self.fu.take();
        }
        out
    }
}
