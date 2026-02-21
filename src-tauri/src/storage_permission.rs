use std::path::PathBuf;
use tauri::AppHandle;
use tracing::{info, warn};

use crate::config::AppConfig;

/// Storage path information struct
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoragePathInfo {
    pub path_name: String,
    pub uri: String,
    pub raw_path: Option<String>,
    pub is_valid: bool,
}

/// Server start prerequisites check result
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub current_path: Option<StoragePathInfo>,
}

/// Validate storage permission for a given URI
/// On Android: checks SAF permission
/// On Desktop: checks if path exists and is a directory
#[tauri::command]
pub async fn validate_storage_permission(
    app: AppHandle,
    uri: String,
) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        use crate::platform::android;
        let is_valid = android::check_saf_permission(&app, &uri);
        info!("Validated SAF permission for URI: valid={}", is_valid);
        Ok(is_valid)
    }

    #[cfg(not(target_os = "android"))]
    {
        // On desktop, treat URI as a file path
        let path = PathBuf::from(&uri);
        let is_valid = path.exists() && path.is_dir();
        info!("Validated storage path on desktop: path={:?}, valid={}", path, is_valid);
        Ok(is_valid)
    }
}

/// Save storage path to configuration
/// On Android: persists SAF permission and saves URI
/// On Desktop: just updates the save_path
#[tauri::command]
pub async fn save_storage_path(
    app: AppHandle,
    path_name: String,
    uri: String,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use crate::platform::android;
        
        // First, persist the SAF permission
        let persisted = android::persist_saf_permission(&app, &uri);
        if !persisted {
            warn!("Failed to persist SAF permission for URI: {}", uri);
            return Err("Failed to persist storage permission".to_string());
        }
        info!("SAF permission persisted successfully");
        
        // Try to convert URI to file path for reference
        let raw_path = android::uri_to_file_path(&app, &uri);
        
        // Load current config and update
        let mut config = AppConfig::load();
        config.save_path = PathBuf::from(&path_name);
        config.save_path_uri = Some(uri.clone());
        config.save_path_raw = raw_path.clone();
        
        config.save().map_err(|e| format!("Failed to save config: {}", e))?;
        info!("Storage path saved: name={}, uri={}, raw_path={:?}", path_name, uri, raw_path);
    }

    #[cfg(not(target_os = "android"))]
    {
        // On desktop, just update the save_path
        let mut config = AppConfig::load();
        config.save_path = PathBuf::from(&path_name);
        // Also store URI for consistency (though it's just the path on desktop)
        config.save_path_uri = Some(uri);
        
        config.save().map_err(|e| format!("Failed to save config: {}", e))?;
        info!("Storage path saved on desktop: path={}", path_name);
    }

    Ok(())
}

/// Get current storage path information from config
/// Validates permission if on Android
#[tauri::command]
pub async fn get_storage_path(app: AppHandle) -> Result<Option<StoragePathInfo>, String> {
    let config = AppConfig::load();
    
    // If we have a URI stored, use that for validation
    if let Some(uri) = &config.save_path_uri {
        #[cfg(target_os = "android")]
        {
            use crate::platform::android;
            let is_valid = android::check_saf_permission(&app, uri);
            
            let path_info = StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: uri.clone(),
                raw_path: config.save_path_raw.clone(),
                is_valid,
            };
            
            info!("Retrieved storage path: valid={}", is_valid);
            return Ok(Some(path_info));
        }
        
        #[cfg(not(target_os = "android"))]
        {
            let is_valid = config.save_path.exists() && config.save_path.is_dir();
            
            let path_info = StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: uri.clone(),
                raw_path: Some(config.save_path.to_string_lossy().to_string()),
                is_valid,
            };
            
            info!("Retrieved storage path on desktop: valid={}", is_valid);
            return Ok(Some(path_info));
        }
    }
    
    // No URI stored, just return the path without URI info
    let is_valid = config.save_path.exists() && config.save_path.is_dir();
    
    let path_info = StoragePathInfo {
        path_name: config.save_path.to_string_lossy().to_string(),
        uri: config.save_path.to_string_lossy().to_string(),
        raw_path: Some(config.save_path.to_string_lossy().to_string()),
        is_valid,
    };
    
    Ok(Some(path_info))
}

/// Check server start prerequisites
/// Verifies that storage path is valid and ready for server to start
#[tauri::command]
pub async fn check_server_start_prerequisites(
    app: AppHandle,
) -> Result<ServerStartCheckResult, String> {
    let config = AppConfig::load();
    
    // Check if we have a valid storage path
    let path_info = if let Some(uri) = &config.save_path_uri {
        #[cfg(target_os = "android")]
        {
            use crate::platform::android;
            let is_valid = android::check_saf_permission(&app, uri);
            
            Some(StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: uri.clone(),
                raw_path: config.save_path_raw.clone(),
                is_valid,
            })
        }
        
        #[cfg(not(target_os = "android"))]
        {
            let is_valid = config.save_path.exists() && config.save_path.is_dir();
            
            Some(StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: uri.clone(),
                raw_path: Some(config.save_path.to_string_lossy().to_string()),
                is_valid,
            })
        }
    } else {
        // No URI stored, check if desktop path is valid
        #[cfg(not(target_os = "android"))]
        {
            let is_valid = config.save_path.exists() && config.save_path.is_dir();
            
            Some(StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: config.save_path.to_string_lossy().to_string(),
                raw_path: Some(config.save_path.to_string_lossy().to_string()),
                is_valid,
            })
        }
        
        #[cfg(target_os = "android")]
        {
            // On Android, we really need a URI
            None
        }
    };
    
    // Determine if server can start
    let can_start = match &path_info {
        Some(info) if info.is_valid => true,
        Some(_) => {
            warn!("Storage path exists but permission is not valid");
            false
        }
        None => {
            warn!("No storage path configured");
            false
        }
    };
    
    let reason = if !can_start {
        Some(match &path_info {
            Some(info) if !info.is_valid => {
                "Storage permission is not valid. Please reselect the storage folder.".to_string()
            }
            None => "No storage path configured. Please select a storage folder first.".to_string(),
            _ => "Unknown error".to_string(),
        })
    } else {
        None
    };
    
    let result = ServerStartCheckResult {
        can_start,
        reason,
        current_path: path_info,
    };
    
    info!("Server start prerequisites check: can_start={}", can_start);
    Ok(result)
}

/// Get the last saved storage URI from config
/// Used for pre-selecting the folder in the SAF picker
#[tauri::command]
pub async fn get_last_storage_uri() -> Result<Option<String>, String> {
    let config = AppConfig::load();
    Ok(config.save_path_uri)
}
