// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Generate the output path for a color-graded JPEG.
///
/// Convention: `<input_parent>/ColorGrading/<stem>_<preset_id>_<timestamp>.jpg`
/// Also ensures the `ColorGrading/` subdirectory exists (creates it if needed).
/// If the primary name is already occupied, `_1`..`_99` suffixes are tried —
/// the same collision semantics as `ai_edit::service::write_edited_image`.
pub fn color_grading_output_path(input_path: &Path, preset_id: &str) -> Result<PathBuf, AppError> {
    let parent = input_path
        .parent()
        .ok_or_else(|| AppError::ColorGradingError("No parent directory".into()))?;
    let stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".into());
    // 使用 UTC 而非 Local：部分 Android 设备缺少 tzdata，chrono::Local 会 panic
    // （ai_edit/service.rs 的 chrono_now_string 出于同样原因使用 Utc）。
    // 文件名时间戳只需保证唯一性/可排序，UTC 完全满足。
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let output_dir = parent.join("ColorGrading");
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| AppError::ColorGradingError(format!("Failed to create output dir: {}", e)))?;
    // 冲突保护：实际的 JPEG 写入发生在 C++ FFI（process_file_with_lut）内部，
    // 无法像 ai_edit 的 write_edited_image 那样用 create_new 排他写入，因此在
    // 派生文件名时预检查同名文件是否已存在。worker 串行处理任务，同一秒内的
    // 第二次导出会看到第一次已写入的文件，从而改用带后缀的名字而不是覆盖它。
    let output_name = pick_unique_output_name(&stem, preset_id, &timestamp, |name| {
        output_dir.join(name).exists()
    })?;
    Ok(output_dir.join(output_name))
}

/// Pick a non-colliding output file name: use the primary name when it is
/// free, otherwise try `_1`..`_99` suffixes, and error on exhaustion —
/// mirroring `ai_edit::service::write_edited_image`. `is_taken` decides
/// whether a candidate name is already occupied (pure seam so tests can
/// exercise the retry/exhaustion logic without touching the filesystem).
fn pick_unique_output_name(
    stem: &str,
    preset_id: &str,
    timestamp: &str,
    is_taken: impl Fn(&str) -> bool,
) -> Result<String, AppError> {
    let primary_name = format!("{}_{}_{}.jpg", stem, preset_id, timestamp);
    if !is_taken(&primary_name) {
        return Ok(primary_name);
    }
    for i in 1u32..=99 {
        let retry_name = format!("{}_{}_{}_{}.jpg", stem, preset_id, timestamp, i);
        if !is_taken(&retry_name) {
            return Ok(retry_name);
        }
    }
    Err(AppError::ColorGradingError(
        "Failed to generate output file name: too many file name collisions".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_input(dir: &tempfile::TempDir, relative: &str) -> PathBuf {
        let input = dir.path().join(relative);
        std::fs::create_dir_all(input.parent().unwrap()).unwrap();
        std::fs::write(&input, "").unwrap();
        input
    }

    #[test]
    fn normal_path_generates_correct_structure() {
        let dir = tempfile::tempdir().unwrap();
        let input = create_input(&dir, "photos/IMG_001.NEF");

        let result = color_grading_output_path(&input, "fujifilm-provia").unwrap();

        assert!(result.starts_with(dir.path().join("photos/ColorGrading")));
        let name = result.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("IMG_001_fujifilm-provia_"));
        assert!(name.ends_with(".jpg"));
    }

    #[test]
    fn no_file_stem_uses_output_default() {
        let dir = tempfile::tempdir().unwrap();
        // A path ending in `..` has a parent directory but no file stem
        // (`file_name()`/`file_stem()` return None), exercising the
        // `"output"` fallback branch.
        let input = dir.path().join("photos/..");

        let result = color_grading_output_path(&input, "custom-lut").unwrap();

        let name = result.file_name().unwrap().to_string_lossy();
        assert!(
            name.starts_with("output_custom-lut_"),
            "expected output stem fallback, got: {}",
            name
        );
        assert!(name.ends_with(".jpg"));
    }

    #[test]
    fn relative_file_has_parent_on_most_platforms() {
        let input = PathBuf::from("IMG_001.NEF");
        let result = color_grading_output_path(&input, "fujifilm-provia");
        // May succeed or fail depending on current dir permissions, but must not panic
        let _ = result;
    }

    #[test]
    fn path_with_spaces_works() {
        let dir = tempfile::tempdir().unwrap();
        let input = create_input(&dir, "my photos/DSC 1234.ARW");

        let result = color_grading_output_path(&input, "sony-cine").unwrap();

        let name = result.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("DSC 1234_sony-cine_"));
    }

    #[test]
    fn color_grading_subdir_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let input = create_input(&dir, "photos/test.RAF");

        let result = color_grading_output_path(&input, "fujifilm-provia").unwrap();

        let output_dir = result.parent().unwrap();
        assert!(output_dir.exists());
        assert!(output_dir.ends_with("ColorGrading"));
    }

    #[test]
    fn existing_color_grading_dir_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("photos/ColorGrading")).unwrap();
        let input = create_input(&dir, "photos/test.NEF");

        let result = color_grading_output_path(&input, "fujifilm-provia").unwrap();

        assert!(result.parent().unwrap().exists());
    }

    #[test]
    fn first_write_name_is_unchanged() {
        // 主名未被占用时，保持默认命名格式，不追加任何后缀。
        let name = pick_unique_output_name(
            "IMG_001",
            "fujifilm-provia",
            "20260818_120000",
            |_| false,
        )
        .unwrap();
        assert_eq!(name, "IMG_001_fujifilm-provia_20260818_120000.jpg");
    }

    #[test]
    fn collision_produces_suffixed_name() {
        // 主名被占用 → 追加 _1；_1 也被占用 → 追加 _2（与 ai_edit 的重试语义一致）。
        let taken: Vec<&str> = vec![
            "IMG_001_fujifilm-provia_20260818_120000.jpg",
            "IMG_001_fujifilm-provia_20260818_120000_1.jpg",
        ];
        let name = pick_unique_output_name(
            "IMG_001",
            "fujifilm-provia",
            "20260818_120000",
            |n| taken.contains(&n),
        )
        .unwrap();
        assert_eq!(name, "IMG_001_fujifilm-provia_20260818_120000_2.jpg");
    }

    #[test]
    fn exhaustion_errors() {
        // 1..=99 全部被占用时报错，与 ai_edit 的 "too many file name collisions" 对齐。
        let result = pick_unique_output_name(
            "IMG_001",
            "fujifilm-provia",
            "20260818_120000",
            |_| true,
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("too many file name collisions"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn output_path_avoids_existing_file() {
        // 集成点检查：实际的文件写入发生在 C++ FFI 内部，这里模拟第一次导出
        // 完成后（主名已存在于磁盘），同一秒内再次派生路径，不得复用已存在
        // 的名字（否则会覆盖前一次导出）。无论两次调用是否跨秒，断言都成立：
        // 同秒 → 带后缀的新名字；跨秒 → 新时间戳的名字。
        let dir = tempfile::tempdir().unwrap();
        let input = create_input(&dir, "photos/IMG_001.NEF");

        let first = color_grading_output_path(&input, "fujifilm-provia").unwrap();
        std::fs::write(&first, "existing").unwrap();

        let second = color_grading_output_path(&input, "fujifilm-provia").unwrap();
        assert_ne!(
            first, second,
            "must not return a name that already exists on disk"
        );
        assert!(
            !second.exists(),
            "returned path must be free: {}",
            second.display()
        );
    }
}
