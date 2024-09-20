use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use openssl::ssl::{SslAcceptor, SslConnector, SslFiletype, SslMethod, SslVerifyMode};
use tonic::transport::Channel;

/// Load the certs from test dir and create acceptor
pub fn get_openssl_acceptor() -> SslAcceptor {
    let curr_dir = std::env::current_dir().unwrap();
    let root_dir = curr_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    // println!("{:?}",root_dir);
    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    acceptor
        .set_private_key_file(root_dir.join("tests/key.pem"), SslFiletype::PEM)
        .unwrap();
    acceptor
        .set_certificate_chain_file(root_dir.join("tests/cert.pem"))
        .unwrap();
    acceptor.build()
}

pub fn get_test_openss_connector<P: AsRef<Path>>(
    cert_path: P,
    key_path: P,
) -> Result<SslConnector, yarrp::Error> {
    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_ca_file(cert_path.as_ref()).unwrap();
    connector.set_certificate_file(cert_path.as_ref(), SslFiletype::PEM)?;
    connector.set_private_key_file(key_path.as_ref(), SslFiletype::PEM)?;
    connector.set_verify(SslVerifyMode::NONE); // TODO
    Ok(connector.build())
}

pub async fn connect_openssl_channel<P: AsRef<Path>>(
    cert_path: P,
    key_path: P,
    addr: SocketAddr,
) -> Result<Channel, yarrp::Error> {
    let connector = get_test_openss_connector(cert_path, key_path)?;

    tonic::transport::Endpoint::try_from("http://[::]:50051")?
        .connect_with_connector(tower::service_fn(move |_| {
            let ssl = connector
                .configure()
                .unwrap()
                .into_ssl("localhost")
                .unwrap();
            async move {
                // Connect and get ssl stream
                let io = tokio::net::TcpStream::connect(&addr).await.unwrap();
                let mut stream = yarrp_openssl::accept_stream::SslStream::new(ssl, io).unwrap();
                std::pin::Pin::new(&mut stream).connect().await.unwrap();
                Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
            }
        }))
        .await
        .map_err(yarrp::Error::from)
}

fn get_root_dir() -> PathBuf {
    let curr_dir = std::env::current_dir().unwrap();
    let root_dir = curr_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    root_dir.to_path_buf()
}

pub fn get_test_cert_path() -> PathBuf {
    get_root_dir().join("tests/cert.pem")
}

pub fn get_test_key_path() -> PathBuf {
    get_root_dir().join("tests/key.pem")
}

#[cfg(test)]
mod tests {
    use super::get_openssl_acceptor;

    #[test]
    fn cert_load_test() {
        let _acceptor = get_openssl_acceptor();
    }
}
