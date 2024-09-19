use std::{net::SocketAddr, sync::Arc};

use rustls_cng::cert::CertContext;
use tokio::net::TcpStream;
use tokio_rustls::{
    rustls::{ClientConfig, RootCertStore},
    TlsConnector,
};
use yarrp_rustls::cng::ClientCertResolver;

// TODO: refactor into the yarrp-rustls crate
/// Create client tls stream, uses the certs as both root and client cert.
/// i.e. configures mutual auth with selfsigned cert.
pub async fn get_client_stream(
    root_certs: Vec<CertContext>,
    sv_addr: SocketAddr,
) -> std::io::Result<tokio_rustls::client::TlsStream<tokio::net::TcpStream>> {
    let mut root_store = RootCertStore::empty();
    root_store
        .add(root_certs.first().unwrap().as_der().into())
        .unwrap();
    let client_cert = ClientCertResolver::try_from_certs(root_certs).unwrap();
    let client_config =
        ClientConfig::builder_with_provider(Arc::new(rustls_symcrypt::default_symcrypt_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(root_store)
            .with_client_cert_resolver(Arc::new(client_cert));

    let connector = TlsConnector::from(Arc::new(client_config));
    let stream = TcpStream::connect(&sv_addr).await.unwrap();
    let domain = tokio_rustls::rustls::pki_types::ServerName::try_from("localhost")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid dnsname"))
        .unwrap()
        .to_owned();
    connector.connect(domain, stream).await
}
