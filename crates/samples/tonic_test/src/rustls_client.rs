use std::{net::SocketAddr, sync::Arc};

use rustls_cng::cert::CertContext;
use tokio::net::TcpStream;
use tokio_rustls::{
    rustls::{ClientConfig, RootCertStore},
    TlsConnector,
};
use x509_parser::prelude::FromDer;
use yarrp_rustls::{
    accept_stream::{ClientAuthorizor, LambdaClientAuthorizor},
    cng::ClientCertResolver,
};

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

/// make dummy error
fn make_error() -> yarrp::Error {
    yarrp::Error::from(std::io::Error::from(std::io::ErrorKind::NotFound))
}

/// Client authorizor that checks the subject name strict match. This is used on server acceptor
pub fn get_common_name_authorizor(cn: String) -> Arc<dyn ClientAuthorizor> {
    let client_authorize_cb = move |conn: &tokio_rustls::rustls::ServerConnection| {
        let peer = conn.peer_certificates();
        match peer {
            Some(c) => {
                let cc = c.first().ok_or(make_error())?;
                let cert = x509_parser::certificate::X509Certificate::from_der(cc)
                    .map_err(yarrp::Error::from)?;
                let subjs = cert
                    .1
                    .subject
                    .iter_common_name()
                    .filter_map(|sn| sn.as_str().map_or(None, |s| Some(s.to_string())))
                    .collect::<Vec<_>>();
                // Get alternative names
                // let alt = cert.1.subject_alternative_name().map_or(vec![],|s|{
                //     s.map_or(vec![], |ss|{
                //         ss.value.general_names.iter().map(|n| n.to_string()).collect::<Vec<_>>()
                //     })
                // });
                // subjs.extend(alt);
                println!("subjs : {:?}", subjs);
                match subjs.contains(&cn) {
                    true => Ok(()),
                    false => Err(make_error()),
                }
            }
            None => Err(make_error()),
        }
    };
    Arc::new(LambdaClientAuthorizor::new(client_authorize_cb))
}
