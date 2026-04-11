// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use tracing::{error, info};

use crate::config::AppConfig;
use crate::error::AppError;

fn lock_result<T>(result: std::sync::LockResult<T>) -> Result<T, AppError> {
    result.map_err(|e| AppError::Other(format!("Config lock poisoned: {}", e)))
}

#[derive(Clone)]
pub struct ConfigService {
    config: Arc<RwLock<AppConfig>>,
    config_path: PathBuf,
}

impl ConfigService {
    pub fn new() -> Result<Self, AppError> {
        let service = Self::new_with_path(AppConfig::config_path());
        service.load()?;
        Ok(service)
    }

    pub fn new_with_path(config_path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(AppConfig::default())),
            config_path,
        }
    }

    pub fn load(&self) -> Result<AppConfig, AppError> {
        let loaded_config = Self::load_from_path(&self.config_path)?;
        let mut guard = lock_result(self.config.write())?;
        let result = loaded_config.clone();
        *guard = loaded_config;
        Ok(result)
    }

    pub fn get(&self) -> Result<AppConfig, AppError> {
        let guard = lock_result(self.config.read())?;
        Ok(guard.clone())
    }

    pub fn update(&self, new_config: AppConfig) -> Result<(), AppError> {
        let new_config = new_config.normalized_for_current_platform();
        let mut guard = lock_result(self.config.write())?;
        *guard = new_config;
        Ok(())
    }

    #[cfg(test)]
    pub fn save(&self) -> Result<(), AppError> {
        let config = self.get()?;
        Self::save_to_path(&self.config_path, &config)
    }

    pub fn mutate_and_persist<F, R>(&self, mutate: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut AppConfig) -> R,
    {
        let mut guard = lock_result(self.config.write())?;

        let mut next_config = guard.clone();
        let result = mutate(&mut next_config);
        next_config = next_config.normalized_for_current_platform();
        Self::save_to_path(&self.config_path, &next_config)?;
        *guard = next_config;

        Ok(result)
    }

    fn load_from_path(path: &Path) -> Result<AppConfig, AppError> {
        #[cfg_attr(not(target_os = "android"), allow(unused_mut))]
        let mut config = if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        error!(config_path = ?path, error = %e, "Failed to parse config, using defaults");
                        AppConfig::default()
                    }
                },
                Err(e) => {
                    error!(config_path = ?path, error = %e, "Failed to read config, using defaults");
                    AppConfig::default()
                }
            }
        } else {
            AppConfig::default()
        };

        config = config.normalized_for_current_platform();

        if !path.exists() {
            Self::save_to_path(path, &config)?;
        }

        info!(config_path = ?path, "Config loaded into ConfigService");
        Ok(config)
    }

    fn save_to_path(path: &Path, config: &AppConfig) -> Result<(), AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let config_to_save = config.clone().normalized_for_current_platform();

        let content = serde_json::to_string_pretty(&config_to_save)?;
        fs::write(path, content)?;
        info!(config_path = ?path, "Config persisted by ConfigService");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn load_reads_existing_config_file() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        let mut expected = AppConfig::default();
        expected.port = 4242;
        let content = serde_json::to_string_pretty(&expected).expect("failed to serialize config");
        fs::write(&config_path, content).expect("failed to write config");

        let service = ConfigService::new_with_path(config_path);
        let loaded = service.load().expect("failed to load config");

        assert_eq!(loaded.port, 4242);
        assert_eq!(service.get().expect("failed to get config").port, 4242);
    }

    #[test]
    fn update_is_visible_on_subsequent_reads() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path);

        service.load().expect("failed to load config");
        let mut updated = service.get().expect("failed to get config");
        updated.port = 5050;
        service.update(updated).expect("failed to update config");

        assert_eq!(service.get().expect("failed to get config").port, 5050);
    }

    #[test]
    fn save_persists_updates_for_new_service_instances() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");
        let mut updated = service.get().expect("failed to get config");
        updated.port = 6060;
        service.update(updated).expect("failed to update config");
        service.save().expect("failed to save config");

        let reloaded_service = ConfigService::new_with_path(config_path);
        let reloaded = reloaded_service.load().expect("failed to reload config");
        assert_eq!(reloaded.port, 6060);
    }

    #[test]
    fn mutate_and_persist_updates_memory_and_disk_atomically() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        service
            .mutate_and_persist(|config| {
                config.port = 7070;
            })
            .expect("failed to mutate and persist config");

        assert_eq!(service.get().expect("failed to get config").port, 7070);

        let reloaded_service = ConfigService::new_with_path(config_path);
        let reloaded = reloaded_service.load().expect("failed to reload config");
        assert_eq!(reloaded.port, 7070);
    }

    #[test]
    fn mutate_and_persist_keeps_save_path_in_memory_and_on_disk_consistent() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        let expected_save_path = {
            #[cfg(target_os = "android")]
            {
                PathBuf::from(crate::constants::ANDROID_DEFAULT_STORAGE_PATH)
            }

            #[cfg(not(target_os = "android"))]
            {
                PathBuf::from("/tmp/custom-cameraftp")
            }
        };

        service
            .mutate_and_persist(|config| {
                config.save_path = PathBuf::from("/tmp/custom-cameraftp");
            })
            .expect("failed to mutate and persist config");

        assert_eq!(
            service.get().expect("failed to get config").save_path,
            expected_save_path
        );

        let reloaded_service = ConfigService::new_with_path(config_path);
        let reloaded = reloaded_service.load().expect("failed to reload config");
        assert_eq!(reloaded.save_path, expected_save_path);
    }

    #[test]
    fn load_falls_back_to_defaults_when_config_is_invalid() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, "{ invalid json").expect("failed to write invalid config");

        let service = ConfigService::new_with_path(config_path);
        let loaded = service.load().expect("failed to load config");

        assert_eq!(loaded.port, AppConfig::default().port);
        assert_eq!(
            service.get().expect("failed to get config").port,
            AppConfig::default().port
        );
    }
}
