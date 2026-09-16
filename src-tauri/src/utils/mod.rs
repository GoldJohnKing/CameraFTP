// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 工具模块
//!
//! 提供跨平台的通用辅助函数和 trait。

pub mod fs;
pub(crate) mod batch_state;
pub(crate) mod task_worker;

// 测试共享辅助（仅测试编译）：ai_edit / color_grading 测试模块共用的
// wait_until / event_collector，避免两份重复拷贝。
#[cfg(test)]
pub(crate) mod test_support;

// Android JNI 引导助手（模块整体仅在 Android 编译）
#[cfg(target_os = "android")]
pub mod jni;

// 公开常用函数以便直接使用
pub use fs::{is_path_writable, wait_for_file_ready};

/// Percent-decode a URI path component (handles UTF-8 encoded file paths).
#[cfg(target_os = "windows")]
pub fn percent_decode(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_roundtrips_frontend_encodeuricomponent_output() {
        // 真实 Windows 路径：盘符冒号、反斜杠、空格、括号、中文文件名。
        // 手工模拟 JS encodeURIComponent 的输出（其不转义 A-Za-z0-9 与 -_.!~*'()）：
        //   ':' → %3A、'\' → %5C、空格 → %20、'(' ')' '.' 保留原样、
        //   '新' → UTF-8 %E6%96%B0、'建' → UTF-8 %E5%BB%BA
        let original = r"C:\photos\新建 (1).jpg";
        let encoded = r"C%3A%5Cphotos%5C%E6%96%B0%E5%BB%BA%20(1).jpg";

        let decoded = percent_decode(encoded);
        assert_eq!(decoded, original, "percent_decode must invert encodeURIComponent");
        // 与 image-preview handler 的用法一致：PathBuf::from(decoded).to_string_lossy()
        // 必须无损还原（覆盖缓存键与磁盘路径的一致性）
        assert_eq!(
            std::path::PathBuf::from(&decoded).to_string_lossy(),
            original
        );
    }
}
