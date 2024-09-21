pub mod openssl_helper;

use openssl::ssl::{SslAcceptor, SslVerifyMode, SslVersion};

pub fn get_test_openssl_acceptor(skip_verify: bool) -> SslAcceptor {
    let mut acceptor =
        openssl::ssl::SslAcceptor::mozilla_intermediate(openssl::ssl::SslMethod::tls()).unwrap();
    acceptor
        .set_private_key_file(
            crate::openssl_helper::get_test_key_path(),
            openssl::ssl::SslFiletype::PEM,
        )
        .unwrap();
    let cert = crate::openssl_helper::get_test_cert_path();
    acceptor.set_certificate_chain_file(cert.as_path()).unwrap();
    acceptor.set_ca_file(cert.as_path()).unwrap();
    acceptor
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .unwrap();
    // This is for client
    // acceptor.set_alpn_protos(b"\x02h2").unwrap();
    // Set server alpn to http2. This is required for dotnet client.
    acceptor.set_alpn_select_callback(|_sslref, _cli| Ok(b"h2"));
    if skip_verify {
        acceptor.set_verify(SslVerifyMode::NONE);
    } else {
        // currently verify only returnes true.
        // TODO: generate fix name cert, or integrate with Windows cert export.
        let verify = |preverify_ok: bool, ctx: &mut openssl::x509::X509StoreContextRef| {
            if !preverify_ok {
                // cert has problem
                let e = ctx.error();
                println!("verify failed : {}", e);
                return false;
            }
            // further check subject name.
            let ch = ctx.chain();
            if ch.is_none() {
                return false;
            }
            let ch = ch.unwrap();
            let leaf = ch.iter().last();
            if leaf.is_none() {
                return false;
            }
            let leaf = leaf.unwrap();
            let sn = leaf.subject_name();
            sn.entries().any(|e| {
                let name = String::from_utf8_lossy(e.data().as_slice());
                println!("sn: {}", name);
                true
            })
        };
        // Requires peer cert and verify callback to pass.
        acceptor.set_verify_callback(
            SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT,
            verify,
        );
    }
    acceptor.build()
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use tonic::transport::Channel;
    use yarrp::accept_stream::TcpListenerStream;

    use crate::{
        get_test_openssl_acceptor,
        openssl_helper::{get_test_cert_path, get_test_key_path, get_test_openss_connector},
    };

    async fn get_test_client_channel(addr: SocketAddr) -> Channel {
        let cert = crate::openssl_helper::get_test_cert_path();
        let key = crate::openssl_helper::get_test_key_path();
        crate::openssl_helper::connect_openssl_channel(cert, key, addr)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn basic_tonic_openssl_test() {
        // tonic server with ssl
        let (sv_l, sv_addr) = tonic_test::create_listener_server().await;
        let client_channel = async { get_test_client_channel(sv_addr).await };

        let sv_tls_acceptor = get_test_openssl_acceptor(false);
        let sv_incoming =
            async { yarrp_openssl::accept_stream::OpensslAcceptStream::new(sv_l, sv_tls_acceptor) };

        tonic_test::basic_tonic_test_case(client_channel, sv_incoming).await;
    }

    #[tokio::test]
    async fn basic_openssl_proxy_test() {
        // Create proxy server stream
        let (proxy_l, proxy_addr) = tonic_test::create_listener_server().await;

        let user_client_channel = async { get_test_client_channel(proxy_addr).await };

        // build openssl proxy server
        let tls_acceptor = get_test_openssl_acceptor(false);

        let proxy_incoming =
            async { yarrp_openssl::accept_stream::OpensslAcceptStream::new(proxy_l, tls_acceptor) };

        // tonic server
        let (sv_l, sv_addr) = tonic_test::create_listener_server().await;
        let tonic_server_incoming = async { TcpListenerStream::new(sv_l) };
        let proxy_client = yarrp::connector::TcpConnector::new(sv_addr);

        tonic_test::basic_proxy_test_case(
            user_client_channel,
            proxy_incoming,
            tonic_server_incoming,
            proxy_client,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_double_openssl_proxy_test() {
        // Tests tonic server client runs on openssl and proxy server client runs on openssl too.
        // Create proxy server stream
        let (proxy_l, proxy_addr) = tonic_test::create_listener_server().await;

        let user_client_channel = async { get_test_client_channel(proxy_addr).await };

        // build openssl proxy server
        let proxy_tls_acceptor = get_test_openssl_acceptor(false);
        let proxy_incoming = async {
            yarrp_openssl::accept_stream::OpensslAcceptStream::new(proxy_l, proxy_tls_acceptor)
        };

        // tonic server with ssl
        let (sv_l, sv_addr) = tonic_test::create_listener_server().await;
        let sv_tls_acceptor = get_test_openssl_acceptor(false);
        let tonic_server_incoming =
            async { yarrp_openssl::accept_stream::OpensslAcceptStream::new(sv_l, sv_tls_acceptor) };

        // proxy client with ssl
        let conn_inner =
            get_test_openss_connector(get_test_cert_path(), get_test_key_path()).unwrap();
        let proxy_client = yarrp_openssl::connector::OpensslConnector::new(conn_inner, sv_addr);

        tonic_test::basic_proxy_test_case(
            user_client_channel,
            proxy_incoming,
            tonic_server_incoming,
            proxy_client,
        )
        .await;
    }
}
