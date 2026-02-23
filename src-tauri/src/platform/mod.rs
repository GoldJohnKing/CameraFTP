mod types;
pub use types::{StorageInfo, PermissionStatus, ServerStartCheckResult};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "android")]
pub mod android;
