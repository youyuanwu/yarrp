// reference: https://github.com/stefansundin/hyper-reverse-proxy/tree/master

use std::future::Future;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_stream::StreamExt;

pub use std::error::Error as StdError;
pub use tokio_util::sync::CancellationToken;
pub type Error = Box<dyn StdError + Send + Sync>;

// TODO: support other http body for service.
/// Serves the proxy server on incoming streams.
/// Pipe all http requests into svc. Wait for signal shutdown.
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
                    Some(s) => {
                        match s{
                            Ok(ss) => ss,
                            Err(e) => {
                                println!("incoming has error, skip. {:?}", e.into());
                                continue;
                            },
                        }
                    },
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
