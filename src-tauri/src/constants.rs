/// 应用常量定义

pub mod ftp {
    /// FTP 服务器默认端口
    pub const DEFAULT_PORT: u16 = 2121;

    /// FTP 被动模式端口范围
    pub const PASSIVE_PORT_RANGE: (u16, u16) = (50000, 50100);

    /// 空闲连接超时时间（秒）
    pub const IDLE_TIMEOUT_SECONDS: u64 = 600;
}

pub mod paths {
    /// 应用名称
    pub const APP_NAME: &str = "camera-ftp-companion";

    /// 默认存储目录名称
    pub const DEFAULT_STORAGE_DIR: &str = "CameraFTP";

    /// 配置文件名称
    pub const CONFIG_FILE_NAME: &str = "config.json";
}

pub mod platform {
    /// Windows 平台特定常量
    pub mod windows {
        /// 开机自启动注册表项名称
        pub const AUTOSTART_REG_KEY: &str = "CameraFTPCompanion";
    }

    /// Android 平台特定常量
    pub mod android {
        /// 外部存储根目录
        pub const EXTERNAL_STORAGE_ROOT: &str = "/storage/emulated/0";

        /// DCIM 目录路径
        pub const DCIM_DIR: &str = "DCIM/CameraFTP";

        /// 日志目录路径（相对于外部存储）
        pub const LOG_DIR: &str = "DCIM/CameraFTP/logs";

        /// 测试文件名称（用于权限检查）
        pub const PERMISSION_TEST_FILE: &str = ".permission_test";
    }
}

pub mod defaults {
    /// 服务器统计信息默认值
    pub const DEFAULT_CONNECTED_CLIENTS: u32 = 0;
    pub const DEFAULT_FILES_RECEIVED: u32 = 0;
    pub const DEFAULT_BYTES_RECEIVED: u64 = 0;
}
