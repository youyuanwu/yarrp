use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::future::BoxFuture;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};
use tokio_stream::Stream;

use super::ClientAuthorizor;

type RustlsStream = tokio_rustls::server::TlsStream<TcpStream>;

pub struct RustlsAcceptStream {
    inner: TcpListener,
    tls: tokio_rustls::TlsAcceptor,
    fu: Option<BoxFuture<'static, Result<RustlsStream, crate::Error>>>,
    authorizor: Option<Arc<dyn ClientAuthorizor>>,
}

impl RustlsAcceptStream {
    pub fn new(
        tcp: TcpListener,
        tls: tokio_rustls::TlsAcceptor,
        authorizor: Option<Arc<dyn ClientAuthorizor>>,
    ) -> Self {
        Self {
            inner: tcp,
            tls,
            fu: None,
            authorizor,
        }
    }
}

impl Stream for RustlsAcceptStream {
    type Item = Result<tokio_rustls::server::TlsStream<TcpStream>, crate::Error>;

    // TODO: This is not efficient since tls and accept does not happen in parallel
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
                Err(e) => return Poll::Ready(Some(Err(e.into()))),
            },
            Poll::Pending => return Poll::Pending,
        };

        assert!(self.fu.is_none());
        let tls_cp = self.tls.clone();
        let authorizor = self.authorizor.clone();
        self.fu = Some(Box::pin(async move {
            let mut tls_s = tls_cp.accept(s).await.map_err(crate::Error::from)?;
            let (_, conn) = tls_s.get_ref();
            // authorize or reject the connection via callback
            if let Some(auth) = authorizor {
                if let Err(e) = auth.on_connect(conn) {
                    // authorize failed, close the connection.
                    let _ = tls_s.shutdown().await;
                    return Err(e);
                }
            }
            Ok(tls_s)
        }));
        let out = self.fu.as_mut().unwrap().as_mut().poll(cx).map(Some);
        if matches!(out, Poll::Ready(_)) {
            // clear an prepare next
            self.fu.take();
        }
        out
    }
}
