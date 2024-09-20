pub mod openssl_helper;

#[cfg(test)]
mod tests {
    use openssl::ssl::SslVerifyMode;
    use yarrp::accept_stream::TcpListenerStream;

    #[tokio::test]
    async fn basic_openssl_proxy_test() {
        // Create proxy server stream
        let (proxy_l, proxy_addr) = tonic_test::create_listener_server().await;

        let cert = crate::openssl_helper::get_test_cert_path();
        let key = crate::openssl_helper::get_test_key_path();

        let client_channel = async {
            crate::openssl_helper::connect_openssl_channel(cert, key, proxy_addr)
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
        let cert = crate::openssl_helper::get_test_cert_path();
        acceptor.set_certificate_chain_file(cert.as_path()).unwrap();
        acceptor.set_ca_file(cert.as_path()).unwrap();
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
