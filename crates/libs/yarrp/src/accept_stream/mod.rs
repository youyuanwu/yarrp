mod rustls;

// Tcp accept stream
pub use tokio_stream::wrappers::TcpListenerStream;

mod uds;
//pub use uds::make_uds_accept_stream;
//pub use uds_windows::UnixListener;
pub use uds::UdsAcceptStream;

/// Ssl accept stream using rustls
/// TODO: feature switch
pub use rustls::RustlsAcceptStream;

// TODO: openssl
