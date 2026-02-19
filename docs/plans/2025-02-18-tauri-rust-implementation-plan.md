# Camera FTP Companion - Tauri + Rust 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 使用Tauri + Rust构建跨平台（Windows/Android）相机FTP伴侣应用

**Architecture:** Tauri v2框架，React前端，Rust后端，共享一套代码库支持Windows桌面端和Android移动端

**Tech Stack:** Tauri v2, React, TypeScript, TailwindCSS, Rust, Tokio

---

## 前置准备

### 任务 0: 环境检查

**检查Rust安装:**
```bash
rustc --version
cargo --version
```
Expected: Rust >= 1.75.0

**检查Node.js:**
```bash
node --version
npm --version
```
Expected: Node >= 18.0.0

**安装Tauri CLI:**
```bash
cargo install tauri-cli@^2.0
```

---

## 阶段 1: 项目初始化

### 任务 1: 创建Tauri项目

**Files:**
- Create: 整个项目目录结构

**Step 1: 初始化Tauri项目**

```bash
cd /mnt/d/GitRepos/CameraFTPCompanion
cargo create-tauri-app camera-ftp-companion --template react-ts --manager npm
```

**Step 2: 进入项目目录**

```bash
cd camera-ftp-companion
```

**Step 3: 验证项目结构**

```bash
ls -la
```
Expected: 看到 src/, src-tauri/, package.json 等文件

**Step 4: 安装依赖**

```bash
npm install
```

**Step 5: 开发模式测试**

```bash
cargo tauri dev
```
Expected: 应用窗口打开，显示Tauri + React欢迎界面

**Step 6: Commit**

```bash
git add .
git commit -m "chore: initialize tauri project with react-ts template"
```

---

### 任务 2: 安装前端依赖

**Files:**
- Modify: `camera-ftp-companion/package.json`

**Step 1: 安装TailwindCSS及相关依赖**

```bash
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

**Step 2: 配置Tailwind**

Edit: `tailwind.config.js`
```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
```

**Step 3: 添加Tailwind指令**

Edit: `src/index.css`
```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

**Step 4: 安装图标库**

```bash
npm install lucide-react
```

**Step 5: Commit**

```bash
git add .
git commit -m "chore: setup tailwindcss and lucide-react"
```

---

## 阶段 2: 后端核心 - FTP服务器

### 任务 3: 创建FTP服务器模块结构

**Files:**
- Create: `src-tauri/src/ftp/mod.rs`
- Create: `src-tauri/src/ftp/server.rs`
- Create: `src-tauri/src/ftp/session.rs`
- Create: `src-tauri/src/ftp/protocol.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 添加Tokio依赖**

Edit: `src-tauri/Cargo.toml`
```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

**Step 2: 创建FTP模块入口**

Create: `src-tauri/src/ftp/mod.rs`
```rust
pub mod server;
pub mod session;
pub mod protocol;

pub use server::{FtpServer, ServerConfig, ServerState};
```

**Step 3: 创建协议定义**

Create: `src-tauri/src/ftp/protocol.rs`
```rust
#[derive(Debug, Clone)]
pub enum FtpCommand {
    User(String),
    Pass(String),
    Pwd,
    Cwd(String),
    Type(String),
    Mode(String),
    Stru(String),
    Pasv,
    Stor(String),
    Quit,
    Unknown(String),
}

impl FtpCommand {
    pub fn parse(line: &str) -> Self {
        let line = line.trim();
        let (cmd, arg) = if let Some(space_pos) = line.find(' ') {
            (&line[..space_pos], &line[space_pos + 1..])
        } else {
            (line, "")
        };
        
        match cmd.to_ascii_uppercase().as_str() {
            "USER" => Self::User(arg.to_string()),
            "PASS" => Self::Pass(arg.to_string()),
            "PWD" => Self::Pwd,
            "CWD" => Self::Cwd(arg.to_string()),
            "TYPE" => Self::Type(arg.to_string()),
            "MODE" => Self::Mode(arg.to_string()),
            "STRU" => Self::Stru(arg.to_string()),
            "PASV" => Self::Pasv,
            "STOR" => Self::Stor(arg.to_string()),
            "QUIT" => Self::Quit,
            _ => Self::Unknown(line.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FtpResponse {
    ServiceReady,
    UserOk,
    LoggedIn,
    PassiveMode(String),
    FileStatusOk,
    TransferComplete,
    Goodbye,
    SyntaxError,
    NotImplemented,
    NeedPassword,
    NotLoggedIn,
}

impl FtpResponse {
    pub fn to_string(&self) -> String {
        match self {
            Self::ServiceReady => "220 Welcome to Camera FTP Companion\r\n".to_string(),
            Self::UserOk => "331 User name okay, need password\r\n".to_string(),
            Self::LoggedIn => "230 User logged in, proceed\r\n".to_string(),
            Self::PassiveMode(addr) => format!("227 Entering Passive Mode ({}).\r\n", addr),
            Self::FileStatusOk => "150 File status okay; about to open data connection\r\n".to_string(),
            Self::TransferComplete => "226 Transfer complete\r\n".to_string(),
            Self::Goodbye => "221 Goodbye\r\n".to_string(),
            Self::SyntaxError => "500 Syntax error, command unrecognized\r\n".to_string(),
            Self::NotImplemented => "502 Command not implemented\r\n".to_string(),
            Self::NeedPassword => "331 Please specify the password\r\n".to_string(),
            Self::NotLoggedIn => "530 Not logged in\r\n".to_string(),
        }
    }
}
```

**Step 4: 更新lib.rs**

Edit: `src-tauri/src/lib.rs`
```rust
pub mod ftp;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 5: Commit**

```bash
git add .
git commit -m "feat(ftp): add ftp protocol definitions and module structure"
```

---

### 任务 4: 实现FTP服务器核心

**Files:**
- Create: `src-tauri/src/ftp/server.rs`

**Step 1: 创建服务器状态结构**

Create: `src-tauri/src/ftp/server.rs`
```rust
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::session::FtpSession;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub port: u16,
    pub root_path: PathBuf,
    pub allow_anonymous: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 21,
            root_path: PathBuf::from("./ftp_root"),
            allow_anonymous: true,
        }
    }
}

pub struct ServerState {
    pub is_running: AtomicBool,
    pub connected_clients: AtomicUsize,
    pub files_received: AtomicU64,
    pub bytes_received: AtomicU64,
    pub last_file: Mutex<Option<String>>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            is_running: AtomicBool::new(false),
            connected_clients: AtomicUsize::new(0),
            files_received: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            last_file: Mutex::new(None),
        }
    }
    
    pub fn snapshot(&self) -> ServerStateSnapshot {
        ServerStateSnapshot {
            is_running: self.is_running.load(Ordering::Relaxed),
            connected_clients: self.connected_clients.load(Ordering::Relaxed),
            files_received: self.files_received.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            last_file: self.last_file.try_lock().ok().and_then(|g| g.clone()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStateSnapshot {
    pub is_running: bool,
    pub connected_clients: usize,
    pub files_received: u64,
    pub bytes_received: u64,
    pub last_file: Option<String>,
}

pub struct FtpServer {
    config: ServerConfig,
    state: Arc<ServerState>,
    listener: Option<TcpListener>,
}

impl FtpServer {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            state: Arc::new(ServerState::new()),
            listener: None,
        }
    }
    
    pub async fn start(&mut self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));
        let listener = TcpListener::bind(addr).await?;
        let actual_addr = listener.local_addr()?;
        
        info!("FTP server listening on {}", actual_addr);
        
        // Create root directory if not exists
        tokio::fs::create_dir_all(&self.config.root_path).await?;
        
        self.state.is_running.store(true, Ordering::Relaxed);
        self.listener = Some(listener);
        
        // Spawn accept loop
        let state = self.state.clone();
        let root_path = self.config.root_path.clone();
        let listener = self.listener.as_ref().unwrap().try_clone()?;
        
        tokio::spawn(async move {
            Self::accept_loop(listener, state, root_path).await;
        });
        
        Ok(actual_addr)
    }
    
    pub fn stop(&self) {
        self.state.is_running.store(false, Ordering::Relaxed);
        info!("FTP server stopping");
    }
    
    pub fn state(&self) -> Arc<ServerState> {
        self.state.clone()
    }
    
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
    
    async fn accept_loop(listener: TcpListener, state: Arc<ServerState>, root_path: PathBuf) {
        while state.is_running.load(Ordering::Relaxed) {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from {}", addr);
                    state.connected_clients.fetch_add(1, Ordering::Relaxed);
                    
                    let state_clone = state.clone();
                    let root_clone = root_path.clone();
                    
                    tokio::spawn(async move {
                        let mut session = FtpSession::new(stream, addr, root_clone, state_clone);
                        if let Err(e) = session.run().await {
                            error!("Session error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                    if !state.is_running.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }
    }
}
```

**Step 2: 创建Session处理**

Create: `src-tauri/src/ftp/session.rs`
```rust
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use super::protocol::{FtpCommand, FtpResponse};
use super::server::{ServerState, ServerStateSnapshot};

pub struct FtpSession {
    control_stream: TcpStream,
    client_addr: SocketAddr,
    root_path: PathBuf,
    state: Arc<ServerState>,
    is_authenticated: bool,
    current_dir: PathBuf,
    data_listener: Option<TcpListener>,
    data_port: Option<u16>,
}

impl FtpSession {
    pub fn new(
        stream: TcpStream,
        addr: SocketAddr,
        root_path: PathBuf,
        state: Arc<ServerState>,
    ) -> Self {
        Self {
            control_stream: stream,
            client_addr: addr,
            root_path: root_path.clone(),
            state,
            is_authenticated: false,
            current_dir: root_path,
            data_listener: None,
            data_port: None,
        }
    }
    
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Send greeting
        self.send_response(FtpResponse::ServiceReady).await?;
        
        let mut buffer = vec![0u8; 1024];
        
        loop {
            let n = self.control_stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            
            let command_str = String::from_utf8_lossy(&buffer[..n]);
            debug!("Received: {}", command_str.trim());
            
            let command = FtpCommand::parse(&command_str);
            debug!("Parsed command: {:?}", command);
            
            match command {
                FtpCommand::Quit => {
                    self.send_response(FtpResponse::Goodbye).await?;
                    break;
                }
                _ => {
                    if let Err(e) = self.handle_command(command).await {
                        error!("Error handling command: {}", e);
                    }
                }
            }
        }
        
        self.state.connected_clients.fetch_sub(1, Ordering::Relaxed);
        info!("Client {} disconnected", self.client_addr);
        Ok(())
    }
    
    async fn handle_command(&mut self, cmd: FtpCommand) -> Result<(), Box<dyn std::error::Error>> {
        match cmd {
            FtpCommand::User(_) => {
                // Accept any username for anonymous
                self.send_response(FtpResponse::UserOk).await?;
            }
            FtpCommand::Pass(_) => {
                // Accept any password
                self.is_authenticated = true;
                self.send_response(FtpResponse::LoggedIn).await?;
            }
            FtpCommand::Pwd => {
                let path = "/";
                self.send_raw(&format!("257 \"{}\" is current directory.\r\n", path)).await?;
            }
            FtpCommand::Cwd(path) => {
                self.current_dir = self.root_path.join(path.trim_start_matches('/'));
                self.send_raw("250 Directory successfully changed.\r\n").await?;
            }
            FtpCommand::Type(t) => {
                self.send_raw(&format!("200 Type set to {}.\r\n", t)).await?;
            }
            FtpCommand::Mode(m) => {
                self.send_raw(&format!("200 Mode set to {}.\r\n", m)).await?;
            }
            FtpCommand::Stru(s) => {
                self.send_raw(&format!("200 Structure set to {}.\r\n", s)).await?;
            }
            FtpCommand::Pasv => {
                self.handle_pasv().await?;
            }
            FtpCommand::Stor(filename) => {
                self.handle_stor(&filename).await?;
            }
            FtpCommand::Unknown(cmd) => {
                warn!("Unknown command: {}", cmd);
                self.send_response(FtpResponse::NotImplemented).await?;
            }
            _ => {
                self.send_response(FtpResponse::NotImplemented).await?;
            }
        }
        Ok(())
    }
    
    async fn handle_pasv(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create data listener on random port
        let data_listener = TcpListener::bind("0.0.0.0:0").await?;
        let port = data_listener.local_addr()?.port();
        
        // Convert port to FTP format (p1,p2 where port = p1*256 + p2)
        let p1 = port / 256;
        let p2 = port % 256;
        
        // Format: h1,h2,h3,h4,p1,p2
        let addr_str = format!("127,0,0,1,{},{}", p1, p2);
        
        self.data_listener = Some(data_listener);
        self.data_port = Some(port);
        
        self.send_response(FtpResponse::PassiveMode(addr_str)).await?;
        Ok(())
    }
    
    async fn handle_stor(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_authenticated {
            self.send_response(FtpResponse::NotLoggedIn).await?;
            return Ok(());
        }
        
        let filepath = self.current_dir.join(filename);
        
        self.send_response(FtpResponse::FileStatusOk).await?;
        
        // Accept data connection
        if let Some(listener) = self.data_listener.take() {
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(30),
                listener.accept()
            ).await {
                Ok(Ok((mut data_stream, _))) => {
                    // Create file
                    match tokio::fs::File::create(&filepath).await {
                        Ok(mut file) => {
                            let mut buffer = vec![0u8; 8192];
                            let mut total_bytes: u64 = 0;
                            
                            loop {
                                match data_stream.read(&mut buffer).await {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if let Err(e) = file.write_all(&buffer[..n]).await {
                                            error!("File write error: {}", e);
                                            break;
                                        }
                                        total_bytes += n as u64;
                                    }
                                    Err(e) => {
                                        error!("Data stream read error: {}", e);
                                        break;
                                    }
                                }
                            }
                            
                            // Update stats
                            self.state.files_received.fetch_add(1, Ordering::Relaxed);
                            self.state.bytes_received.fetch_add(total_bytes, Ordering::Relaxed);
                            
                            let filename_str = filepath.file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string());
                            
                            if let Some(name) = filename_str {
                                let mut last_file = self.state.last_file.lock().await;
                                *last_file = Some(name);
                            }
                            
                            info!("File saved: {} ({} bytes)", filepath.display(), total_bytes);
                            self.send_response(FtpResponse::TransferComplete).await?;
                        }
                        Err(e) => {
                            error!("Failed to create file: {}", e);
                            self.send_raw("451 Failed to create file.\r\n").await?;
                        }
                    }
                }
                Ok(Err(e)) => {
                    error!("Data connection accept error: {}", e);
                    self.send_raw("425 Can't open data connection.\r\n").await?;
                }
                Err(_) => {
                    error!("Data connection timeout");
                    self.send_raw("425 Data connection timeout.\r\n").await?;
                }
            }
        } else {
            self.send_raw("425 Use PASV first.\r\n").await?;
        }
        
        Ok(())
    }
    
    async fn send_response(&mut self, response: FtpResponse) -> Result<(), Box<dyn std::error::Error>> {
        let msg = response.to_string();
        self.control_stream.write_all(msg.as_bytes()).await?;
        debug!("Sent: {}", msg.trim());
        Ok(())
    }
    
    async fn send_raw(&mut self, msg: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.control_stream.write_all(msg.as_bytes()).await?;
        debug!("Sent: {}", msg.trim());
        Ok(())
    }
}
```

**Step 3: 修复编译错误**

在session.rs顶部添加缺失的导入：
```rust
use std::sync::atomic::Ordering;
use std::sync::Arc;
```

**Step 4: 测试编译**

```bash
cd src-tauri
cargo check
```
Expected: 无错误

**Step 5: Commit**

```bash
git add .
git commit -m "feat(ftp): implement basic ftp server with passive mode and file upload"
```

---

## 阶段 3: 网络管理模块

### 任务 5: 实现网络管理

**Files:**
- Create: `src-tauri/src/network.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建网络模块**

Create: `src-tauri/src/network.rs`
```rust
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, serde::Serialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip: String,
    pub is_wifi: bool,
    pub is_ethernet: bool,
    pub is_up: bool,
}

pub struct NetworkManager;

impl NetworkManager {
    /// 获取本机所有IPv4地址
    pub fn list_interfaces() -> Vec<NetworkInterface> {
        let mut interfaces = Vec::new();
        
        if let Ok(ifaddrs) = nix::ifaddrs::getifaddrs() {
            for ifaddr in ifaddrs {
                if let Some(addr) = ifaddr.address {
                    if let Some(sockaddr) = addr.as_sockaddr_in() {
                        let ip = IpAddr::V4(Ipv4Addr::from(sockaddr.ip()));
                        
                        // Skip loopback
                        if ip.is_loopback() {
                            continue;
                        }
                        
                        let name = ifaddr.interface_name.clone();
                        let is_wifi = name.to_lowercase().contains("wlan") 
                            || name.to_lowercase().contains("wi-fi")
                            || name.to_lowercase().contains("wifi");
                        let is_ethernet = name.to_lowercase().contains("eth")
                            || name.to_lowercase().contains("en");
                        
                        interfaces.push(NetworkInterface {
                            name: name.clone(),
                            ip: ip.to_string(),
                            is_wifi,
                            is_ethernet,
                            is_up: true,
                        });
                    }
                }
            }
        }
        
        interfaces
    }
    
    /// 推荐最佳IP地址
    /// 优先级: WiFi > 以太网 > 其他
    pub fn recommended_ip() -> Option<String> {
        let interfaces = Self::list_interfaces();
        
        // 优先WiFi
        if let Some(iface) = interfaces.iter().find(|i| i.is_wifi) {
            return Some(iface.ip.clone());
        }
        
        // 其次以太网
        if let Some(iface) = interfaces.iter().find(|i| i.is_ethernet) {
            return Some(iface.ip.clone());
        }
        
        // 最后任意可用
        interfaces.first().map(|i| i.ip.clone())
    }
    
    /// 检查端口是否可用
    pub async fn is_port_available(port: u16) -> bool {
        match TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    /// 查找可用端口
    pub async fn find_available_port(start: u16) -> Option<u16> {
        for port in start..=65535 {
            if Self::is_port_available(port).await {
                return Some(port);
            }
        }
        None
    }
}
```

**Step 2: 添加nix依赖**

Edit: `src-tauri/Cargo.toml`
```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
nix = { version = "0.27", features = ["net"] }
```

**Step 3: 更新lib.rs**

Edit: `src-tauri/src/lib.rs`
```rust
pub mod ftp;
pub mod network;

use ftp::{FtpServer, ServerConfig};
use network::NetworkManager;

// ... rest of the file
```

**Step 4: Commit**

```bash
git add .
git commit -m "feat(network): add network interface detection and port management"
```

---

## 阶段 4: 配置管理

### 任务 6: 实现配置持久化

**Files:**
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建配置模块**

Create: `src-tauri/src/config.rs`
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub auto_open: bool,
    pub auto_open_program: Option<String>,
    pub port: u16,
    pub file_extensions: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: Self::default_pictures_dir(),
            auto_open: true,
            auto_open_program: None,
            port: 21,
            file_extensions: vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "raw".to_string(),
                "png".to_string(),
                "arw".to_string(),
                "cr2".to_string(),
                "nef".to_string(),
                "orf".to_string(),
                "rw2".to_string(),
            ],
        }
    }
}

impl AppConfig {
    fn default_pictures_dir() -> PathBuf {
        dirs::picture_dir().unwrap_or_else(|| PathBuf::from("./pictures"))
    }
    
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("camera-ftp-companion"))
            .unwrap_or_else(|| PathBuf::from("./config"))
            .join("config.json")
    }
    
    pub fn load() -> Self {
        let path = Self::config_path();
        
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(config) => {
                            info!("Config loaded from {:?}", path);
                            return config;
                        }
                        Err(e) => {
                            error!("Failed to parse config: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read config file: {}", e);
                }
            }
        }
        
        // Create default config
        let config = Self::default();
        if let Err(e) = config.save() {
            error!("Failed to save default config: {}", e);
        }
        config
    }
    
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        
        info!("Config saved to {:?}", path);
        Ok(())
    }
}
```

**Step 2: 添加dirs依赖**

Edit: `src-tauri/Cargo.toml`
```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
nix = { version = "0.27", features = ["net"] }
dirs = "5.0"
```

**Step 3: 更新lib.rs**

Edit: `src-tauri/src/lib.rs`
```rust
pub mod config;
pub mod ftp;
pub mod network;

use config::AppConfig;
use ftp::{FtpServer, ServerConfig};
use network::NetworkManager;

// ... rest
```

**Step 4: Commit**

```bash
git add .
git commit -m "feat(config): add configuration management with persistence"
```

---

## 阶段 5: Tauri命令和状态管理

### 任务 7: 实现Tauri命令

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建命令模块**

Create: `src-tauri/src/commands.rs`
```rust
use tauri::{command, AppHandle, Emitter, State};
use std::sync::Mutex;
use tracing::{error, info};

use crate::config::AppConfig;
use crate::ftp::{FtpServer, ServerConfig, ServerStateSnapshot};
use crate::network::NetworkManager;

pub struct FtpServerState(pub Mutex<Option<FtpServer>>);

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerInfo {
    pub is_running: bool,
    pub ip: String,
    pub port: u16,
    pub url: String,
    pub username: String,
    pub password_info: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatsUpdate {
    pub connected_clients: usize,
    pub files_received: u64,
    pub bytes_received: u64,
    pub last_file: Option<String>,
}

#[command]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, String> {
    info!("Starting FTP server...");
    
    let config = AppConfig::load();
    
    // Check if already running
    {
        let server_guard = state.0.lock().map_err(|e| e.to_string())?;
        if server_guard.is_some() {
            return Err("Server is already running".to_string());
        }
    }
    
    // Ensure save directory exists
    if let Err(e) = tokio::fs::create_dir_all(&config.save_path).await {
        return Err(format!("Failed to create save directory: {}", e));
    }
    
    // Find available port
    let port = if NetworkManager::is_port_available(config.port).await {
        config.port
    } else {
        NetworkManager::find_available_port(1025)
            .await
            .ok_or("No available port found")?
    };
    
    // Get recommended IP
    let ip = NetworkManager::recommended_ip()
        .ok_or("No network interface available")?;
    
    // Create and start server
    let server_config = ServerConfig {
        port,
        root_path: config.save_path.clone(),
        allow_anonymous: true,
    };
    
    let mut server = FtpServer::new(server_config);
    match server.start().await {
        Ok(_) => {
            info!("FTP server started on {}:{}", ip, port);
            
            // Store server
            {
                let mut server_guard = state.0.lock().map_err(|e| e.to_string())?;
                *server_guard = Some(server);
            }
            
            // Emit event
            let _ = app.emit("server-started", (ip.clone(), port));
            
            Ok(ServerInfo {
                is_running: true,
                ip: ip.clone(),
                port,
                url: format!("ftp://{}:{}", ip, port),
                username: "anonymous".to_string(),
                password_info: "(任意密码)".to_string(),
            })
        }
        Err(e) => {
            error!("Failed to start server: {}", e);
            Err(format!("Failed to start server: {}", e))
        }
    }
}

#[command]
pub fn stop_server(state: State<'_, FtpServerState>, app: AppHandle) -> Result<(), String> {
    info!("Stopping FTP server...");
    
    let mut server_guard = state.0.lock().map_err(|e| e.to_string())?;
    
    if let Some(server) = server_guard.take() {
        server.stop();
        let _ = app.emit("server-stopped", ());
        info!("FTP server stopped");
        Ok(())
    } else {
        Err("Server is not running".to_string())
    }
}

#[command]
pub fn get_server_status(state: State<'_, FtpServerState>) -> Result<Option<ServerStateSnapshot>, String> {
    let server_guard = state.0.lock().map_err(|e| e.to_string())?;
    
    if let Some(server) = server_guard.as_ref() {
        Ok(Some(server.state().snapshot()))
    } else {
        Ok(None)
    }
}

#[command]
pub fn get_network_info() -> Result<Vec<crate::network::NetworkInterface>, String> {
    Ok(NetworkManager::list_interfaces())
}

#[command]
pub fn load_config() -> AppConfig {
    AppConfig::load()
}

#[command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())
}

#[command]
pub async fn check_port_available(port: u16) -> bool {
    NetworkManager::is_port_available(port).await
}
```

**Step 2: 更新lib.rs**

Edit: `src-tauri/src/lib.rs`
```rust
pub mod commands;
pub mod config;
pub mod ftp;
pub mod network;

use commands::{FtpServerState, start_server, stop_server, get_server_status, 
               get_network_info, load_config, save_config, check_port_available};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(FtpServerState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            get_server_status,
            get_network_info,
            load_config,
            save_config,
            check_port_available,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: 修复导入**

在lib.rs顶部添加：
```rust
use std::sync::Mutex;
```

**Step 4: Commit**

```bash
git add .
git commit -m "feat(commands): implement tauri commands for server control and config"
```

---

## 阶段 6: 前端UI实现

### 任务 8: 创建UI组件

**Files:**
- Create: `src/components/ServerCard.tsx`
- Create: `src/components/StatsCard.tsx`
- Create: `src/components/InfoCard.tsx`

**Step 1: 创建类型定义**

Create: `src/types/index.ts`
```typescript
export interface ServerInfo {
  is_running: boolean;
  ip: string;
  port: number;
  url: string;
  username: string;
  password_info: string;
}

export interface ServerStatus {
  is_running: boolean;
  connected_clients: number;
  files_received: number;
  bytes_received: number;
  last_file: string | null;
}

export interface AppConfig {
  save_path: string;
  auto_open: boolean;
  auto_open_program: string | null;
  port: number;
  file_extensions: string[];
}

export interface NetworkInterface {
  name: string;
  ip: string;
  is_wifi: boolean;
  is_ethernet: boolean;
  is_up: boolean;
}
```

**Step 2: 创建ServerCard组件**

Create: `src/components/ServerCard.tsx`
```typescript
import { useState } from 'react';
import { Power, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ServerInfo } from '../types';

interface ServerCardProps {
  onStatusChange: (info: ServerInfo | null) => void;
}

export function ServerCard({ onStatusChange }: ServerCardProps) {
  const [isRunning, setIsRunning] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleToggle = async () => {
    setIsLoading(true);
    setError(null);

    try {
      if (isRunning) {
        await invoke('stop_server');
        setIsRunning(false);
        setServerInfo(null);
        onStatusChange(null);
      } else {
        const info = await invoke<ServerInfo>('start_server');
        setIsRunning(true);
        setServerInfo(info);
        onStatusChange(info);
      }
    } catch (err) {
      setError(err as string);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-800">FTP服务器</h2>
        <div className={`w-3 h-3 rounded-full ${
          isRunning ? 'bg-green-500 animate-pulse' : 'bg-red-500'
        }`} />
      </div>

      <p className="text-gray-600 mb-6">
        {isRunning 
          ? `运行中 - ${serverInfo?.ip}:${serverInfo?.port}`
          : '服务器已停止，点击启动接收照片'
        }
      </p>

      <button
        onClick={handleToggle}
        disabled={isLoading}
        className={`w-full py-3 px-4 rounded-lg font-medium flex items-center justify-center gap-2 transition-colors ${
          isRunning
            ? 'bg-red-50 text-red-600 hover:bg-red-100'
            : 'bg-blue-600 text-white hover:bg-blue-700'
        } disabled:opacity-50 disabled:cursor-not-allowed`}
      >
        {isLoading ? (
          <Loader2 className="w-5 h-5 animate-spin" />
        ) : (
          <Power className="w-5 h-5" />
        )}
        {isRunning ? '停止服务器' : '启动服务器'}
      </button>

      {error && (
        <p className="mt-3 text-sm text-red-600 text-center">{error}</p>
      )}
    </div>
  );
}
```

**Step 3: 创建StatsCard组件**

Create: `src/components/StatsCard.tsx`
```typescript
import { useEffect, useState } from 'react';
import { Camera, Image, HardDrive, Clock } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ServerStatus } from '../types';

export function StatsCard() {
  const [stats, setStats] = useState<ServerStatus>({
    is_running: false,
    connected_clients: 0,
    files_received: 0,
    bytes_received: 0,
    last_file: null,
  });

  useEffect(() => {
    // Initial load
    loadStats();

    // Set up polling
    const interval = setInterval(loadStats, 1000);

    // Listen for events
    const unlisten = listen('server-started', () => {
      loadStats();
    });

    return () => {
      clearInterval(interval);
      unlisten.then(f => f());
    };
  }, []);

  const loadStats = async () => {
    try {
      const status = await invoke<ServerStatus | null>('get_server_status');
      if (status) {
        setStats(status);
      }
    } catch (err) {
      console.error('Failed to load stats:', err);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 MB';
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(1)} MB`;
  };

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
      <h2 className="text-lg font-semibold text-gray-800 mb-4">传输统计</h2>

      <div className="space-y-4">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-blue-50 flex items-center justify-center">
            <Camera className="w-5 h-5 text-blue-600" />
          </div>
          <div>
            <p className="text-sm text-gray-500">已连接相机</p>
            <p className="text-lg font-semibold text-gray-800">
              {stats.connected_clients} 台
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-green-50 flex items-center justify-center">
            <Image className="w-5 h-5 text-green-600" />
          </div>
          <div>
            <p className="text-sm text-gray-500">已接收照片</p>
            <p className="text-lg font-semibold text-gray-800">
              {stats.files_received} 张
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-purple-50 flex items-center justify-center">
            <HardDrive className="w-5 h-5 text-purple-600" />
          </div>
          <div>
            <p className="text-sm text-gray-500">总数据量</p>
            <p className="text-lg font-semibold text-gray-800">
              {formatBytes(stats.bytes_received)}
            </p>
          </div>
        </div>

        {stats.last_file && (
          <div className="flex items-center gap-3 pt-4 border-t border-gray-100">
            <div className="w-10 h-10 rounded-lg bg-orange-50 flex items-center justify-center">
              <Clock className="w-5 h-5 text-orange-600" />
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm text-gray-500">最新照片</p>
              <p className="text-sm font-medium text-gray-800 truncate">
                {stats.last_file}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
```

**Step 4: 创建InfoCard组件**

Create: `src/components/InfoCard.tsx`
```typescript
import { useState } from 'react';
import { Wifi, Copy, Check } from 'lucide-react';
import { ServerInfo } from '../types';

interface InfoCardProps {
  serverInfo: ServerInfo | null;
}

export function InfoCard({ serverInfo }: InfoCardProps) {
  const [copied, setCopied] = useState(false);

  const copyConnectionInfo = () => {
    if (!serverInfo) return;
    
    const info = `协议: FTP (被动模式)
地址: ${serverInfo.url}
用户名: ${serverInfo.username}
密码: ${serverInfo.password_info}`;
    
    navigator.clipboard.writeText(info);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (!serverInfo) {
    return (
      <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
        <h2 className="text-lg font-semibold text-gray-800 mb-4">连接信息</h2>
        <p className="text-gray-500 text-center py-4">
          启动服务器后显示连接信息
        </p>
      </div>
    );
  }

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-800">连接信息</h2>
        <div className="w-10 h-10 rounded-lg bg-indigo-50 flex items-center justify-center">
          <Wifi className="w-5 h-5 text-indigo-600" />
        </div>
      </div>

      <div className="space-y-3 text-sm">
        <div className="flex justify-between">
          <span className="text-gray-500">协议</span>
          <span className="font-medium text-gray-800">FTP (被动模式)</span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">地址</span>
          <span className="font-medium text-gray-800 font-mono">
            {serverInfo.url}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">用户名</span>
          <span className="font-medium text-gray-800">
            {serverInfo.username}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">密码</span>
          <span className="font-medium text-gray-800">
            {serverInfo.password_info}
          </span>
        </div>
      </div>

      <button
        onClick={copyConnectionInfo}
        className="mt-4 w-full py-2 px-4 bg-gray-50 text-gray-700 rounded-lg flex items-center justify-center gap-2 hover:bg-gray-100 transition-colors"
      >
        {copied ? (
          <>
            <Check className="w-4 h-4" />
            已复制
          </>
        ) : (
          <>
            <Copy className="w-4 h-4" />
            复制连接信息
          </>
        )}
      </button>
    </div>
  );
}
```

**Step 5: Commit**

```bash
git add .
git commit -m "feat(ui): add server card, stats card, and info card components"
```

---

### 任务 9: 更新主应用

**Files:**
- Modify: `src/App.tsx`

**Step 1: 重写App组件**

Edit: `src/App.tsx`
```typescript
import { useState } from 'react';
import { ServerCard } from './components/ServerCard';
import { StatsCard } from './components/StatsCard';
import { InfoCard } from './components/InfoCard';
import { ServerInfo } from './types';
import { Camera } from 'lucide-react';

function App() {
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);

  return (
    <div className="min-h-screen bg-gray-50">
      <div className="max-w-md mx-auto p-4">
        {/* Header */}
        <header className="text-center py-6">
          <div className="w-16 h-16 bg-blue-600 rounded-2xl flex items-center justify-center mx-auto mb-3">
            <Camera className="w-8 h-8 text-white" />
          </div>
          <h1 className="text-2xl font-bold text-gray-900">
            图传伴侣
          </h1>
          <p className="text-sm text-gray-500 mt-1">
            Camera FTP Companion
          </p>
        </header>

        {/* Main Content */}
        <div className="space-y-4">
          <ServerCard onStatusChange={setServerInfo} />
          <StatsCard />
          <InfoCard serverInfo={serverInfo} />
        </div>

        {/* Footer */}
        <footer className="text-center py-6 text-xs text-gray-400">
          <p>© 2025 Camera FTP Companion</p>
          <p className="mt-1">让摄影工作流更简单</p>
        </footer>
      </div>
    </div>
  );
}

export default App;
```

**Step 2: 测试开发模式**

```bash
cargo tauri dev
```
Expected: 应用打开，显示三个卡片

**Step 3: Commit**

```bash
git add .
git commit -m "feat(app): integrate components into main app layout"
```

---

## 阶段 7: 平台适配

### 任务 10: Windows系统托盘

**Files:**
- Create: `src-tauri/src/platform/windows.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建Windows平台模块**

Create: `src-tauri/src/platform/windows.rs`
```rust
use tauri::{AppHandle, Manager};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tracing::error;

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let show_i = MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
    let hide_i = MenuItem::with_id(app, "hide", "隐藏", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    
    let menu = Menu::with_items(app, &[&show_i, &hide_i, &quit_i])?;
    
    // Build tray icon
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .menu_on_left_click(true)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "hide" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;
    
    Ok(())
}
```

**Step 2: 创建平台入口模块**

Create: `src-tauri/src/platform/mod.rs`
```rust
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "android")]
pub mod android;
```

**Step 3: 更新lib.rs集成托盘**

Edit: `src-tauri/src/lib.rs`
```rust
pub mod commands;
pub mod config;
pub mod ftp;
pub mod network;
pub mod platform;

// ... imports

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(FtpServerState(Mutex::new(None)))
        .setup(|app| {
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = platform::windows::setup_tray(app.handle()) {
                    eprintln!("Failed to setup tray: {}", e);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            get_server_status,
            get_network_info,
            load_config,
            save_config,
            check_port_available,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 4: Commit**

```bash
git add .
git commit -m "feat(platform): add windows system tray support"
```

---

### 任务 11: Android前台服务

**Files:**
- Create: `src-tauri/src/platform/android.rs`
- Create: `src-tauri/tauri-plugin-camera-ftp/` (插件目录)

**Step 1: 创建Android平台模块**

Create: `src-tauri/src/platform/android.rs`
```rust
use tauri::AppHandle;

/// Android前台服务需要在Java/Kotlin层实现
/// 这里提供Rust端的接口

pub fn start_foreground_service(_app: &AppHandle) {
    // Android前台服务需要在build.gradle中配置
    // 并通过JNI调用
    // 参见tauri-plugin-camera-ftp实现
}

pub fn stop_foreground_service(_app: &AppHandle) {
    // 同上
}
```

**Step 2: 添加Android配置文件**

Create: `src-tauri/gen/android/app/src/main/AndroidManifest.xml`
```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
    
    <application
        android:label="图传伴侣"
        android:name=".MainApplication"
        android:theme="@style/Theme.Tauri"
        android:usesCleartextTraffic="true">
        
        <activity
            android:name=".MainActivity"
            android:configChanges="orientation|keyboardHidden|keyboard|screenSize|smallestScreenSize|screenLayout">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
        
        <service
            android:name=".FtpForegroundService"
            android:enabled="true"
            android:exported="false"
            android:foregroundServiceType="dataSync" />
    </application>
</manifest>
```

**Step 3: Commit**

```bash
git add .
git commit -m "feat(platform): add android foreground service skeleton"
```

---

## 阶段 8: 构建和测试

### 任务 12: Windows构建

**Step 1: 生产构建**

```bash
cargo tauri build
```

**Step 2: 测试安装**

```bash
# 安装并测试
# 构建输出在 src-tauri/target/release/bundle/msi/
```

**Step 3: Commit**

```bash
git add .
git commit -m "chore(build): successful windows build"
```

---

### 任务 13: Android构建

**Step 1: 初始化Android项目**

```bash
cargo tauri android init
```

**Step 2: 开发模式测试**

```bash
cargo tauri android dev
```

**Step 3: 生产构建**

```bash
cargo tauri android build
```

**Step 4: Commit**

```bash
git add .
git commit -m "chore(build): successful android build"
```

---

## 测试清单

### 功能测试
- [ ] 启动服务器成功
- [ ] 自动检测IP地址
- [ ] 端口冲突自动切换
- [ ] 相机连接成功
- [ ] 照片上传成功
- [ ] 统计显示正确
- [ ] 停止服务器正常

### Windows特有测试
- [ ] 最小化到托盘
- [ ] 托盘菜单功能正常
- [ ] 窗口显示/隐藏

### Android特有测试
- [ ] 后台服务保活
- [ ] 通知栏显示
- [ ] 横竖屏切换

---

## 附录: 常见问题

### Q: 端口21需要管理员权限？
A: 使用1024以上的端口，或在代码中自动切换到可用端口

### Q: Android后台被系统杀死？
A: 需要实现前台服务(Foreground Service)并添加电池优化白名单

### Q: Windows Defender拦截？
A: 需要代码签名证书，或提示用户添加到白名单

---

**计划创建完成**

**保存位置:** `docs/plans/2025-02-18-tauri-rust-implementation-plan.md`
