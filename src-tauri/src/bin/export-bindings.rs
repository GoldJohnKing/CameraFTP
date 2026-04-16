// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use camera_ftp_companion_lib::{
    ai_edit::progress::AiEditProgressEvent,
    commands::ExifInfo,
    config::{
        AdvancedConnectionConfig, AndroidImageOpenMethod, AndroidImageViewerConfig, AppConfig,
        AuthConfig, ImageOpenMethod, PreviewWindowConfig,
    },
    file_index::FileInfo,
    ftp::{ServerInfo, ServerStateSnapshot},
    platform::{PermissionStatus, ServerStartCheckResult, StorageInfo},
};
use ts_rs::{ExportError, TS};

fn export_type<T: TS + ?Sized + 'static>() -> Result<(), ExportError> {
    T::export_all()
}

fn main() -> Result<(), ExportError> {
    export_type::<AuthConfig>()?;
    export_type::<AdvancedConnectionConfig>()?;
    export_type::<ImageOpenMethod>()?;
    export_type::<PreviewWindowConfig>()?;
    export_type::<AndroidImageOpenMethod>()?;
    export_type::<AndroidImageViewerConfig>()?;
    export_type::<AppConfig>()?;
    export_type::<ExifInfo>()?;
    export_type::<FileInfo>()?;
    export_type::<StorageInfo>()?;
    export_type::<PermissionStatus>()?;
    export_type::<ServerStartCheckResult>()?;
    export_type::<ServerStateSnapshot>()?;
    export_type::<ServerInfo>()?;
    export_type::<camera_ftp_companion_lib::ftp::types::ServerRuntimeView>()?;
    export_type::<AiEditProgressEvent>()?;

    #[cfg(target_os = "windows")]
    export_type::<camera_ftp_companion_lib::auto_open::ConfigChangedEvent>()?;

    Ok(())
}
