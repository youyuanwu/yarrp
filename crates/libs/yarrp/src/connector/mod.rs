// This mod implementes various Connector
// that is used by hyper-util legacy client.
cfg_if::cfg_if! {
  if #[cfg(target_os = "windows")] {
mod win_uds;
pub use win_uds::UdsConnector;
  }else{
mod uds;
  }
}

mod tcp;
pub use tcp::TcpConnector;
