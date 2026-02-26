mod service;
#[cfg(target_os = "windows")]
pub mod windows;

pub use service::AutoOpenService;
