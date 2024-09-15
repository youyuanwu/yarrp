// reference: https://github.com/stefansundin/hyper-reverse-proxy/tree/master

use std::{future::Future, net::SocketAddr, sync::Arc};

use crate::uds_service::ProxyService;
use crate::{acceptor::RustlsAcceptStream, conn::UdsConnector};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
};
use tokio_stream::StreamExt;

pub use std::error::Error as StdError;
pub use tokio_util::sync::CancellationToken;
pub type Error = Box<dyn StdError + Send + Sync>;

/// Serves the proxy on the addr
pub async fn serve_proxy(
    addr: SocketAddr,
    token: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting to serve on https://{}", addr);

    // Create a TCP listener via tokio.
    let incoming = TcpListener::bind(&addr).await?;

    // Build TLS configuration.
    let (mut server_config, _) = crate::test_util::load_test_server_config();
    server_config.alpn_protocols = vec![b"h2".to_vec()]; // b"http/1.1".to_vec(), b"http/1.0".to_vec()
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    let test_socket = crate::test_util::get_test_socket_path();
    let conn = UdsConnector::new(test_socket);
    let service = ProxyService::new(conn).await;

    let rustls_accept_stream = RustlsAcceptStream::new(incoming, tls_acceptor);

    serve_with_incoming(rustls_accept_stream, service, async move {
        token.cancelled().await
    })
    .await?;
    Ok(())
}

pub async fn serve_proxy_tcp(
    addr: SocketAddr,
    token: CancellationToken,
) -> Result<(), crate::Error> {
    let incoming = TcpListener::bind(&addr).await?;
    let stream = tokio_stream::wrappers::TcpListenerStream::new(incoming);
    let test_socket = crate::test_util::get_test_socket_path();
    let conn = UdsConnector::new(test_socket);
    let service = ProxyService::new(conn).await;
    serve_with_incoming(stream, service, async move { token.cancelled().await }).await?;
    Ok(())
}

pub async fn serve_with_incoming<I, IO, IE, S, F>(
    mut incoming: I,
    svc: S,
    signal: F,
) -> Result<(), crate::Error>
where
    I: tokio_stream::Stream<Item = Result<IO, IE>> + Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    IE: Into<crate::Error>,
    S: hyper::service::Service<
            hyper::Request<hyper::body::Incoming>,
            Response = hyper::Response<hyper::body::Incoming>,
            Error: Into<crate::Error>,
        >
        + Send
        + 'static
        + Clone,
    S::Future: 'static + Send,
    F: Future<Output = ()>,
{
    let mut sig = std::pin::pin!(signal);
    loop {
        // get the next stream to run http on
        let inc_stream = tokio::select! {
            res = incoming.next() => {
                match res {
                    Some(s) => s.map_err(|e| e.into())?,
                    None => {
                        println!("incoming ended");
                        return Ok(());
                    }
                }
            }
            _ = &mut sig =>{
                println!("cancellation triggered");
                break Ok(());
            }
        };

        let svc_cp = svc.clone();
        tokio::spawn(async move {
            if let Err(err) =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                    .serve_connection(hyper_util::rt::TokioIo::new(inc_stream), svc_cp)
                    .await
            {
                if let Some(e) = err.downcast_ref::<hyper::Error>() {
                    if let Some(s) = e.source() {
                        if let Some(ss) = s.downcast_ref::<std::io::Error>() {
                            if ss.kind() == std::io::ErrorKind::ConnectionReset {
                                // client closed connection
                                // TODO: maybe some read is not finished (in dotnet).
                                return;
                            }
                            eprintln!("failed to serve connection io error: {ss:#}");
                        }
                    }
                    eprintln!("failed to serve connection hyper error: {e:#}");
                } else {
                    eprintln!("failed to serve connection general error: {err:#}");
                }
            }
        });
    }
}
