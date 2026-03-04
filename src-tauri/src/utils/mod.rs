//! 工具模块
//!
//! 提供跨平台的通用辅助函数和 trait。

pub mod fs;

// 公开常用函数以便直接使用
pub use fs::{ensure_dir_exists, is_path_writable};
