// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use crate::error::AppError;
use super::ffi::{RaPreviewSession, RawAlchemyLib};
use super::lut_data;
use super::presets::find_preset;

const PREVIEW_JPEG_QUALITY: i32 = 50;

static GLOBAL_PREVIEW_STATE: OnceLock<ColorGradingPreviewState> = OnceLock::new();

struct ActiveSession {
    session: RaPreviewSession,
    image_path: String,
}

pub struct ColorGradingPreviewState {
    inner: Mutex<Option<ActiveSession>>,
}

impl ColorGradingPreviewState {
    pub fn get_global() -> &'static Self {
        GLOBAL_PREVIEW_STATE.get().expect("ColorGradingPreviewState not initialized")
    }

    pub fn ensure_init() -> &'static Self {
        GLOBAL_PREVIEW_STATE.get_or_init(Self::new)
    }

    fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn begin(
        &self,
        image_path: &str,
        lensfun_db_path: Option<&str>,
        half_size: bool,
        max_preview_width: u32,
        max_preview_height: u32,
    ) -> Result<(), AppError> {
        let lib = RawAlchemyLib::get()?;
        let input_path = Path::new(image_path);

        let mut guard = self.inner.lock().await;

        if let Some(active) = guard.take() {
            tracing::info!(old_image = %active.image_path, "Ending previous preview session");
            end_session_internal(&lib, active);
        }

        tracing::info!(image = image_path, half_size, max_w = max_preview_width, max_h = max_preview_height, "Beginning preview session (decoding RAW)...");

        let session = tokio::task::spawn_blocking({
            let input_path = input_path.to_path_buf();
            let lensfun = lensfun_db_path.map(String::from);
            move || {
                lib.begin_preview_session(
                    &input_path,
                    lensfun.as_deref(),
                    half_size,
                    max_preview_width,
                    max_preview_height,
                )
            }
        })
        .await
        .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

        tracing::info!(image = image_path, "Preview session ready");

        *guard = Some(ActiveSession {
            session,
            image_path: image_path.to_string(),
        });

        Ok(())
    }

    pub async fn apply(
        &self,
        lut_id: &str,
        metering_mode: &str,
        ev_offset: f32,
        max_width: u32,
        max_height: u32,
    ) -> Result<Vec<u8>, AppError> {
        let lib = RawAlchemyLib::get()?;
        let preset = find_preset(lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;
        let lut_data = lut_data::get_lut_data(&preset.id)?;

        let mut guard = self.inner.lock().await;
        let active = guard.as_mut()
            .ok_or_else(|| AppError::ColorGradingError("No active preview session".into()))?;

        let session_addr = active.session.ptr as usize;
        let log_space = preset.log_space.clone();
        let metering = metering_mode.to_string();

        tracing::debug!(lut = lut_id, ev = ev_offset,
                        max_w = max_width, max_h = max_height, "Applying preview grading");

        tokio::task::spawn_blocking(move || {
            let session = RaPreviewSession { ptr: session_addr as *mut std::ffi::c_void };
            lib.apply_preview_grading(
                &session,
                Some(log_space.as_str()),
                &lut_data,
                ev_offset,
                &metering,
                PREVIEW_JPEG_QUALITY,
                max_width,
                max_height,
            )
        })
        .await
        .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))?
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
}
