#[tokio::main]
async fn main() -> Result<(), yarrp_openssl::Error> {
    let sv_l = tokio::net::TcpListener::bind("127.0.0.1:5047")
        .await
        .map_err(yarrp_openssl::Error::from)?;
    let sv_tls_acceptor = tonic_openssl_test::get_test_openssl_acceptor(true);
    let sv_incoming = yarrp_openssl::accept_stream::OpensslAcceptStream::new(sv_l, sv_tls_acceptor);

    let token = yarrp::CancellationToken::new();
    tonic_test::run_hello_server(token, sv_incoming).await?;
    Ok(())
}
