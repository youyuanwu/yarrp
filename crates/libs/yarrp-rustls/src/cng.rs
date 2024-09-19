// windows cng support

use std::sync::Arc;

use rustls::{
    client::ResolvesClientCert,
    pki_types::CertificateDer,
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};
use rustls_cng::{cert::CertContext, signer::CngSigningKey};

/// Used for rustls with_cert_resolver.
/// always return the same key.
/// It is possible to return different key based on client.
#[derive(Debug)]
pub struct ServerCertResolver {
    key: Arc<CertifiedKey>,
}

impl ResolvesServerCert for ServerCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        println!(
            "Tls Client hello server name: {:?}",
            client_hello.server_name()
        );
        Some(self.key.clone())
    }
}

impl ServerCertResolver {
    pub fn try_from_certs(certs: Vec<CertContext>) -> Result<Self, crate::Error> {
        assert!(!certs.is_empty());
        Ok(Self::create_default(certs))
    }

    pub fn get_certs(&self) -> Vec<CertificateDer<'static>> {
        self.key.cert.clone()
    }
}

impl ServerCertResolver {
    fn create_default(contexts: Vec<CertContext>) -> Self {
        Self {
            key: create_key(contexts),
        }
    }
}

// client cert resolver that always returns the same cert
#[derive(Debug)]
pub struct ClientCertResolver {
    key: Arc<CertifiedKey>,
}

impl ClientCertResolver {
    pub fn try_from_certs(certs: Vec<CertContext>) -> Result<Self, crate::Error> {
        Ok(Self {
            key: create_key(certs),
        })
    }
}

impl ResolvesClientCert for ClientCertResolver {
    fn resolve(
        &self,
        _root_hint_subjects: &[&[u8]],
        _sigschemes: &[rustls::SignatureScheme],
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        // TODO: hints should be parsed but need to use 3rd party lib.
        //println!("cert hints: {:?}", _root_hint_subjects.iter().map(|s| String::from_utf8_lossy(s).into_owned()).collect::<Vec<_>>());
        Some(self.key.clone())
    }

    fn has_certs(&self) -> bool {
        true
    }
}

fn create_key(contexts: Vec<CertContext>) -> Arc<CertifiedKey> {
    // attempt to acquire a private key and construct CngSigningKey
    let (context, key) = contexts
        .into_iter()
        .find_map(|ctx| {
            // find the first cert
            let key = ctx.acquire_key().ok()?;
            CngSigningKey::new(key).ok().map(|key| (ctx, key))
        })
        .unwrap();

    println!("Key alg group: {:?}", key.key().algorithm_group());
    println!("Key alg: {:?}", key.key().algorithm());

    // attempt to acquire a full certificate chain
    let chain = context.as_chain_der().ok().unwrap();
    let certs = chain.into_iter().map(Into::into).collect::<Vec<_>>();
    Arc::new(CertifiedKey::new(certs, Arc::new(key)))
}
