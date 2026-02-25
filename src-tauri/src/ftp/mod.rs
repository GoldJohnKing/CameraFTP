//! FTP服务器模块
//!
//! 该模块提供了完整的FTP服务器功能，采用Actor模式实现，包括：
//! - 事件驱动架构（EventBus）
//! - 统计信息Actor（StatsActor）
//! - 服务器Actor（FtpServerActor）
//! - 监听器（Listeners）

pub mod error;
pub mod events;
pub mod listeners;
pub mod server;
pub mod server_factory;
pub mod stats;
pub mod types;

// 重新导出主要类型
pub use error::FtpError;
pub use events::{EventBus, EventProcessor, StatsEventHandler};
pub use server::{create_ftp_server, FtpServerActor, FtpServerHandle};
pub use server_factory::{
    spawn_event_processor, start_ftp_server, ServerStartupContext, ServerStartupOptions,
};
pub use stats::{StatsActor, StatsActorWorker};
pub use types::{
    DomainEvent, ServerConfig, ServerInfo, ServerStateSnapshot, ServerStatus,
    ServerStats, StopReason,
};
