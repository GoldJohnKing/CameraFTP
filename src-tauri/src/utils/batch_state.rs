// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

/// Tracks aggregate progress for a batch of tasks processed sequentially.
/// Shared by AI edit and color grading worker loops.
#[derive(Default)]
pub(crate) struct BatchState {
    pub completed_count: u32,
    pub failed_count: u32,
    pub failed_files: Vec<String>,
    pub output_files: Vec<String>,
}

impl BatchState {
    pub fn processed_count(&self) -> u32 {
        self.completed_count + self.failed_count
    }

    pub fn reset(&mut self) {
        self.completed_count = 0;
        self.failed_count = 0;
        self.failed_files.clear();
        self.output_files.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_completed_and_failed() {
        let mut state = BatchState::default();
        state.completed_count += 1;
        state.output_files.push("/out/a.jpg".to_string());
        assert_eq!(state.processed_count(), 1);

        state.failed_count += 1;
        state.failed_files.push("bad.jpg".to_string());
        assert_eq!(state.processed_count(), 2);
    }

    #[test]
    fn reset_clears_everything() {
        let mut state = BatchState::default();
        state.completed_count = 5;
        state.failed_count = 2;
        state.failed_files.push("x.jpg".to_string());
        state.output_files.push("/out/y.jpg".to_string());
        state.reset();
        assert_eq!(state.processed_count(), 0);
        assert!(state.failed_files.is_empty());
        assert!(state.output_files.is_empty());
    }
}
