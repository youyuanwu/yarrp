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

    // // internal helper to set context and execute sync function.
    // fn with_context<F, R>(&mut self, ctx: &mut Context<'_>, f: F) -> R
    // where
    //     F: FnOnce(&mut Self) -> R,
    // {
    //     self.context = ctx as *mut _ as usize;
    //     let r = f(self);
    //     self.context = 0;
    //     r
    // }
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
    pub fn new(
        inner: schannel::tls_stream::Builder,
        cred: schannel::schannel_cred::SchannelCred,
    ) -> Self {
        Self { inner, cred }
    }

    // This needs to be poll
    pub async fn accept<IO>(&mut self, stream: IO) -> io::Result<SslStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let s_wrap = StreamWrapper { stream, context: 0 };
        let cred = self.cred.clone();
        // TODO: This does not work due to cx not set.
        conv_schannel_err(self.inner.accept(cred, s_wrap)).await

        // cannot impl this api from poll. TODO:

        // match future::poll_fn(|cx| { self.as_mut().poll_accept(cx, stream)}).await {
        //     Ok(s) => Ok(s),
        //     Err(e) => {
        //         let mid = e?;
        //         mid.await
        //     },
        // }
        // todo!()
    }

    /// TODO: This has problem convert to async
    pub fn poll_accept<IO>(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        stream: IO,
    ) -> Poll<Result<SslStream<IO>, io::Result<MidHandShake<IO>>>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let mut s_wrap = StreamWrapper { stream, context: 0 };
        s_wrap.context = cx as *mut _ as usize;
        let cred = self.cred.clone();
        let res = self.inner.accept(cred, s_wrap);

        match res {
            Ok(mut s) => {
                s.get_mut().context = 0;
                Poll::Ready(Ok(SslStream(s)))
            }
            Err(e) => match e {
                schannel::tls_stream::HandshakeError::Failure(error) => {
                    Poll::Ready(Err(Err(error)))
                }
                schannel::tls_stream::HandshakeError::Interrupted(mut mid_handshake_tls_stream) => {
                    mid_handshake_tls_stream.get_mut().context = 0;
                    Poll::Ready(Err(Ok(MidHandShake(Some(mid_handshake_tls_stream)))))
                }
            },
        }
    }
}

// connector
pub struct TlsConnector {
    inner: schannel::tls_stream::Builder,
    cred: schannel::schannel_cred::SchannelCred,
}

impl TlsConnector {
    pub fn new(
        inner: schannel::tls_stream::Builder,
        cred: schannel::schannel_cred::SchannelCred,
    ) -> Self {
        Self { inner, cred }
    }

    pub async fn connect<IO>(&mut self, stream: IO) -> io::Result<SslStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let s_wrap = StreamWrapper { stream, context: 0 };
        let cred = self.cred.clone();
        conv_schannel_err(self.inner.connect(cred, s_wrap)).await
    }
}

// handle mid handshake error for accept or connect
async fn conv_schannel_err<S>(
    err: Result<
        schannel::tls_stream::TlsStream<StreamWrapper<S>>,
        schannel::tls_stream::HandshakeError<StreamWrapper<S>>,
    >,
) -> io::Result<SslStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match err {
        Ok(s) => Ok(SslStream(s)),
        Err(e) => match e {
            schannel::tls_stream::HandshakeError::Failure(e) => Err(e),
            schannel::tls_stream::HandshakeError::Interrupted(mid_handshake_tls_stream) => {
                MidHandShake(Some(mid_handshake_tls_stream)).await
            }
        },
    }
}

pub enum StartHandShake<S> {
    Mid(MidHandShake<S>),
    Done(SslStream<S>),
}

// pub struct StartHandShakeFu<S> {
//     f: dyn FnOnce() -> StartHandShake<S>,
// }

// impl<S> Future for StartHandShakeFu<S>
// {
//     type Output = StartHandShake<S>;
// }

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
            Err(e) => match e {
                schannel::tls_stream::HandshakeError::Failure(error) => Poll::Ready(Err(error)),
                schannel::tls_stream::HandshakeError::Interrupted(mut mid_handshake_tls_stream) => {
                    mid_handshake_tls_stream.get_mut().context = 0;
                    mut_self.0 = Some(mid_handshake_tls_stream);
                    Poll::Pending
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
