mod service;
#[cfg(target_os = "windows")]
mod windows;

pub use service::AutoOpenService;
