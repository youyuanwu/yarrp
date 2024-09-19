pub mod openssl_helper;

#[cfg(test)]
mod tests {
    use yarrp::accept_stream::TcpListenerStream;

    #[tokio::test]
    async fn basic_openssl_proxy_test() {
        // Create proxy server stream
        let (proxy_l, proxy_addr) = tonic_test::create_listener_server().await;

        let cert = crate::openssl_helper::get_test_cert_path();

        let client_channel = async {
            crate::openssl_helper::connect_openssl_channel(cert, proxy_addr)
                .await
                .unwrap()
        };

        // build openssl proxy server
        let mut acceptor =
            openssl::ssl::SslAcceptor::mozilla_intermediate(openssl::ssl::SslMethod::tls())
                .unwrap();
        acceptor
            .set_private_key_file(
                crate::openssl_helper::get_test_key_path(),
                openssl::ssl::SslFiletype::PEM,
            )
            .unwrap();
        acceptor
            .set_certificate_chain_file(crate::openssl_helper::get_test_cert_path())
            .unwrap();
        let tls_acceptor = acceptor.build();

        let proxy_incoming =
            async { yarrp_openssl::accept_stream::OpensslAcceptStream::new(proxy_l, tls_acceptor) };

        // tonic server
        let (sv_l, sv_addr) = tonic_test::create_listener_server().await;

        tonic_test::basic_test_case(
            client_channel,
            proxy_incoming,
            TcpListenerStream::new(sv_l),
            sv_addr,
        )
        .await;
    }
}
