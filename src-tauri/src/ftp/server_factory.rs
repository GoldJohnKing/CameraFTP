// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 服务器工厂 - 统一服务器启动逻辑

use crate::config_service::ConfigService;
use crate::constants::{
    DEFAULT_FTP_PORT_WINDOWS, DEFAULT_FTP_PORT_ANDROID,
    MIN_PORT, IDLE_TIMEOUT_SECONDS,
};
use crate::error::AppError;
use crate::ftp::{
    create_ftp_server, EventBus, EventProcessor, FtpServerHandle, FtpAuthConfig,
    StatsEventHandler, TrayUpdateHandler,
};
use crate::ftp::types::{FtpServerSlot, ServerConfig};
use crate::network::NetworkManager;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, oneshot};
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct ServerStartupContext {
    pub port: u16,
    pub ip: String,
    pub event_bus: EventBus,
    pub display_credentials: (Option<String>, Option<String>),
}

pub(crate) async fn start_ftp_server(
    state: &Arc<Mutex<FtpServerSlot>>,
    app_handle: AppHandle,
) -> Result<ServerStartupContext, AppError> {
    // 竞态修复（sentinel 状态机）：在执行任何耗时步骤（配置读取、存储校验、
    // 端口探测、Actor 创建与监听）之前，先原子性地认领启动权（None → Starting）。
    // 认领后，其余并发的 start 调用（如 UI 按钮与托盘菜单同时触发）会在此处被拒绝，
    // 不会各自创建 FtpServerActor 并绑定出两个端口的孤儿服务器。
    claim_start_slot(state).await?;

    // catch_unwind：即使启动过程 panic，也走统一的失败收尾以释放槽位，
    // 避免槽位卡死在 Starting 导致后续启动永久失败
    let result = futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(
        start_ftp_server_claimed(app_handle),
    ))
    .await
    .unwrap_or_else(|payload| {
        error!(panic = ?payload, "FTP server startup panicked");
        Err(AppError::Other("FTP server startup panicked".to_string()))
    });

    finish_claimed_start(state, result).await
}

/// 认领启动权：`None → Starting`；`Starting`/`Running` 一律拒绝。
///
/// 错误统一使用 `ServerAlreadyRunning`（与旧的“句柄已存在”检查保持同一约定），
/// 不区分“启动中”与“已运行”，避免向公共错误契约（前端按 code 匹配）引入新变体。
pub(crate) async fn claim_start_slot(
    state: &Arc<Mutex<FtpServerSlot>>,
) -> Result<(), AppError> {
    let mut guard = state.lock().await;
    match *guard {
        FtpServerSlot::None => {
            *guard = FtpServerSlot::Starting;
            Ok(())
        }
        FtpServerSlot::Starting | FtpServerSlot::Running(_) => {
            Err(AppError::ServerAlreadyRunning)
        }
    }
}

/// 启动收尾：成功则提交 `Running`，任何失败则回滚槽位为 `None`。
///
/// 本函数是认领之后所有失败路径的唯一出口，覆盖：
/// `start_ftp_server_claimed` 内的全部早退 `?`（配置读取、存储校验、端口、网络接口）、
/// `start_actor_system` 的启动失败分支（构建/监听/就绪超时），以及 panic（经 catch_unwind）。
async fn finish_claimed_start(
    state: &Arc<Mutex<FtpServerSlot>>,
    result: Result<(ServerStartupContext, FtpServerHandle), AppError>,
) -> Result<ServerStartupContext, AppError> {
    match result {
        Ok((ctx, server_handle)) => {
            commit_running_slot(state, server_handle).await;
            Ok(ctx)
        }
        Err(e) => {
            release_start_slot(state).await;
            Err(e)
        }
    }
}

/// 提交运行句柄（仅当槽位仍为 Starting 时写入，防止意外覆盖其他状态）
async fn commit_running_slot(state: &Arc<Mutex<FtpServerSlot>>, handle: FtpServerHandle) {
    let mut guard = state.lock().await;
    if matches!(*guard, FtpServerSlot::Starting) {
        *guard = FtpServerSlot::Running(handle);
    }
}

/// 回滚启动权（仅当槽位仍为 Starting 时复位，不触碰 Running/None）
async fn release_start_slot(state: &Arc<Mutex<FtpServerSlot>>) {
    let mut guard = state.lock().await;
    if matches!(*guard, FtpServerSlot::Starting) {
        *guard = FtpServerSlot::None;
    }
}

/// 认领后的启动流程：读取配置 → 校验存储 → 选择端口/IP → 启动 Actor 系统。
/// 本函数内的所有早退错误路径由 `finish_claimed_start` 统一回滚槽位。
async fn start_ftp_server_claimed(
    app_handle: AppHandle,
) -> Result<(ServerStartupContext, FtpServerHandle), AppError> {
    let config_service = app_handle.state::<Arc<ConfigService>>();
    let config = config_service
        .get()
        .map_err(|e| AppError::Other(format!("Failed to read config from service: {}", e)))?;

    // 统一通过 PlatformService 验证存储路径
    // 这会处理平台特定的权限检查和目录创建
    // 平台实现内部包含阻塞的文件系统检查（exists/create_dir_all/可写探测），
    // 放入 spawn_blocking 避免阻塞异步运行时
    let app_handle_for_storage = app_handle.clone();
    let save_path = tokio::task::spawn_blocking(move || {
        crate::platform::get_platform()
            .ensure_storage_ready(&app_handle_for_storage)
            .map_err(|e| {
                error!(error = %e, "Storage not ready");
                AppError::StoragePermissionError(e)
            })
    })
    .await
    .map_err(|e| AppError::Other(format!("Storage readiness task failed: {}", e)))??;

    // ensure_storage_ready 返回已验证的存储路径，转换为 PathBuf 供后续使用
    let save_path = std::path::PathBuf::from(save_path);

    // 查找可用端口
    // 当 advanced_connection 禁用时，Windows 使用默认端口 21，Android 使用 2121
    let default_port = if cfg!(target_os = "windows") {
        DEFAULT_FTP_PORT_WINDOWS
    } else {
        DEFAULT_FTP_PORT_ANDROID
    };
    let requested_port = if config.advanced_connection.enabled {
        config.port
    } else {
        default_port
    };

    let port = if NetworkManager::is_port_available(requested_port).await {
        requested_port
    } else if config.auto_select_port {
        warn!(
            requested_port = requested_port,
            "Port not available, searching for alternative"
        );
        NetworkManager::find_available_port(MIN_PORT)
            .await
            .ok_or_else(|| {
                error!("No available port found");
                AppError::NoAvailablePort
            })?
    } else {
        return Err(AppError::NoAvailablePort);
    };

    // 获取推荐IP
    let ip = NetworkManager::recommended_ip().ok_or_else(|| {
        error!("No network interface available");
        AppError::NoNetworkInterface
    })?;

    // 创建服务器配置
    // 注意：PASV 端口使用 libunftp 默认范围 49152-65535（无需手动配置）
    let server_config = ServerConfig {
        port,
        root_path: save_path.clone(),
        idle_timeout_seconds: IDLE_TIMEOUT_SECONDS,
        auth: if config.advanced_connection.enabled {
            FtpAuthConfig::from(&config.advanced_connection.auth)
        } else {
            FtpAuthConfig::default()
        },
    };

    let display_credentials = server_config.auth.to_display_credentials();

    start_actor_system(Some(app_handle), server_config, ip, display_credentials).await
}

/// 创建并启动 Actor 系统（统计 Actor、服务器 Actor）并完成监听。
///
/// 与 AppHandle 相关的配置/存储解析不在本函数内，
/// 便于单元测试以 `None` AppHandle 与显式 `ServerConfig` 直接驱动（见模块测试）。
/// 成功返回 (启动上下文, 服务器句柄)；失败时中止 Actor 任务并返回错误。
async fn start_actor_system(
    app_handle: Option<AppHandle>,
    server_config: ServerConfig,
    ip: String,
    display_credentials: (Option<String>, Option<String>),
) -> Result<(ServerStartupContext, FtpServerHandle), AppError> {
    let port = server_config.port;

    // 创建FTP服务器Actor
    let (server_handle, server_actor, stats_worker, event_bus) = create_ftp_server(app_handle);

    // 运行统计Actor Worker（必须在后台运行，否则统计不会更新）
    tokio::spawn(async move {
        stats_worker.run().await;
    });

    // 运行服务器Actor
    let actor_handle = tokio::spawn(async move {
        server_actor.run().await;
    });

    // 启动服务器
    match server_handle.start(server_config).await {
        Ok(bind_addr) => {
            info!(
                bind_addr = %bind_addr,
                ip = %ip,
                port = port,
                "FTP server started successfully"
            );

            Ok((
                ServerStartupContext {
                    port,
                    ip,
                    event_bus,
                    display_credentials,
                },
                server_handle,
            ))
        }
        Err(e) => {
            error!(error = %e, "Failed to start FTP server");
            actor_handle.abort();
            Err(e)
        }
    }
}

pub(crate) fn spawn_event_processor(app_handle: AppHandle, event_bus: &EventBus) -> oneshot::Receiver<()> {
    let app_handle_for_tray = app_handle.clone();
    let (ready_tx, ready_rx) = oneshot::channel();

    // 从 EventBus 借用组件，不获取所有权
    // 这样 event_bus 继续由调用者拥有，服务器和处理器共享同一个状态通道
    let runtime_state = event_bus.runtime_state();
    let state_rx = runtime_state.subscribe();
    // 保留 runtime_state 用于直接查询，避免 watch channel 的竞态条件
    let runtime_state_for_processor = runtime_state.clone();

    tokio::spawn(async move {
        let processor = EventProcessor::from_parts(
            state_rx,
            Some(runtime_state_for_processor)
        )
            .register_runtime_state_handler(StatsEventHandler::new(app_handle.clone()))
            .register_runtime_state_handler(TrayUpdateHandler::new(app_handle_for_tray));

        // 先运行处理器并等待其完成初始化（读取当前状态并分发给 handlers）
        // 这确保在返回 ready 信号前，所有 runtime state handlers 都已处理当前状态
        processor.run_with_ready_signal(ready_tx).await;
    });

    ready_rx
}

/// Complete server startup sequence: start FTP server and spawn event processor.
///
/// This encapsulates the shared logic used by both manual start (`start_server` command)
/// and autostart (`execute_autostart_server` on Windows).
pub async fn start_server_with_event_pipeline(
    state: &Arc<Mutex<FtpServerSlot>>,
    app_handle: AppHandle,
    ready_timeout: std::time::Duration,
) -> Result<ServerStartupContext, AppError> {
    let ctx = start_ftp_server(state, app_handle.clone()).await?;

    let ready_rx = spawn_event_processor(app_handle.clone(), &ctx.event_bus);

    if tokio::time::timeout(ready_timeout, ready_rx).await.is_err() {
        info!("Event processor readiness timed out during startup");
    }

    Ok(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ftp::types::{FtpAuthConfig, FtpServerSlot, ServerConfig};
    use std::path::Path;
    use std::time::Duration;

    /// TLS 证书预热互斥锁：首个需要真实监听的测试串行生成证书，
    /// 避免并行测试同时触发生成导致证书文件写入竞争
    static TLS_WARMUP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn warm_up_tls_certs() {
        let _guard = TLS_WARMUP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::crypto::tls::ensure_valid_certificate().expect("TLS cert warmup");
    }

    /// 选取一个当前空闲的 TCP 端口（bind(:0) 后立即释放；存在微小 TOCTOU，测试可接受）
    fn pick_free_port() -> u16 {
        std::net::TcpListener::bind(("127.0.0.1", 0))
            .expect("bind ephemeral port")
            .local_addr()
            .expect("local addr")
            .port()
    }

    /// 探测端口是否已被监听
    async fn port_is_listening(port: u16) -> bool {
        tokio::time::timeout(
            Duration::from_millis(200),
            tokio::net::TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        .map(|result| result.is_ok())
        .unwrap_or(false)
    }

    fn test_config(port: u16, root_path: &Path) -> ServerConfig {
        ServerConfig {
            port,
            root_path: root_path.to_path_buf(),
            idle_timeout_seconds: 900,
            auth: FtpAuthConfig::default(),
        }
    }

    /// 测试用启动路径：复用生产的 认领 → Actor 启动 → 提交/回滚 全套状态机，
    /// 仅以 None AppHandle 与显式 ServerConfig 替代配置读取
    /// （配置读取依赖 Tauri 托管状态，无法在单元测试中构建）
    async fn start_via_slot(
        state: &Arc<Mutex<FtpServerSlot>>,
        config: ServerConfig,
    ) -> Result<ServerStartupContext, AppError> {
        claim_start_slot(state).await?;
        let display_credentials = config.auth.to_display_credentials();
        let result =
            start_actor_system(None, config, "127.0.0.1".to_string(), display_credentials).await;
        finish_claimed_start(state, result).await
    }

    /// 停止槽位中运行的服务器并复位槽位（测试清理用）
    async fn stop_and_reset(state: &Arc<Mutex<FtpServerSlot>>) {
        let handle = {
            let mut guard = state.lock().await;
            match std::mem::replace(&mut *guard, FtpServerSlot::None) {
                FtpServerSlot::Running(handle) => Some(handle),
                other => {
                    *guard = other;
                    None
                }
            }
        };
        if let Some(handle) = handle {
            let _ = handle.stop().await;
        }
    }

    /// 回归测试：两个并发 start 只允许一个真正监听。
    ///
    /// 旧行为（竞态）：commands 层与工厂层的“已运行”检查都在释放状态锁之后才进行
    /// Actor 创建与 listen，两个并发调用可各自通过检查并绑定两个端口，
    /// 后完成者覆盖 FtpServerState，先启动的服务器成为无法停止的孤儿。
    /// 修复后：第二个调用在认领阶段即被拒绝（ServerAlreadyRunning）。
    #[tokio::test]
    async fn concurrent_double_start_yields_single_listener() {
        warm_up_tls_certs();

        let state = Arc::new(Mutex::new(FtpServerSlot::None));
        let dir = tempfile::tempdir().expect("tempdir");

        // 两个任务使用不同端口：若竞态仍存在，两者都会成功并各自绑定端口
        let port_a = pick_free_port();
        let port_b = loop {
            let candidate = pick_free_port();
            if candidate != port_a {
                break candidate;
            }
        };

        let state_a = Arc::clone(&state);
        let state_b = Arc::clone(&state);
        let root_a = dir.path().to_path_buf();
        let root_b = dir.path().to_path_buf();

        let (result_a, result_b) = tokio::join!(
            async move { start_via_slot(&state_a, test_config(port_a, &root_a)).await },
            async move { start_via_slot(&state_b, test_config(port_b, &root_b)).await },
        );

        // 恰好一个成功，另一个必须收到“已在运行”错误
        let (winner_port, loser_err) = match (result_a, result_b) {
            (Ok(ctx), Err(e)) => (ctx.port, e),
            (Err(e), Ok(ctx)) => (ctx.port, e),
            (Ok(_), Ok(_)) => {
                panic!("both concurrent starts succeeded — duplicate listener race regressed")
            }
            (Err(a), Err(b)) => panic!("both concurrent starts failed: {a:?} / {b:?}"),
        };
        assert!(
            matches!(loser_err, AppError::ServerAlreadyRunning),
            "loser should be rejected with ServerAlreadyRunning, got: {loser_err:?}"
        );

        // 绑定的监听端口数 == 1：仅胜者端口在监听，败者端口保持空闲
        assert!(port_is_listening(winner_port).await, "winner port should listen");
        let loser_port = if winner_port == port_a { port_b } else { port_a };
        assert!(!port_is_listening(loser_port).await, "loser port must stay free");

        // 槽位最终为 Running
        assert!(matches!(*state.lock().await, FtpServerSlot::Running(_)));

        stop_and_reset(&state).await;
        assert!(matches!(*state.lock().await, FtpServerSlot::None));
    }

    /// 回归测试：认领后启动失败必须回滚槽位为 None，且允许后续重试成功。
    #[tokio::test]
    async fn start_failure_resets_slot() {
        warm_up_tls_certs();

        let state = Arc::new(Mutex::new(FtpServerSlot::None));
        let dir = tempfile::tempdir().expect("tempdir");

        // 以“普通文件作为根目录”制造确定性启动失败：
        // Actor 的 prepare_root_directory（create_dir_all）必然报错，
        // 失败发生在任何监听之前
        let blocker = dir.path().join("blocker.txt");
        std::fs::write(&blocker, "not a directory").expect("write blocker file");

        let bad_port = pick_free_port();
        let error = start_via_slot(&state, test_config(bad_port, &blocker))
            .await
            .expect_err("start with file-as-root must fail");
        assert!(!matches!(error, AppError::ServerAlreadyRunning));

        // 失败后槽位必须回到 None（而非卡死在 Starting）
        assert!(
            matches!(*state.lock().await, FtpServerSlot::None),
            "slot must reset to None after start failure"
        );

        // 槽位复位后，随后的启动应当可以成功
        let good_root = dir.path().join("storage");
        std::fs::create_dir_all(&good_root).expect("create good root");
        let good_port = pick_free_port();
        let ctx = start_via_slot(&state, test_config(good_port, &good_root))
            .await
            .expect("retry after failure must succeed");
        assert_eq!(ctx.port, good_port);
        assert!(matches!(*state.lock().await, FtpServerSlot::Running(_)));

        stop_and_reset(&state).await;
    }

    /// 单元级回归测试：Starting 槽位必须直接拒绝后续启动，且不产生任何新监听。
    #[tokio::test]
    async fn starting_slot_rejects_second_start() {
        // 手动将槽位置为 Starting，模拟另一任务正处于启动窗口
        let state = Arc::new(Mutex::new(FtpServerSlot::Starting));
        let dir = tempfile::tempdir().expect("tempdir");

        // 认领入口直接拒绝
        let error = claim_start_slot(&state)
            .await
            .expect_err("claim must be rejected while Starting");
        assert!(matches!(error, AppError::ServerAlreadyRunning));

        // 完整启动路径同样在认领阶段被拒（不创建 Actor、不监听端口）
        let port = pick_free_port();
        let error = start_via_slot(&state, test_config(port, dir.path()))
            .await
            .expect_err("start path must be rejected while Starting");
        assert!(matches!(error, AppError::ServerAlreadyRunning));
        assert!(!port_is_listening(port).await, "no new listener may be created");

        // 被拒的调用不得破坏正在进行的启动（槽位保持 Starting）
        assert!(matches!(*state.lock().await, FtpServerSlot::Starting));
    }
}

