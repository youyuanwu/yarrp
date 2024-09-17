use futures::future::BoxFuture;
use std::os::windows::io::AsRawSocket;
use std::os::windows::io::FromRawSocket;
use std::os::windows::io::IntoRawSocket;
use std::path::Path;
use std::sync::Arc;
use std::task::Poll;
use uds_windows::UnixListener;

pub struct UdsAcceptStream {
    inner: Arc<UnixListener>,
    fu: Option<BoxFuture<'static, std::io::Result<tokio::net::TcpStream>>>,
}

impl UdsAcceptStream {
    pub fn new(ul: UnixListener) -> Self {
        // ul.set_nonblocking(true).unwrap();
        // let std_listener = unsafe { std::net::TcpListener::from_raw_socket(ul.into_raw_socket()) };
        // let l = tokio::net::TcpListener::from_std(std_listener).unwrap();
        Self {
            inner: Arc::new(ul),
            fu: None,
        }
    }

    pub fn bind<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let l = UnixListener::bind(path)?;
        Ok(Self::new(l))
    }

    // accept a stream from inner
    // TODO: This does not reacting to cancellation from select. So the test is disabled for uds.
    async fn accept(
        inner: Arc<UnixListener>,
    ) -> std::io::Result<(tokio::net::TcpStream, uds_windows::SocketAddr)> {
        let (unix_stream, addr) = tokio::task::spawn_blocking(move || inner.accept()).await??;
        unix_stream.set_nonblocking(true).unwrap();
        // Create a std::net::TcpStream from the raw Unix socket. Windows APIs that
        // accept sockets have defined behavior for Unix sockets (either they
        // successfully handle it or return an error), so this should be safe.
        let std_stream =
            unsafe { std::net::TcpStream::from_raw_socket(unix_stream.into_raw_socket()) };
        let tokio_tcp_stream = tokio::net::TcpStream::from_std(std_stream)?;
        Ok((tokio_tcp_stream, addr))
    }
}

impl tokio_stream::Stream for UdsAcceptStream {
    type Item = std::io::Result<tokio::net::TcpStream>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(f) = self.fu.as_mut() {
            let pl = f.as_mut().poll(cx).map(Some);
            // clean this and next time tcp accept is called.
            if pl.is_ready() {
                self.fu.take();
            }
            return pl;
        }

        let inner = self.inner.clone();
        self.fu = Some(Box::pin(async move {
            Self::accept(inner).await.map(|(x, _)| x)
        }));
        let out = self.fu.as_mut().unwrap().as_mut().poll(cx).map(Some);
        if matches!(out, Poll::Ready(_)) {
            // clear an prepare next
            self.fu.take();
        }
        out
    }
}

// This naive uds listener on win does not work. See: https://github.com/tokio-rs/tokio/issues/2201
// pub fn make_uds_accept_stream(ul: UnixListener) -> std::io::Result<super::TcpListenerStream> {
//     ul.set_nonblocking(true).unwrap();
//     let std_listener = unsafe { std::net::TcpListener::from_raw_socket(ul.into_raw_socket()) };
//     let l = tokio::net::TcpListener::from_std(std_listener)?;
//     Ok(super::TcpListenerStream::new(l))
// }

impl Drop for UdsAcceptStream {
    fn drop(&mut self) {
        let sock = self.inner.as_raw_socket();
        // hack to close the socket to abort accept blocking call. TODO: this is not safe.
        let _std_sock = unsafe { std::net::TcpListener::from_raw_socket(sock) };
    }
}
