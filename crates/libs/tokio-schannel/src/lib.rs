// schannel

use std::{
    fmt,
    future::Future,
    io::{self, Read, Write},
    pin::Pin,
    task::{Context, Poll},
};

use schannel::tls_stream::MidHandshakeTlsStream;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

// ----------- wrapper copied from openssl tokio
struct StreamWrapper<S> {
    stream: S,
    context: usize,
}

impl<S> fmt::Debug for StreamWrapper<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.stream, fmt)
    }
}

impl<S> StreamWrapper<S> {
    /// # Safety
    ///
    /// Must be called with `context` set to a valid pointer to a live `Context` object, and the
    /// wrapper must be pinned in memory.
    unsafe fn parts(&mut self) -> (Pin<&mut S>, &mut Context<'_>) {
        debug_assert_ne!(self.context, 0);
        let stream = Pin::new_unchecked(&mut self.stream);
        let context = &mut *(self.context as *mut Context<'_>);
        (stream, context)
    }
}

impl<S> Read for StreamWrapper<S>
where
    S: AsyncRead,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let (stream, cx) = unsafe { self.parts() };
        let mut buf = ReadBuf::new(buf);
        match stream.poll_read(cx, &mut buf)? {
            Poll::Ready(()) => Ok(buf.filled().len()),
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}

impl<S> Write for StreamWrapper<S>
where
    S: AsyncWrite,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let (stream, cx) = unsafe { self.parts() };
        match stream.poll_write(cx, buf) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let (stream, cx) = unsafe { self.parts() };
        match stream.poll_flush(cx) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}

// cvt error to poll
fn cvt<T>(r: io::Result<T>) -> Poll<io::Result<T>> {
    match r {
        Ok(v) => Poll::Ready(Ok(v)),
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
        Err(e) => Poll::Ready(Err(e)),
    }
}

// ----------- wrapper copied from openssl tokio end

#[derive(Debug)]
pub struct SslStream<S>(schannel::tls_stream::TlsStream<StreamWrapper<S>>);

impl<S> SslStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    // /// Like [`TlsStream::new`](schannel::tls_stream::TlsStream).
    // pub fn new( stream: S) -> Result<Self, ErrorStack> {
    //     ssl::SslStream::new(ssl, StreamWrapper { stream, context: 0 }).map(SslStream)
    // }
    //pub fn poll_connect()

    // pass the ctx in the wrapper and invoke f
    fn with_context<F, R>(self: Pin<&mut Self>, ctx: &mut Context<'_>, f: F) -> R
    where
        F: FnOnce(&mut schannel::tls_stream::TlsStream<StreamWrapper<S>>) -> R,
    {
        let this = unsafe { self.get_unchecked_mut() };
        this.0.get_mut().context = ctx as *mut _ as usize;
        let r = f(&mut this.0);
        this.0.get_mut().context = 0;
        r
    }
}

impl<S> AsyncRead for SslStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.with_context(ctx, |s| {
            // TODO: read into uninitialized for optimize
            match cvt(s.read(buf.initialize_unfilled()))? {
                Poll::Ready(nread) => {
                    buf.advance(nread);
                    Poll::Ready(Ok(()))
                }
                Poll::Pending => Poll::Pending,
            }
        })
    }
}

impl<S> AsyncWrite for SslStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn poll_write(self: Pin<&mut Self>, ctx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.with_context(ctx, |s| cvt(s.write(buf)))
    }

    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<io::Result<()>> {
        self.with_context(ctx, |s| cvt(s.flush()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<io::Result<()>> {
        // TODO: May need to check error and retry
        self.with_context(ctx, |s| cvt(s.shutdown()))
    }
}

// acceptor
pub struct TlsAcceptor {
    inner: schannel::tls_stream::Builder,
    cred: schannel::schannel_cred::SchannelCred,
}

impl TlsAcceptor {
    pub async fn accept<IO>(&mut self, stream: IO) -> io::Result<SslStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let s_wrap = StreamWrapper { stream, context: 0 };
        let cred = self.cred.clone();
        match self.inner.accept(cred, s_wrap) {
            Ok(s) => Ok(SslStream(s)),
            Err(e) => match e {
                schannel::tls_stream::HandshakeError::Failure(e) => Err(e),
                schannel::tls_stream::HandshakeError::Interrupted(mid_handshake_tls_stream) => {
                    MidHandShake(Some(mid_handshake_tls_stream)).await
                }
            },
        }

        //self.with_context(cx, |s| cvt_ossl(s.accept()))
    }
}

pub struct MidHandShake<S>(Option<MidHandshakeTlsStream<StreamWrapper<S>>>);

impl<S: AsyncRead + AsyncWrite + Unpin> Future for MidHandShake<S> {
    type Output = io::Result<SslStream<S>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut_self = self.get_mut();
        let mut s = mut_self.0.take().expect("future polled after completion");
        // save ctx at tokio stream
        s.get_mut().context = cx as *mut _ as usize;
        match s.handshake() {
            Ok(mut st) => {
                st.get_mut().context = 0;
                Poll::Ready(Ok(SslStream(st)))
            }
            Err(e) => {
                match e {
                    schannel::tls_stream::HandshakeError::Failure(error) => Poll::Ready(Err(error)),
                    schannel::tls_stream::HandshakeError::Interrupted(mid_handshake_tls_stream) => {
                        //s.get_mut().context = 0;
                        mut_self.0 = Some(mid_handshake_tls_stream);
                        Poll::Pending
                    }
                }
            }
        }
    }
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
