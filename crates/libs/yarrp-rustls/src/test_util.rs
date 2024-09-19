use std::{path::PathBuf, process::Command, sync::Arc};

use rustls_cng::{
    cert::CertContext,
    store::{CertStore, CertStoreType},
};
use rustls_symcrypt::default_symcrypt_provider;

type CertHash = [u8; 20];

/// returns the server config for hyper server
/// and the context for client to use keys.
pub fn load_test_server_config() -> (rustls::ServerConfig, Vec<CertContext>) {
    let hash = string_to_hash_bytes(get_test_cert_hash());
    let certs = find_certs_by_hash(hash).unwrap();

    let contexts_copy = certs.clone();
    let cert_resolver = crate::cng::ServerCertResolver::try_from_certs(certs).unwrap();

    let prov = Arc::new(default_symcrypt_provider());

    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(cert_resolver.get_certs());
    let verifier =
        rustls::server::WebPkiClientVerifier::builder_with_provider(roots.into(), prov.clone())
            .build()
            .unwrap();

    let config = rustls::ServerConfig::builder_with_provider(prov)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_cert_resolver(Arc::new(cert_resolver));
    (config, contexts_copy)
}

pub fn find_certs_by_hash(hash: CertHash) -> Result<Vec<CertContext>, crate::Error> {
    let store = CertStore::open(CertStoreType::CurrentUser, "My")?;
    // find test cert
    // find by subject name is somehow not working.
    //let contexts = store.find_by_subject_name("CN=YYDEV").unwrap();
    let certs = store.find_by_sha1(hash)?;
    Ok(certs)
}

fn get_test_cert_hash() -> String {
    let output = Command::new("pwsh.exe")
      .args(["-Command", "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value YARRP-Test | Select-Object -ExpandProperty Thumbprint -First 1"]).
      output().expect("Failed to execute command");
    assert!(output.status.success());
    let mut s = String::from_utf8(output.stdout).unwrap();
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    };
    s
}

fn string_to_hash_bytes(hash: String) -> CertHash {
    let mut hash_array: [u8; 20] = [0; 20];
    hex::decode_to_slice(hash.as_bytes(), &mut hash_array).expect("Decoding failed");
    hash_array
}

pub fn get_test_socket_path() -> PathBuf {
    let sock = "my.sock";
    std::env::temp_dir().join(sock)
}
