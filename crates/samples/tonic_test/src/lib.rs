use std::{future::Future, path::Path};

use tokio::io::{AsyncRead, AsyncWrite};
use tonic::transport::{Channel, Endpoint, Error};
use yarrp::connector::UdsConnector;
pub mod rustls_client;

tonic::include_proto!("helloworld"); // The string specified here must match the proto package name

pub async fn connect_uds_channel<P: AsRef<Path>>(path: P) -> Result<Channel, Error> {
    let p = path.as_ref();
    let buf = p.to_path_buf();
    Endpoint::try_from("http://[::]:50051")?
        .connect_with_connector(tower::service_fn(move |_| {
            let path_cp = buf.clone();
            async move {
                // Connect to a Uds socket
                let io = UdsConnector::new(path_cp).connect().await?;
                Ok::<_, std::io::Error>(io)
            }
        }))
        .await
}

#[derive(Default)]
pub struct HelloWorldService {}

#[tonic::async_trait]
impl greeter_server::Greeter for HelloWorldService {
    async fn say_hello(
        &self,
        req: tonic::Request<HelloRequest>,
    ) -> Result<tonic::Response<HelloReply>, tonic::Status> {
        let name = req.into_inner().name;
        Ok(tonic::Response::new(HelloReply {
            message: format!("hello {}", name),
        }))
    }
}

pub async fn run_hello_server<I, IO, IE>(
    token: yarrp::CancellationToken,
    incoming: I,
) -> Result<(), tonic::transport::Error>
where
    I: tokio_stream::Stream<Item = Result<IO, IE>> + Unpin + Send,
    IO: AsyncRead + AsyncWrite + tonic::transport::server::Connected + Unpin + Send + 'static,
    IE: Into<yarrp::Error>,
{
    let greeter = HelloWorldService::default();

    // println!("GreeterServer listening on {}", addr);

    tonic::transport::Server::builder()
        .add_service(crate::greeter_server::GreeterServer::new(greeter))
        .serve_with_incoming_shutdown(incoming, async move { token.cancelled().await })
        .await?;
    Ok(())
}

// returns the listener stream and its local addr from os.
pub async fn create_listener_server() -> (tokio::net::TcpListener, std::net::SocketAddr) {
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    // let stream = tokio_stream::wrappers::TcpListenerStream::new(listener);
    (listener, local_addr)
}

async fn basic_test_tonic_client_inoke(ch: Channel) {
    // send request to proxy
    let mut client = crate::greeter_client::GreeterClient::new(ch);
    //let mut client = crate::greeter_client::GreeterClient::connect(dst)
    let request = tonic::Request::new(HelloRequest {
        name: "Tonic".into(),
    });
    let response = client.say_hello(request).await.unwrap();

    println!("RESPONSE={:?}", response);
}

/// Tests tonic client and server with various streams.
pub async fn basic_tonic_test_case<I2, IO2, IE2>(
    client_channel: impl Future<Output = Channel>,
    sv_incoming: impl Future<Output = I2> + Send + 'static,
) where
    I2: tokio_stream::Stream<Item = Result<IO2, IE2>> + Unpin + Send,
    IO2: AsyncRead + AsyncWrite + tonic::transport::server::Connected + Unpin + Send + 'static,
    IE2: Into<yarrp::Error> + Send,
{
    let sv_token = yarrp::CancellationToken::new();
    let sv_token_cp = sv_token.clone();
    let rt = tokio::runtime::Handle::current();
    // Run tonic server
    let sv_h = rt.spawn(async move {
        let ic = sv_incoming.await;
        crate::run_hello_server(sv_token_cp, ic).await.unwrap();
    });

    // wait for server up
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    // send request to proxy
    basic_test_tonic_client_inoke(client_channel.await).await;

    sv_token.cancel();
    sv_h.await.unwrap();
}

/// Tests send client channel -> proxy incoming -> proxy client -> server incoming
pub async fn basic_proxy_test_case<I, IO, IE, I2, IO2, IE2>(
    client_channel: impl Future<Output = Channel>,
    proxy_incoming: impl Future<Output = I> + Send + 'static,
    sv_incoming: impl Future<Output = I2> + Send + 'static,
    proxy_client_conn: impl hyper_util::client::legacy::connect::Connect + Clone + Send + Sync + 'static,
) where
    I: tokio_stream::Stream<Item = Result<IO, IE>> + Unpin + Send,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    IE: Into<yarrp::Error>,
    I2: tokio_stream::Stream<Item = Result<IO2, IE2>> + Unpin + Send,
    IO2: AsyncRead + AsyncWrite + tonic::transport::server::Connected + Unpin + Send + 'static,
    IE2: Into<yarrp::Error> + Send,
{
    let sv_token = yarrp::CancellationToken::new();
    let sv_token_cp = sv_token.clone();
    let rt = tokio::runtime::Handle::current();
    // Run tonic server
    let sv_h = rt.spawn(async move {
        let ic = sv_incoming.await;
        crate::run_hello_server(sv_token_cp, ic).await.unwrap();
    });

    // run proxy route to tonic
    let sv_token_cp2 = sv_token.clone();
    let proxy_h = rt.spawn(async move {
        // let conn = TcpConnector::new(sv_addr); // tonic addr
        let service = yarrp::proxy_service::ProxyService::new(proxy_client_conn);
        yarrp::serve_with_incoming(proxy_incoming.await, service, async move {
            sv_token_cp2.cancelled().await
        })
        .await
        .unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // send request to proxy
    basic_test_tonic_client_inoke(client_channel.await).await;

    sv_token.cancel();
    sv_h.await.unwrap();
    proxy_h.await.unwrap();
}

#[cfg(test)]
pub mod tonic_tests {
    use hyper_util::rt::TokioIo;
    use tonic::transport::Endpoint;
    use yarrp::accept_stream::TcpListenerStream;

    #[tokio::test]
    async fn basic_tcp_tonic_test() {
        // tonic server
        let (sv_l, sv_addr) = crate::create_listener_server().await;

        let client_channel = async {
            // create client channel
            Endpoint::try_from("http://[::]:50051")
                .unwrap()
                .connect_with_connector(tower::service_fn(move |_| {
                    let addr = sv_addr;
                    async move {
                        let io = tokio::net::TcpStream::connect(addr).await?;
                        Ok::<_, std::io::Error>(TokioIo::new(io))
                    }
                }))
                .await
                .unwrap()
        };

        crate::basic_tonic_test_case(client_channel, async { TcpListenerStream::new(sv_l) }).await;
    }
}

#[cfg(test)]
pub mod tests {
    use std::{net::SocketAddr, sync::Arc};

    use hyper_util::rt::TokioIo;
    use tokio::net::TcpListener;
    use tonic::transport::Endpoint;
    use yarrp::accept_stream::TcpListenerStream;

    use crate::create_listener_server;

    #[tokio::test]
    async fn basic_tcp_proxy_test() {
        // proxy server runs on tcp
        let client_channel = async {
            // create client channel
            Endpoint::from_static("http://[::1]:50052")
                .connect()
                .await
                .unwrap()
        };

        let proxy_incoming = async {
            let addr: SocketAddr = "[::1]:50052".parse().unwrap();
            let incoming = TcpListener::bind(&addr).await.unwrap();
            tokio_stream::wrappers::TcpListenerStream::new(incoming)
        };

        let (sv_l, sv_addr) = create_listener_server().await;

        crate::basic_proxy_test_case(
            client_channel,
            proxy_incoming,
            async { TcpListenerStream::new(sv_l) },
            yarrp::connector::TcpConnector::new(sv_addr),
        )
        .await;
    }

    #[tokio::test]
    async fn basic_uds_proxy_test() {
        // proxy server runs on uds
        let test_socket = std::env::temp_dir().join("mytest.sock");
        hyperlocal_with_windows::remove_unix_socket_if_present(test_socket.as_path())
            .await
            .unwrap();

        let client_channel = async {
            // create client channel
            crate::connect_uds_channel(test_socket.as_path())
                .await
                .unwrap()
        };

        let test_socket_cp = test_socket.clone();
        let proxy_incoming =
            async { yarrp::accept_stream::UdsAcceptStream::bind(test_socket_cp).unwrap() };

        let (sv_l, sv_addr) = create_listener_server().await;

        crate::basic_proxy_test_case(
            client_channel,
            proxy_incoming,
            async { TcpListenerStream::new(sv_l) },
            yarrp::connector::TcpConnector::new(sv_addr),
        )
        .await;
    }

    #[tokio::test]
    async fn basic_rustls_proxy_test() {
        // Create proxy server stream
        let (proxy_l, proxy_addr) = create_listener_server().await;
        // Build TLS configuration.
        let (mut server_config, certs) = yarrp_rustls::test_util::load_test_server_config();
        server_config.alpn_protocols = vec![b"h2".to_vec()]; // b"http/1.1".to_vec(), b"http/1.0".to_vec()
        let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

        let client_authorizer =
            crate::rustls_client::get_common_name_authorizor("YARRP".to_string());

        let proxy_incoming = async move {
            yarrp_rustls::accept_stream::RustlsAcceptStream::new(
                proxy_l,
                tls_acceptor,
                Some(client_authorizer),
            )
        };

        let client_channel = async {
            // create client channel that connects via rustls
            Endpoint::try_from("http://[::]:50051")
                .unwrap()
                .connect_with_connector(tower::service_fn(move |_| {
                    let certs_cp = certs.clone();
                    let proxy_addr_cp = proxy_addr;
                    async move {
                        // Connect to tls proxy
                        crate::rustls_client::get_client_stream(certs_cp, proxy_addr_cp)
                            .await
                            .map(|s| TokioIo::new(s))
                    }
                }))
                .await
                .unwrap()
        };

        // tonic server
        let (sv_l, sv_addr) = create_listener_server().await;

        crate::basic_proxy_test_case(
            client_channel,
            proxy_incoming,
            async { TcpListenerStream::new(sv_l) },
            yarrp::connector::TcpConnector::new(sv_addr),
        )
        .await;
    }
}
