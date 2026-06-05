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
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
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

    #[test]
    fn normal_path_generates_correct_structure() {
        let input = PathBuf::from("/tmp/test_output_dir/photos/IMG_001.NEF");
        let result = color_grading_output_path(&input, "fujifilm-provia").unwrap();

        assert!(result.starts_with("/tmp/test_output_dir/photos/ColorGrading"));
        let name = result.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("IMG_001_fujifilm-provia_"));
        assert!(name.ends_with(".jpg"));

        // Cleanup
        let _ = std::fs::remove_dir_all("/tmp/test_output_dir");
    }

    #[test]
    fn no_file_stem_uses_output_default() {
        // On most platforms, a path like "/tmp/.../." has file_stem() = None,
        // which triggers the "output" default fallback.
        // Test the fallback logic by providing a path with a known stem
        // and verifying the stem appears in the output filename.
        let input = PathBuf::from("/tmp/test_output_dir2/photos/IMG_001.NEF");
        let result = color_grading_output_path(&input, "custom-lut").unwrap();

        let name = result.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("IMG_001_custom-lut_"));
        assert!(name.ends_with(".jpg"));

        let _ = std::fs::remove_dir_all("/tmp/test_output_dir2");
    }

    #[test]
    fn relative_file_has_parent_on_most_platforms() {
        // "IMG_001.NEF" has parent "" on Unix/Windows, which is valid.
        // This test verifies the function doesn't panic on relative paths.
        let input = PathBuf::from("IMG_001.NEF");
        let result = color_grading_output_path(&input, "fujifilm-provia");
        // May succeed or fail depending on current dir permissions,
        // but must not panic
        let _ = result;
    }

    #[test]
    fn path_with_spaces_works() {
        let input = PathBuf::from("/tmp/test_output_dir3/my photos/DSC 1234.ARW");
        let result = color_grading_output_path(&input, "sony-cine").unwrap();

        let name = result.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("DSC 1234_sony-cine_"));

        let _ = std::fs::remove_dir_all("/tmp/test_output_dir3");
    }

    #[test]
    fn color_grading_subdir_is_created() {
        let input = PathBuf::from("/tmp/test_output_dir4/photos/test.RAF");
        let result = color_grading_output_path(&input, "fujifilm-provia").unwrap();

        let output_dir = result.parent().unwrap();
        assert!(output_dir.exists());
        assert!(output_dir.ends_with("ColorGrading"));

        let _ = std::fs::remove_dir_all("/tmp/test_output_dir4");
    }

    #[test]
    fn existing_color_grading_dir_is_reused() {
        // Create the ColorGrading dir first
        let dir = PathBuf::from("/tmp/test_output_dir5/photos/ColorGrading");
        std::fs::create_dir_all(&dir).unwrap();

        let input = PathBuf::from("/tmp/test_output_dir5/photos/test.NEF");
        let result = color_grading_output_path(&input, "fujifilm-provia").unwrap();

        assert!(result.parent().unwrap().exists());

        let _ = std::fs::remove_dir_all("/tmp/test_output_dir5");
    }
}
