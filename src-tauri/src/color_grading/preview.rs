// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::AppError;
use super::ffi::{RaPreviewSession, RawAlchemyLib};
use super::lut_data;
use super::presets::find_preset;

const PREVIEW_JPEG_QUALITY: i32 = 80;

struct ActiveSession {
    session: RaPreviewSession,
    image_path: String,
    preview_output_path: PathBuf,
    enable_lens_correction: bool,
}

pub struct ColorGradingPreviewState {
    inner: Mutex<Option<ActiveSession>>,
}

impl ColorGradingPreviewState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn begin(
        &self,
        image_path: &str,
        lensfun_db_path: Option<&str>,
    ) -> Result<(), AppError> {
        let lib = RawAlchemyLib::get()?;
        let input_path = Path::new(image_path);

        let mut guard = self.inner.lock().await;

        if let Some(active) = guard.take() {
            tracing::info!(old_image = %active.image_path, "Ending previous preview session");
            end_session_internal(&lib, active);
        }

        tracing::info!(image = image_path, "Beginning preview session (decoding RAW)...");

        let session = tokio::task::spawn_blocking({
            let input_path = input_path.to_path_buf();
            let lensfun = lensfun_db_path.map(String::from);
            move || {
                lib.begin_preview_session(
                    &input_path,
                    true,
                    lensfun.as_deref(),
                )
            }
        })
        .await
        .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

        let preview_dir = std::env::temp_dir().join("CameraFTP").join("preview");
        tokio::fs::create_dir_all(&preview_dir).await
            .map_err(|e| AppError::ColorGradingError(format!("Failed to create preview dir: {}", e)))?;

        let preview_output_path = preview_dir.join(format!("preview_{}.jpg", session.ptr as usize));

        tracing::info!(image = image_path, "Preview session ready");

        *guard = Some(ActiveSession {
            session,
            image_path: image_path.to_string(),
            preview_output_path,
            enable_lens_correction: true,
        });

        Ok(())
    }

    pub async fn apply(
        &self,
        lut_id: &str,
        enable_lens_correction: bool,
        use_auto_exposure: bool,
        metering_mode: &str,
        manual_ev: f32,
    ) -> Result<String, AppError> {
        let lib = RawAlchemyLib::get()?;
        let preset = find_preset(lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;
        let lut_data = lut_data::get_lut_data(&preset.id)?;

        let lensfun_db_path = super::resources::get_resources()
            .ok()
            .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

        let (session_addr, output_path) = {
            let mut guard = self.inner.lock().await;
            let active = guard.as_mut()
                .ok_or_else(|| AppError::ColorGradingError("No active preview session".into()))?;

            if enable_lens_correction != active.enable_lens_correction {
                tracing::info!(
                    from = active.enable_lens_correction,
                    to = enable_lens_correction,
                    "Toggling lens correction"
                );
                let session = RaPreviewSession { ptr: active.session.ptr };
                lib.toggle_lens_correction(&session, enable_lens_correction, lensfun_db_path.as_deref())?;
                active.enable_lens_correction = enable_lens_correction;
            }

            (active.session.ptr as usize, active.preview_output_path.clone())
        };
        let output_path_for_url = output_path.clone();

        let log_space = preset.log_space.clone();
        let metering = metering_mode.to_string();

        tracing::debug!(lut = lut_id, ev = manual_ev, lens = enable_lens_correction, "Applying preview grading");

        tokio::task::spawn_blocking(move || {
            let session = RaPreviewSession { ptr: session_addr as *mut std::ffi::c_void };
            lib.apply_preview_grading(
                &session,
                Some(log_space.as_str()),
                &lut_data,
                use_auto_exposure,
                &metering,
                manual_ev,
                PREVIEW_JPEG_QUALITY,
                Path::new(&output_path),
            )
        })
        .await
        .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

        let url = format!(
            "http://image-preview.localhost/{}",
            percent_encode(&output_path_for_url.to_string_lossy())
        );
        Ok(url)
    }

    pub async fn end(&self) -> Result<(), AppError> {
        let lib = RawAlchemyLib::get()?;
        let mut guard = self.inner.lock().await;

        if let Some(active) = guard.take() {
            tracing::info!(image = %active.image_path, "Ending preview session");
            end_session_internal(&lib, active);
        }

        Ok(())
    }
}

fn end_session_internal(lib: &Arc<RawAlchemyLib>, active: ActiveSession) {
    lib.end_preview_session(active.session);
    let _ = std::fs::remove_file(&active.preview_output_path);
}

fn percent_encode(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' | b'\\' => {
                result.push(byte);
            }
            _ => {
                result.extend_from_slice(format!("%{:02X}", byte).as_bytes());
            }
        }
    }
    String::from_utf8(result).unwrap_or_default()
}

impl Drop for ColorGradingPreviewState {
    fn drop(&mut self) {
        if let Some(active) = self.inner.try_lock().ok().and_then(|mut g| g.take()) {
            if let Ok(lib) = RawAlchemyLib::get() {
                lib.end_preview_session(active.session);
                let _ = std::fs::remove_file(&active.preview_output_path);
            }
        }
    }
}
