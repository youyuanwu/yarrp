// windows cng support

use std::sync::Arc;

use rustls::{
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
}

impl ServerCertResolver {
    fn create_default(contexts: Vec<CertContext>) -> Self {
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
        let certs = chain.into_iter().map(Into::into).collect();
        let key_der = CertifiedKey::new(certs, Arc::new(key));
        // cannot use the with single cert API since we use a custom key provider.
        // make resolver
        ServerCertResolver {
            key: Arc::new(key_der),
        }
    }
}
