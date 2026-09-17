// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::error::AppError;

fn lock_result<T>(result: std::sync::LockResult<T>) -> Result<T, AppError> {
    // 锁中毒恢复：config 是纯值数据（Arc<AppConfig> 不可变快照 +
    // Mutex<()> 纯互斥），守卫本身没有结构性破坏——某次持锁 panic 后
    // 若持续报错，会让之后所有 get/mutate/persist 路径永久瘫痪。
    // 恢复守卫（into_inner）优于永久失效；签名保持 Result 不变，
    // 调用方（read/write/persist_lock）语义照旧。
    Ok(result.unwrap_or_else(|e| e.into_inner()))
}

static GLOBAL_CONFIG_SERVICE: OnceLock<Arc<ConfigService>> = OnceLock::new();

#[derive(Clone)]
pub struct ConfigService {
    config: Arc<RwLock<Arc<AppConfig>>>,
    config_path: PathBuf,
    /// Serializes the whole mutate→persist sequence of both the sync
    /// ([`ConfigService::mutate_and_persist`]) and async
    /// ([`ConfigService::mutate_and_persist_async`]) variants. The in-memory
    /// RwLock is only held for the fast phases (clone+mutate+validate / swap
    /// in), so concurrent writers must not interleave between "read current
    /// state" and "persist next state" — otherwise two-phase interleaving
    /// could lose updates.
    persist_lock: Arc<std::sync::Mutex<()>>,
}

impl ConfigService {
    /// Store this instance as the global singleton for JNI/bridge access.
    pub fn set_global(self: &Arc<Self>) {
        let _ = GLOBAL_CONFIG_SERVICE.set(Arc::clone(self));
    }

    /// Get the global ConfigService instance (set during app setup).
    /// Panics if called before `set_global()`.
    pub fn get_global() -> &'static Arc<Self> {
        GLOBAL_CONFIG_SERVICE
            .get()
            .expect("ConfigService global not initialized")
    }

    pub fn new() -> Result<Self, AppError> {
        let service = Self::new_with_path(AppConfig::config_path());
        service.load()?;
        Ok(service)
    }

    pub fn new_with_path(config_path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(Arc::new(AppConfig::default()))),
            config_path,
            persist_lock: Arc::new(std::sync::Mutex::new(())),
        }
    }

    /// Load the config from disk and swap it into memory.
    ///
    /// 仅在初始化阶段（ConfigService::new）调用；不持 persist_lock，
    /// 与运行期 mutate 并发调用会产生竞争。
    pub fn load(&self) -> Result<AppConfig, AppError> {
        let loaded_config = Self::load_from_path(&self.config_path)?;
        let mut guard = lock_result(self.config.write())?;
        let result = loaded_config.clone();
        *guard = Arc::new(loaded_config);
        Ok(result)
    }

    /// Cheap snapshot: clones only an `Arc`, not the whole `AppConfig`.
    /// The snapshot is immutable — later mutations replace the inner Arc
    /// and never mutate an outstanding snapshot in place.
    pub fn get(&self) -> Result<Arc<AppConfig>, AppError> {
        let guard = lock_result(self.config.read())?;
        Ok(Arc::clone(&guard))
    }

    /// Fault-tolerant read: returns the in-memory config, or defaults when the
    /// read fails. Lives here — next to the config state — so platform/commands
    /// code does not depend on each other for it. Note: lock poisoning no
    /// longer fails reads (see `lock_result` — the guard is recovered), so the
    /// fallback is purely defensive.
    pub fn get_or_default(&self) -> AppConfig {
        match self.get() {
            Ok(config) => (*config).clone(),
            Err(e) => {
                error!(error = %e, "Failed to read config from ConfigService, returning defaults");
                AppConfig::default()
            }
        }
    }

    pub fn mutate_and_persist<F, R>(&self, mutate: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut AppConfig) -> R,
    {
        // Serialize the whole mutate→persist sequence; see persist_lock docs.
        // 失败语义：validate 失败或落盘失败时内存保持旧值（`?` 先于换入）。
        let _persist_guard = lock_result(self.persist_lock.lock())?;

        let (next_config, result) = {
            let guard = lock_result(self.config.write())?;
            // 双重解引用：克隆内层 AppConfig（(*guard).clone() 只会浅克隆外层 Arc，
            // 导致下方 mutate(&mut …) 与 normalized_for_current_platform() 类型不符）。
            let mut next_config = (**guard).clone();
            let result = mutate(&mut next_config);
            next_config = next_config.normalized_for_current_platform();

            if let Err(e) = next_config.validate() {
                return Err(AppError::Other(format!("Invalid configuration: {}", e)));
            }

            (next_config, result)
        }; // write guard released here — disk I/O below no longer blocks get()

        Self::save_to_path(&self.config_path, &next_config)?;

        let mut guard = lock_result(self.config.write())?;
        *guard = Arc::new(next_config);

        Ok(result)
    }

    /// Async variant of [`ConfigService::mutate_and_persist`]: identical
    /// semantics (the in-memory config is only swapped in after the persist
    /// succeeded), but the whole mutate→persist sequence — including the
    /// fsync-heavy `save_to_path` — runs on tokio's blocking pool so Tauri
    /// async commands get a `Send` future and do not stall worker threads.
    // Delegating to the sync version on the blocking pool (instead of holding
    // the std MutexGuard across `spawn_blocking().await`, which would make
    // the future !Send and fail Tauri's command bound) also gives strictly
    // better cancellation semantics: the blocking task owns the full
    // save+swap-in sequence and runs it to completion, so disk and memory
    // cannot diverge; the persist_lock still guarantees mutual exclusion
    // between the sync (JNI) and async (command) paths. See the field docs.
    pub async fn mutate_and_persist_async<F, R>(&self, mutate: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut AppConfig) -> R + Send + 'static,
        R: Send + 'static,
    {
        let service = self.clone();
        tokio::task::spawn_blocking(move || service.mutate_and_persist(mutate))
            .await
            .map_err(|e| AppError::Other(format!("Config persist task failed: {}", e)))?
    }

    fn load_from_path(path: &Path) -> Result<AppConfig, AppError> {
        let config = if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        error!(config_path = ?path, error = %e, "Failed to parse config, using defaults");
                        Self::remove_corrupt_config(path);
                        AppConfig::default()
                    }
                },
                Err(e) => {
                    // read 失败≠语义损坏：可能是瞬时 I/O/权限错误，文件内容可能完好。
                    // 不删除、不覆盖（下方 `!path.exists()` 守卫为 false，天然不会写回），
                    // 仅回退默认配置，让下次启动重读原文件。
                    error!(config_path = ?path, error = %e, "Failed to read config, using defaults");
                    AppConfig::default()
                }
            }
        } else {
            AppConfig::default()
        };

        let config = config.normalized_for_current_platform();

        if !path.exists() {
            Self::save_to_path(path, &config)?;
        }

        info!(config_path = ?path, "Config loaded into ConfigService");
        Ok(config)
    }

    /// 删除损坏的配置文件（随后由 load 流程以默认配置重新生成，即新配置覆盖旧文件）。
    /// 仅用于解析失败分支：read 失败（瞬时 I/O/权限错误）不删文件，留给下次启动重读。
    /// 删除失败仅记录警告：回退默认配置的主流程不应被删除失败阻断。
    fn remove_corrupt_config(path: &Path) {
        match fs::remove_file(path) {
            Ok(()) => info!(
                config_path = ?path,
                "Removed corrupt config file; it will be recreated from defaults"
            ),
            Err(e) => warn!(
                config_path = ?path,
                error = %e,
                "Failed to remove corrupt config file"
            ),
        }
    }

    fn save_to_path(path: &Path, config: &AppConfig) -> Result<(), AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(config)?;

        // 原子写入：先写同目录临时文件并 fsync 落盘，再 rename 覆盖目标文件，
        // 避免写入中途崩溃/断电导致 config.json 半写损坏（读取时变成解析失败）。
        // 临时文件名附带 PID，避免并发写时互相冲突。
        let temp_path = {
            let mut file_name = path
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_else(|| "config.json".into());
            file_name.push(format!(".tmp.{}", std::process::id()));
            path.with_file_name(file_name)
        };

        let write_result = (|| -> Result<(), AppError> {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            // 确保数据先于 rename 落盘
            file.sync_all()?;
            drop(file);
            // std::fs::rename 在 Windows 上会替换已存在的目标文件
            fs::rename(&temp_path, path)?;
            Ok(())
        })();

        if let Err(e) = write_result {
            // 清理残留临时文件（清理失败可忽略，不影响错误返回）
            let _ = fs::remove_file(&temp_path);
            return Err(e);
        }

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

    fn assert_mutate_and_persist_roundtrip(
        mutate: impl FnOnce(&mut AppConfig),
        verify: impl Fn(&AppConfig),
    ) {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        service
            .mutate_and_persist(mutate)
            .expect("failed to mutate and persist config");

        // Verify in-memory state
        verify(&service.get().expect("failed to get config"));

        // Verify persisted state survives reload
        let reloaded_service = ConfigService::new_with_path(config_path);
        let reloaded = reloaded_service.load().expect("failed to reload config");
        verify(&reloaded);
    }

    #[test]
    fn mutate_and_persist_failure_leaves_memory_unchanged() {
        // 语义钉死：save_to_path 在 blocked-parent（文件而非目录）上
        // create_dir_all 失败 → 落盘失败 ⇒ 内存保持旧值。同步/异步两版
        // 都必须维护这一契约（mutate_and_persist 的 `?` 先于 `*guard` 换入）。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let blocked_parent = temp_dir.path().join("blocked-parent");
        std::fs::write(&blocked_parent, "not a directory").expect("failed to create blocker file");

        let service = ConfigService::new_with_path(blocked_parent.join("config.json"));

        let result = service.mutate_and_persist(|config| config.port = 7073);

        assert!(result.is_err());
        assert_eq!(
            service.get().expect("failed to get config").port,
            AppConfig::default().port
        );
    }

    #[tokio::test]
    async fn mutate_and_persist_async_keeps_memory_unchanged_on_persist_failure() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let blocked_parent = temp_dir.path().join("blocked-parent");
        std::fs::write(&blocked_parent, "not a directory").expect("failed to create blocker file");

        let service = ConfigService::new_with_path(blocked_parent.join("config.json"));

        let result = service
            .mutate_and_persist_async(|config| config.port = 7074)
            .await;

        assert!(result.is_err());
        assert_eq!(
            service.get().expect("failed to get config").port,
            AppConfig::default().port
        );
    }

    #[tokio::test]
    async fn concurrent_gets_do_not_deadlock_with_mutate_and_persist_async() {
        // 正确性冒烟（非时延断言）：100 个并发 get 与一次落盘变更共存，
        // 全部完成且最终 get 看到新值（耗时阈值断言在 CI 上过于 flaky）。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = Arc::new(ConfigService::new_with_path(config_path));
        service.load().expect("failed to load config");

        let mut readers = Vec::new();
        for _ in 0..100 {
            let reader = Arc::clone(&service);
            readers.push(tokio::spawn(async move {
                reader.get().expect("concurrent get must not fail")
            }));
        }

        service
            .mutate_and_persist_async(|config| config.port = 7075)
            .await
            .expect("failed to mutate and persist config");

        for handle in readers {
            handle.await.expect("reader task must not deadlock");
        }

        assert_eq!(service.get().expect("failed to get config").port, 7075);
    }

    #[test]
    fn get_snapshot_is_stable_across_mutation() {
        // get() 返回不可变快照：后续 mutate 通过替换内部 Arc 生效，
        // 绝不原地修改旧快照。若实现退化为原地 patch，本测试失败。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path);
        service.load().expect("failed to load config");

        let default_port = service.get().expect("failed to get config").port;
        let snapshot = service.get().expect("failed to get snapshot");

        service
            .mutate_and_persist(|config| config.port = 7076)
            .expect("failed to mutate and persist config");

        assert_eq!(
            snapshot.port, default_port,
            "old snapshot must be immutable"
        );
        assert_eq!(
            service.get().expect("failed to get config").port,
            7076,
            "new snapshot must reflect the mutation"
        );
    }

    #[test]
    fn poisoned_config_lock_recovers_instead_of_failing_forever() {
        // X4 语义钉住：锁中毒恢复。旧行为：某次持锁 panic 后，get/mutate
        // 永久返回 "Config lock poisoned" 错误；新行为：恢复守卫，配置
        // 读写继续可用（config 是纯值数据，中毒无结构性破坏）。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        service.load().expect("failed to load config");

        // 持写锁 panic → 守卫在 unwind 中被丢弃，锁进入中毒状态
        let lock = Arc::clone(&service.config);
        let _ = std::thread::spawn(move || {
            let _guard = lock.write().unwrap();
            panic!("poison the config lock");
        })
        .join();

        assert!(
            service.get().is_ok(),
            "get must recover after lock poisoning"
        );
        service
            .mutate_and_persist(|config| config.port = 7321)
            .expect("mutate must recover after lock poisoning");
        assert_eq!(service.get().expect("get after recovery").port, 7321);
    }

    #[test]
    fn mutate_and_persist_updates_memory_and_disk_atomically() {
        assert_mutate_and_persist_roundtrip(
            |config| {
                config.port = 7070;
            },
            |config| {
                assert_eq!(config.port, 7070);
            },
        );
    }

    #[test]
    fn mutate_and_persist_keeps_save_path_in_memory_and_on_disk_consistent() {
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

        assert_mutate_and_persist_roundtrip(
            |config| {
                config.save_path = PathBuf::from("/tmp/custom-cameraftp");
            },
            move |config| {
                assert_eq!(config.save_path, expected_save_path);
            },
        );
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

    #[test]
    fn load_falls_back_to_defaults_for_partially_valid_json() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        // Valid JSON structure but with unknown field — serde ignores unknowns
        // and uses defaults for missing fields due to #[serde(default)]
        fs::write(&config_path, r#"{"unknownField": true}"#).expect("write partial config");

        let service = ConfigService::new_with_path(config_path);
        let loaded = service.load().expect("should load without error");

        // Should use defaults for missing fields
        assert_eq!(loaded.port, AppConfig::default().port);
        assert_eq!(
            service.get().expect("failed to get config").port,
            AppConfig::default().port
        );
    }

    #[test]
    fn load_deletes_corrupt_config_and_falls_back() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, "{ invalid json").expect("failed to write corrupt config");

        let service = ConfigService::new_with_path(config_path.clone());
        let loaded = service.load().expect("failed to load config");

        // Falls back to defaults...
        assert_eq!(loaded.port, AppConfig::default().port);
        // ...the corrupt file is deleted (no .bak backup is kept)...
        let backup_path = temp_dir.path().join("config.json.bak");
        assert!(!backup_path.exists(), "no .bak should be created");
        // ...and a fresh default config replaces it on disk
        let content = fs::read_to_string(&config_path).expect("config.json should be recreated");
        let parsed: AppConfig =
            serde_json::from_str(&content).expect("recreated config should be valid JSON");
        assert_eq!(parsed.port, AppConfig::default().port);
    }

    #[test]
    fn repeated_corrupt_loads_keep_working() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, "{ first corrupt").expect("write corrupt config");

        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("load with first corrupt config");

        fs::write(&config_path, "{ second corrupt").expect("write corrupt config again");
        let service = ConfigService::new_with_path(config_path.clone());
        let loaded = service.load().expect("load with second corrupt config");

        assert_eq!(loaded.port, AppConfig::default().port);
        let content = fs::read_to_string(&config_path).expect("config.json should be recreated");
        let parsed: AppConfig =
            serde_json::from_str(&content).expect("recreated config should be valid JSON");
        assert_eq!(parsed.port, AppConfig::default().port);
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temp_files() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        service
            .mutate_and_persist(|config| {
                config.port = 7071;
            })
            .expect("failed to mutate and persist config");

        // Target file exists with the persisted value and parses cleanly
        let content = fs::read_to_string(&config_path).expect("config.json should exist");
        let parsed: AppConfig =
            serde_json::from_str(&content).expect("config.json should be valid JSON");
        assert_eq!(parsed.port, 7071);

        // No leftover temp files from the atomic write
        let leftovers: Vec<String> = std::fs::read_dir(temp_dir.path())
            .expect("read config dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp."))
            .collect();
        assert!(
            leftovers.is_empty(),
            "atomic write should not leave temp files: {:?}",
            leftovers
        );
    }

    #[test]
    fn load_preserves_file_when_read_fails() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        // 非 UTF-8 字节使 fs::read_to_string 返回 Err（命中 read 失败分支而非
        // serde 解析分支），且在 Windows/Linux 上均确定可复现——这是对瞬时 I/O
        // 读失败（如共享冲突、权限）最接近的可移植模拟。
        let raw = b"{\xFF\xFE\"port\": not utf-8".to_vec();
        fs::write(&config_path, &raw).expect("failed to write non-utf8 config");

        let service = ConfigService::new_with_path(config_path.clone());
        let loaded = service.load().expect("failed to load config");

        // 读失败仅回退默认配置...
        assert_eq!(loaded.port, AppConfig::default().port);
        assert_eq!(
            service.get().expect("failed to get config").port,
            AppConfig::default().port
        );

        // ...但文件既不删除也不覆盖：原始字节原样保留，下次启动可重读。
        let preserved =
            fs::read(&config_path).expect("config.json must be preserved on read failure");
        assert_eq!(preserved, raw);
    }

    #[test]
    fn load_recovers_with_valid_config_after_corrupt_delete() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let corrupt_content = "{ invalid json";
        fs::write(&config_path, corrupt_content).expect("write corrupt config");

        // First load: falls back to defaults and deletes the corrupt file
        let service = ConfigService::new_with_path(config_path.clone());
        let loaded = service
            .load()
            .expect("corrupt config must load with defaults");
        assert_eq!(loaded.port, AppConfig::default().port);

        // A persisted mutation after the corrupt-load must recover the file:
        // config.json on disk becomes valid JSON with the new value...
        service
            .mutate_and_persist(|config| {
                config.port = 7777;
            })
            .expect("persist after corrupt load should succeed");

        let content = fs::read_to_string(&config_path).expect("config.json should exist");
        let parsed: AppConfig =
            serde_json::from_str(&content).expect("recovered config.json should be valid JSON");
        assert_eq!(parsed.port, 7777);

        // ...and no corrupt residue is left behind
        let backup_path = temp_dir.path().join("config.json.bak");
        assert!(!backup_path.exists(), "no .bak should exist");

        // Reloading the recovered file must succeed with the persisted value
        let reloaded_service = ConfigService::new_with_path(config_path);
        let reloaded = reloaded_service.load().expect("reload recovered config");
        assert_eq!(reloaded.port, 7777);
    }

    #[tokio::test]
    async fn concurrent_mutates_do_not_lose_updates() {
        // N=8 并发 mutate_and_persist_async：persist_lock 序列化整个
        // mutate→persist 序列，最终磁盘 config.json 必须是某一次完整写入的
        // 结果（port 与 username 来自同一次 mutate），无字段丢失或交叉。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = Arc::new(ConfigService::new_with_path(config_path.clone()));
        service.load().expect("failed to load config");

        const N: u16 = 8;
        let base_port: u16 = 7100;
        let mut handles = Vec::new();
        for i in 0..N {
            let service = Arc::clone(&service);
            let port = base_port + i;
            let username = format!("concurrent-user-{i}");
            handles.push(tokio::spawn(async move {
                service
                    .mutate_and_persist_async(move |config| {
                        config.port = port;
                        config.advanced_connection.auth.username = username;
                    })
                    .await
                    .expect("concurrent mutate must succeed");
            }));
        }
        for handle in handles {
            handle.await.expect("mutate task must not panic");
        }

        // 磁盘序列化结果：可解析、port ∈ 写入集合、username 为最后写入者之一且非空
        let content = fs::read_to_string(&config_path).expect("config.json should exist");
        let parsed: AppConfig =
            serde_json::from_str(&content).expect("config.json should be valid JSON");
        let expected_ports: std::collections::HashSet<u16> =
            (0..N).map(|i| base_port + i).collect();
        let expected_usernames: std::collections::HashSet<String> =
            (0..N).map(|i| format!("concurrent-user-{i}")).collect();
        assert!(
            expected_ports.contains(&parsed.port),
            "persisted port {} must be one of the written values",
            parsed.port
        );
        assert!(
            expected_usernames.contains(&parsed.advanced_connection.auth.username),
            "persisted username {:?} must be one of the written values",
            parsed.advanced_connection.auth.username
        );
        assert!(
            !parsed.advanced_connection.auth.username.is_empty(),
            "username must never be lost to a torn write"
        );
        // 配对断言：port 与 username 必须来自同一次完整 mutate（persist_lock
        // 序列化保证）——若出现交叉写（port 来自 A、username 来自 B），
        // 下式将得不到任何合法的 concurrent-user-{i} 值
        assert_eq!(
            parsed.advanced_connection.auth.username,
            format!("concurrent-user-{}", parsed.port - base_port),
            "port and username must originate from the same serialized mutate"
        );

        // 内存快照与磁盘一致（最后一次落盘的结果）
        let in_memory = service.get().expect("failed to get config");
        assert_eq!(in_memory.port, parsed.port);
        assert_eq!(
            in_memory.advanced_connection.auth.username,
            parsed.advanced_connection.auth.username
        );
    }
}
