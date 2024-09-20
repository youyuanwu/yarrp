use std::{pin::Pin, task::Poll};

use futures::{future::BoxFuture, Stream};
use openssl::ssl::Ssl;
use tokio::net::TcpListener;

use crate::OpensslStream;

pub struct OpensslAcceptStream {
    inner: TcpListener,
    tls: openssl::ssl::SslAcceptor,
    fu: Option<BoxFuture<'static, Result<OpensslStream, crate::Error>>>,
}

impl OpensslAcceptStream {
    pub fn new(inner: TcpListener, tls: openssl::ssl::SslAcceptor) -> Self {
        Self {
            inner,
            tls,
            fu: None,
        }
    }
}

impl Stream for OpensslAcceptStream {
    type Item = Result<OpensslStream, crate::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
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
        let tls_cp = self.tls.clone();
        let fu = async move {
            let ssl = Ssl::new(tls_cp.context())?;
            let mut stream = tokio_openssl::SslStream::new(ssl, s).map_err(crate::Error::from)?;
            Pin::new(&mut stream)
                .accept()
                .await
                .map_err(crate::Error::from)?;
            // let s = OpensslStream(stream);
            Ok(OpensslStream::new(stream))
        };
        self.fu = Some(Box::pin(fu));
        let out = self.fu.as_mut().unwrap().as_mut().poll(cx).map(Some);
        if matches!(out, Poll::Ready(_)) {
            // clear an prepare next
            self.fu.take();
        }
        out
    }
}
