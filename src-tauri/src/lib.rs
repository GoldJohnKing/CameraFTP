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
    get_metering_modes,
    get_current_file_index,
    get_file_list,
    get_image_exif,
    get_latest_image,
    get_platform,
    get_server_runtime_state,
    get_storage_info,
    hide_main_window,
    is_raw_file,
    load_config,
    navigate_to_file,
    open_external_link,
    open_folder_select_file,
    open_preview_window,
    quit_application,
    request_all_files_permission,
    save_auth_config,
    save_config,
    select_executable_file,
    select_save_directory,
    set_autostart_command,
    show_main_window,
    start_server,
    stop_server,
    trigger_ai_edit,
    update_preview_config,
    FtpServerState,
};

fn setup_logging() {
    use tracing_subscriber::EnvFilter;

    // Debug: log to file
    #[cfg(debug_assertions)]
    {
        use std::fs;
        #[cfg(target_os = "android")]
        use std::path::PathBuf;
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

        #[cfg(target_os = "android")]
        let log_dir = PathBuf::from(platform::android::DEFAULT_STORAGE_PATH).join("logs");

        #[cfg(target_os = "windows")]
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

        let env_filter = EnvFilter::new("debug");

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_appender)
            .init();

        tracing::info!(log_file = ?log_file, "Logging initialized");
    }

    // Release: stderr only, no log files
    #[cfg(not(debug_assertions))]
    {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("info"))
            .with_ansi(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_target(true)
            .init();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Release: init stderr logging immediately (no path dependency)
    #[cfg(not(debug_assertions))]
    setup_logging();

    // 获取平台实例
    let platform = platform::get_platform();
    let is_autostart = platform.is_autostart_mode();

    if is_autostart {
        tracing::info!("Running in autostart mode - window will be hidden");
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .setup(move |app| {
            // 统一平台初始化（托盘、权限等）
            if let Err(e) = platform.setup(app.handle()) {
                eprintln!("Platform setup failed: {}", e);
            }

            // 初始化应用数据目录（所有平台）
            config::init_app_paths(app.handle());

            // Debug: init file logging after paths are set
            #[cfg(debug_assertions)]
            setup_logging();

            let config_service = Arc::new(ConfigService::new()?);
            app.manage(Arc::clone(&config_service));
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
            {
                let app_data_dir = app.path().app_data_dir()
                    .expect("Failed to resolve app data dir");
                if let Err(e) = color_grading::resources::ensure_resources(&app_data_dir) {
                    tracing::warn!("Color grading resource extraction failed: {}", e);
                }

                let lib_path = resolve_raw_alchemy_lib_path();
                if let Err(e) = color_grading::ffi::RawAlchemyLib::load_global(&lib_path) {
                    tracing::error!("Failed to load RawAlchemyCpp: {}", e);
                }

                app.manage(color_grading::ColorGradingService::new(app.handle().clone(), Arc::clone(&config_service)));
            }

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
            request_all_files_permission,
            ensure_storage_ready,
            check_server_start_prerequisites,

            // 自动预览配置（Windows）
            update_preview_config,
            open_preview_window,
            select_executable_file,
            open_folder_select_file,
            open_external_link,

            // 文件索引
            get_file_list,
            get_current_file_index,
            navigate_to_file,
            get_latest_image,

            // EXIF 信息
            get_image_exif,

            // AI 修图
            trigger_ai_edit,
            enqueue_ai_edit,
            cancel_ai_edit,

            // 调色
            get_color_grading_presets,
    get_metering_modes,
            enqueue_color_grading,
            cancel_color_grading,
            is_raw_file,
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
            let path_encoded = request
                .uri()
                .path()
                .strip_prefix('/')
                .unwrap_or("")
                .to_string();

            std::thread::spawn(move || {
                let path = PathBuf::from(percent_decode(&path_encoded));
                let content_type = image_preview::content_type_for(&path);
                match cache.get_or_load(&path) {
                    Ok(bytes) => responder.respond(
                        tauri::http::Response::builder()
                            .status(200)
                            .header("Content-Type", content_type)
                            .body(bytes.to_vec())
                            .unwrap(),
                    ),
                    Err(e) => {
                        tracing::error!(
                            "Failed to load image preview for {}: {}",
                            path_encoded,
                            e
                        );
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(500)
                                .body(b"Failed to load image".to_vec())
                                .unwrap(),
                        );
                    }
                }
            });
        },
    );

    builder.run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Fatal error running Tauri application: {}", e);
            std::process::exit(1);
        });
}

fn resolve_raw_alchemy_lib_path() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    {
        // Kotlin side calls System.loadLibrary("raw_alchemy_core") in MainActivity.onCreate().
        // After that, dlopen("libraw_alchemy_core.so") finds the already-loaded library.
        std::path::PathBuf::from("libraw_alchemy_core.so")
    }
    #[cfg(target_os = "windows")]
    {
        // DLL is embedded in the exe via include_bytes! and extracted to temp at startup.
        match color_grading::ffi::embedded_dll::extract_to_temp() {
            Ok(path) => path,
            Err(e) => {
                tracing::error!("Failed to extract embedded DLL: {}. Falling back to exe dir.", e);
                let exe_dir = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                exe_dir.join("raw_alchemy_core.dll")
            }
        }
    }
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

/// Percent-decode a URI path component (handles UTF-8 encoded file paths).
#[cfg(target_os = "windows")]
fn percent_decode(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
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
