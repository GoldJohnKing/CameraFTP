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
pub use events::{EventBus, EventBusConfig, EventProcessor, StatsEventHandler};
pub use server::{create_ftp_server, FtpServerActor, FtpServerHandle};
pub use server_factory::{
    spawn_event_processor, start_ftp_server, ServerStartupContext, ServerStartupOptions,
};
pub use stats::{StatsActor, StatsActorWorker};
pub use types::{
    DomainEvent, ServerConfig, ServerInfo, ServerStateSnapshot, ServerStatus,
    ServerStats, StopReason,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_status_transitions() {
        assert!(ServerStatus::Stopped.can_start());
        assert!(!ServerStatus::Stopped.can_stop());

        assert!(!ServerStatus::Running.can_start());
        assert!(ServerStatus::Running.can_stop());

        assert!(!ServerStatus::Starting.can_start());
        assert!(!ServerStatus::Starting.can_stop());

        assert!(!ServerStatus::Stopping.can_start());
        assert!(!ServerStatus::Stopping.can_stop());
    }

    #[test]
    fn test_server_stats_has_changed() {
        let stats1 = ServerStats {
            active_connections: 1,
            total_uploads: 10,
            total_bytes_received: 1000,
            last_uploaded_file: Some("test.jpg".to_string()),
        };

        let stats2 = ServerStats {
            active_connections: 1,
            total_uploads: 10,
            total_bytes_received: 1000,
            last_uploaded_file: Some("test.jpg".to_string()),
        };

        let stats3 = ServerStats {
            active_connections: 2,
            total_uploads: 11,
            total_bytes_received: 1500,
            last_uploaded_file: Some("test2.jpg".to_string()),
        };

        assert!(!stats1.has_changed(&stats2));
        assert!(stats1.has_changed(&stats3));
    }

    #[tokio::test]
    async fn test_stats_actor() {
        let (handle, mut worker) = StatsActor::new();

        // 在后台运行worker
        let worker_task = tokio::spawn(async move {
            worker.run().await;
        });

        // 测试记录上传
        handle.record_upload("test.jpg".to_string(), 1024).await;

        // 获取统计
        let stats = handle.get_stats_direct().await;
        assert_eq!(stats.total_uploads, 1);

        // 停止worker
        drop(handle);
        let _ = worker_task.await;
    }

    #[test]
    fn test_event_bus() {
        let bus = EventBus::new();

        let mut rx = bus.subscribe();

        bus.emit_server_started("127.0.0.1:21".to_string());

        // 注意：由于广播通道的特性，这里只能测试发送是否成功
        // 在实际使用中，接收方会在另一个任务中接收事件
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_event_bus_incremental_update() {
        use tokio::runtime::Runtime;

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let bus = EventBus::new();

            // 第一次更新应该发布
            let stats1 = ServerStats {
                active_connections: 1,
                total_uploads: 10,
                total_bytes_received: 1000,
                last_uploaded_file: Some("test.jpg".to_string()),
            };
            bus.emit_stats_updated(stats1.clone()).await;

            // 相同的统计不应该发布
            bus.emit_stats_updated(stats1).await;

            // 不同的统计应该发布
            let stats2 = ServerStats {
                active_connections: 2,
                total_uploads: 11,
                total_bytes_received: 1500,
                last_uploaded_file: Some("test2.jpg".to_string()),
            };
            bus.emit_stats_updated(stats2).await;
        });
    }
}
