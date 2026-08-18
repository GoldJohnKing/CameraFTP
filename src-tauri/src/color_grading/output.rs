// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Generate the output path for a color-graded JPEG.
///
/// Convention: `<input_parent>/ColorGrading/<stem>_<preset_id>_<timestamp>.jpg`
/// Also ensures the `ColorGrading/` subdirectory exists (creates it if needed).
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
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let output_dir = parent.join("ColorGrading");
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| AppError::ColorGradingError(format!("Failed to create output dir: {}", e)))?;
    let output_name = format!("{}_{}_{}.jpg", stem, preset_id, timestamp);
    Ok(output_dir.join(output_name))
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
}
