// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod ai_edit;
pub mod auto_open;
pub mod commands;
pub mod config;
pub mod config_service;
pub mod constants;
pub mod crypto;
pub mod error;
pub mod file_index;
pub mod ftp;
pub mod image_utils;
pub mod color_grading;
#[cfg(target_os = "windows")]
pub mod image_preview;
pub mod network;
pub mod platform;
pub mod utils;

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Manager;

#[cfg(target_os = "windows")]
use image_preview::ImagePreviewCache;

use auto_open::AutoOpenService;
use config_service::ConfigService;
use file_index::FileIndexService;

use commands::{
    begin_color_grading_preview,
    apply_color_grading_preview,
    end_color_grading_preview,
    check_permission_status,
    check_port_available,
    check_server_start_prerequisites,
    cancel_ai_edit,
    cancel_color_grading,
    ensure_storage_ready,
    enqueue_ai_edit,
    enqueue_color_grading,
    get_autostart_status,
    get_color_grading_presets,
    get_current_file_index,
    get_file_list,
    get_image_exif,
    get_raw_orientation,
    inject_exif_orientation,
    get_latest_image,
    get_platform,
    get_server_runtime_state,
    get_storage_info,
    hide_main_window,
    load_config,
    navigate_to_file,
    open_external_link,
    open_folder_select_file,
    open_preview_window,
    open_save_directory,
    quit_application,
    save_auth_config,
    save_config,
    select_executable_file,
    select_save_directory,
    set_autostart_command,
    show_main_window,
    start_server,
    stop_server,
    update_preview_config,
    FtpServerState,
};

// desktop-generic (not android-specific)
#[cfg(not(target_os = "android"))]
fn setup_logging() {
    use tracing_subscriber::EnvFilter;

    // Desktop: always log to a file under the app config dir (needed for
    // diagnostics in release). app_config_dir() resolves from dirs:: at first
    // call, so this works before config::init_app_paths() too.
    {
        use std::fs;
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

        let log_dir = config::app_config_dir().join("logs");
        let log_file = log_dir.join("app.log");
        let log_file_for_writer = log_file.clone();

        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create log directory {:?}: {}", log_dir, e);
        }

        let file_appender = tracing_subscriber::fmt::layer()
            .with_writer(move || {
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file_for_writer)
                {
                    Ok(file) => Box::new(file) as Box<dyn std::io::Write + Send + Sync>,
                    Err(_) => Box::new(std::io::stderr()) as Box<dyn std::io::Write + Send + Sync>,
                }
            })
            .with_ansi(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_target(true);

        #[cfg(debug_assertions)]
        let env_filter = EnvFilter::new("debug");
        #[cfg(not(debug_assertions))]
        let env_filter = EnvFilter::new("info");

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_appender)
            .init();

        tracing::info!(log_file = ?log_file, "Logging initialized");

        // Capture the log-file path so it can be pushed into the C++ core via
        // ra_set_log_file after the RawAlchemy DLL loads (replaces the former
        // RA_NN_LOG_FILE env var — invisible to MSVC std::getenv on Windows).
        let _ = color_grading::bootstrap::NN_LOG_FILE.set(log_file);
    }
}

/// Android logging: logcat is the only sink. See [`android_logging::setup`].
#[cfg(target_os = "android")]
mod android_logging;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Desktop release: init file logging immediately (no path dependency).
    // Android defers to the setup() closure where app_data_dir is known — logcat
    // needs no path, and the optional file mirror must go to app-private storage.
    #[cfg(all(not(debug_assertions), not(target_os = "android")))]
    setup_logging();

    // 获取平台实例
    let platform = platform::get_platform();
    let is_autostart = platform.is_autostart_mode();

    if is_autostart {
        tracing::info!("Running in autostart mode - window will be hidden");
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(FtpServerState(Arc::new(Mutex::new(ftp::FtpServerSlot::None))))
        .setup(move |app| {
            // 统一平台初始化（托盘、权限等）
            if let Err(e) = platform.setup(app.handle()) {
                eprintln!("Platform setup failed: {}", e);
            }

            // 初始化应用数据目录（所有平台）
            config::init_app_paths(app.handle());

            // Desktop debug: init file logging after paths are set.
            #[cfg(all(debug_assertions, not(target_os = "android")))]
            setup_logging();

            // Android: logcat is always on.
            #[cfg(target_os = "android")]
            {
                android_logging::setup();
            }

            let config_service = Arc::new(ConfigService::new()?);
            config_service.set_global();
            app.manage(Arc::clone(&config_service));

            // 启动时扩展 asset protocol scope：save_config 在运行期对新保存目录
            // allow_directory，但重启后 tauri.conf.json 的静态 scope 不含该目录；
            // 此处按持久化配置补齐，使重启后语义与 save_config 路径一致。
            // 无条件扩展（含默认路径）：消除"默认路径必已含于静态 scope"的
            // 隐式假设——allow_directory 幂等且廉价，重复添加无害；
            // 失败仅 warn，不阻断启动
            match config_service.get() {
                Ok(config) => {
                    let save_path = config.save_path.clone();
                    if let Err(e) = app.asset_protocol_scope().allow_directory(&save_path, true) {
                        tracing::warn!(
                            error = %e,
                            path = ?save_path,
                            "Failed to extend asset protocol scope for configured save_path"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Failed to read config for startup asset scope extension"
                    );
                }
            }
            let file_index = Arc::new(FileIndexService::new(Arc::clone(&config_service)));
            tauri::async_runtime::block_on(file_index.set_app_handle(app.handle().clone()));
            app.manage(file_index);

            // 在 setup 中管理 AutoOpenService
            app.manage(AutoOpenService::new(app.handle().clone(), Arc::clone(&config_service)));
            app.manage(ai_edit::AiEditService::new(app.handle().clone(), Arc::clone(&config_service)));

            // Image preview cache with memory caching (Windows only)
            #[cfg(target_os = "windows")]
            app.manage(Arc::new(ImagePreviewCache::new()));

            // Initialize color grading: load RawAlchemyCpp library + extract resources
            color_grading::bootstrap::init(app.handle(), &config_service);

            // 开机自启模式：隐藏窗口
            if is_autostart {
                platform.hide_window_on_autostart(app.handle());
            }

            // 设置主窗口关闭处理（桌面平台）
            #[cfg(target_os = "windows")]
            setup_window_close_handler(app.handle());

            // 如果是开机启动模式，自动启动服务器
            if is_autostart {
                let state: tauri::State<'_, FtpServerState> = app.state();
                platform.execute_autostart_server(app.handle(), &state.0);
            }

            // 启动后台任务
            spawn_background_tasks(app.handle());

            // 托盘图标状态更新现在由 TrayUpdateHandler 的运行时状态订阅驱动

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 服务器控制
            start_server,
            stop_server,
            get_server_runtime_state,
            
            // 配置管理
            load_config,
            save_config,
            save_auth_config,
            select_save_directory,
            
            // 网络
            check_port_available,
            
            // 平台
            get_platform,
            
            // 自动启动（Windows）
            set_autostart_command,
            get_autostart_status,
            
            // 应用控制
            quit_application,
            hide_main_window,
            show_main_window,
            
            // 存储权限（新 API）
            get_storage_info,
            check_permission_status,
            ensure_storage_ready,
            check_server_start_prerequisites,

            // 预览配置与文件操作（Windows）
            update_preview_config,
            open_preview_window,
            select_executable_file,
            open_folder_select_file,
            open_save_directory,
            open_external_link,

            // 文件索引
            get_file_list,
            get_current_file_index,
            navigate_to_file,
            get_latest_image,

            // EXIF 信息
            get_image_exif,
            get_raw_orientation,
            inject_exif_orientation,

            // AI 修图
            enqueue_ai_edit,
            cancel_ai_edit,

            // 调色
            get_color_grading_presets,
            enqueue_color_grading,
            cancel_color_grading,
            begin_color_grading_preview,
            apply_color_grading_preview,
            end_color_grading_preview,
        ]);

    #[cfg(target_os = "windows")]
    let builder = builder.register_asynchronous_uri_scheme_protocol(
        "image-preview",
        |ctx, request, responder| {
            use std::path::PathBuf;
            use std::sync::Arc;

            let cache: Arc<ImagePreviewCache> = ctx
                .app_handle()
                .state::<Arc<ImagePreviewCache>>()
                .inner()
                .clone();
            // 每个 preview 请求都必须位于当前配置的 save_path 之下。
            // 用 Arc 快照读取（避免每请求整份 AppConfig 深拷贝）；读取失败
            // 回落默认配置并记录（旧 get_or_default 静默吞掉了失败）。
            let save_root = match ctx.app_handle().state::<Arc<ConfigService>>().get() {
                Ok(config) => config.save_path.clone(),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "image-preview: config read failed, falling back to default save_path"
                    );
                    crate::config::AppConfig::default().save_path
                }
            };
            let path_encoded = request
                .uri()
                .path()
                .strip_prefix('/')
                .unwrap_or("")
                .to_string();

            // 每请求一个无上限 OS 线程改为走 tokio blocking 池（天然限流）。
            // responder 满足 Send + 'static，可在 blocking 任务内应答。
            // JoinHandle 刻意丢弃（fire-and-forget，同 bootstrap.rs 的用法）。
            tauri::async_runtime::spawn_blocking(move || {
                fn not_found_response() -> tauri::http::Response<Vec<u8>> {
                    tauri::http::Response::builder()
                        .status(404)
                        .body(b"Not Found".to_vec())
                        .unwrap()
                }

                let requested = PathBuf::from(utils::percent_decode(&path_encoded));
                let response = match image_preview::validate_preview_path(&requested, &save_root)
                {
                    Ok(Some(path)) => {
                        // content_type 用 canonical 路径判 RAW 扩展名；缓存键必须用
                        // 原始（percent-decode 后）请求路径经分隔符归一（cache_key）：
                        // invalidate 调用点（file_index 的 remove_file 删除、
                        // add_file 的 EXIF 回填替换条目）传的都是原始字符串，
                        // 键若用 canonical（Windows `\\?\` verbatim）会永不匹配
                        // 导致失效变 no-op。不同拼写的重复条目由 LRU 有界。
                        let content_type = image_preview::content_type_for(&path);
                        match cache.get_or_load(&requested) {
                            Ok(bytes) => tauri::http::Response::builder()
                                .status(200)
                                .header("Content-Type", content_type)
                                .body(bytes.to_vec())
                                .unwrap(),
                            Err(e) => {
                                tracing::error!(
                                    "Failed to load image preview for {}: {}",
                                    path_encoded,
                                    e
                                );
                                tauri::http::Response::builder()
                                    .status(500)
                                    .body(b"Failed to load image".to_vec())
                                    .unwrap()
                            }
                        }
                    }
                    // Ok(None)（越界/非文件）与 Err（不存在/不可访问）响应必须
                    // 一致（统一 404 "Not Found"）：若以 403 区分"存在但越界"，
                    // 状态码本身就构成存在性 oracle。两种失败仅在 warn 日志中
                    // 可区分（原因不同），响应体不泄露差异。
                    Ok(None) => {
                        tracing::warn!(
                            requested = %path_encoded,
                            "image-preview request outside save_path rejected"
                        );
                        not_found_response()
                    }
                    Err(e) => {
                        tracing::warn!(
                            requested = %path_encoded,
                            error = %e,
                            "image-preview request path not found"
                        );
                        not_found_response()
                    }
                };
                responder.respond(response);
            });
        },
    );

    builder.run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Fatal error running Tauri application: {}", e);
            std::process::exit(1);
        });
}

/// 设置主窗口关闭请求处理器（桌面平台）
#[cfg(target_os = "windows")]
fn setup_window_close_handler(app_handle: &tauri::AppHandle) {
    use tauri::Emitter;
    
    if let Some(window) = app_handle.get_webview_window("main") {
        let handle = app_handle.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = crate::platform::get_platform().show_main_window(&handle);
                let _ = handle.emit("window-close-requested", ());
            }
        });
    }
}

/// 启动后台任务（文件扫描、文件监听等）
/// 先执行文件扫描，扫描完成后再启动文件监听，避免竞态条件
fn spawn_background_tasks(app_handle: &tauri::AppHandle) {
    let handle = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        // 1. 先执行文件扫描
        let file_index: tauri::State<'_, Arc<FileIndexService>> = handle.state::<Arc<FileIndexService>>();
        if let Err(e) = file_index.scan_directory().await {
            tracing::error!("Failed to scan directory: {}", e);
        }

        // 2. 扫描完成后，启动文件监听
        let file_index_arc = Arc::clone(&file_index);
        match FileIndexService::start_watcher(file_index_arc).await {
            Ok(true) => tracing::info!("File watcher started successfully"),
            Ok(false) => tracing::info!("File watcher not started (unsupported platform)"),
            Err(e) => tracing::error!("Failed to start file watcher: {}", e),
        }
    });
}
