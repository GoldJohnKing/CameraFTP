/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

interface Dependency {
  name: string;
  description: string;
  url: string;
}

export interface DependencyGroup {
  title: string;
  deps: Dependency[];
}

export const DEPENDENCIES: DependencyGroup[] = [
  {
    title: '应用框架',
    deps: [
      {
        name: 'Tauri',
        description: '使用 Web 前端构建桌面/移动应用的框架',
        url: 'https://tauri.app/',
      },
      {
        name: 'React',
        description: '用于构建用户界面的 JavaScript 库',
        url: 'https://react.dev/',
      },
      {
        name: 'TailwindCSS',
        description: '实用优先的 CSS 框架',
        url: 'https://tailwindcss.com/',
      },
      {
        name: 'Lucide',
        description: '精美的开源图标库',
        url: 'https://lucide.dev/',
      },
      {
        name: 'Zustand',
        description: '轻量级 React 状态管理库',
        url: 'https://zustand.docs.pmnd.rs/',
      },
      {
        name: 'Sonner',
        description: '优雅的 Toast 通知组件',
        url: 'https://sonner.emilkowal.dev/',
      },
      {
        name: 'TypeScript',
        description: '前端开发语言，JavaScript 的类型安全超集',
        url: 'https://www.typescriptlang.org/',
      },
      {
        name: 'Vite',
        description: '前端构建工具与开发服务器',
        url: 'https://vitejs.dev/',
      },
      {
        name: 'tauri-plugin-dialog',
        description: 'Tauri 原生对话框插件（文件/消息选择）',
        url: 'https://v2.tauri.app/plugin/dialog/',
      },
    ],
  },
  {
    title: 'FTP 服务器',
    deps: [
      {
        name: 'libunftp',
        description: 'Rust 编写的异步 FTP 服务器库',
        url: 'https://docs.rs/libunftp/',
      },
      {
        name: 'unftp-sbe-fs',
        description: 'libunftp 的文件系统存储后端',
        url: 'https://docs.rs/unftp-sbe-fs/',
      },
      {
        name: 'unftp-core',
        description: 'libunftp 核心类型与接口',
        url: 'https://docs.rs/unftp-core/',
      },
      {
        name: 'tokio',
        description: 'Rust 异步运行时',
        url: 'https://tokio.rs/',
      },
      {
        name: 'tokio-util',
        description: 'Tokio 异步工具库',
        url: 'https://docs.rs/tokio-util/',
      },
    ],
  },
  {
    title: '图像与文件处理',
    deps: [
      {
        name: 'nom-exif',
        description: 'EXIF 元数据解析库',
        url: 'https://docs.rs/nom-exif/',
      },
      {
        name: 'image',
        description: 'Rust 图像处理库',
        url: 'https://docs.rs/image/',
      },
      {
        name: 'heic',
        description: '纯 Rust 实现的 HEIC/HEIF 图像解码器',
        url: 'https://docs.rs/heic/',
      },
      {
        name: 'notify',
        description: '跨平台文件系统事件监听库',
        url: 'https://docs.rs/notify/',
      },
      {
        name: 'zip',
        description: 'ZIP 压缩/解压库',
        url: 'https://docs.rs/zip/',
      },
      {
        name: 'flate2',
        description: 'DEFLATE 压缩/解压库',
        url: 'https://docs.rs/flate2/',
      },
      {
        name: 'memchr',
        description: 'SIMD 加速的字节扫描库',
        url: 'https://docs.rs/memchr/',
      },
    ],
  },
  {
    title: '网络与通信',
    deps: [
      {
        name: 'reqwest',
        description: 'Rust HTTP 客户端库',
        url: 'https://docs.rs/reqwest/',
      },
      {
        name: 'local-ip-address',
        description: '获取本机 IP 地址和网络接口信息',
        url: 'https://docs.rs/local-ip-address/',
      },
      {
        name: 'rcgen',
        description: 'Rust TLS 证书生成库',
        url: 'https://docs.rs/rcgen/',
      },
    ],
  },
  {
    title: '安全与工具',
    deps: [
      {
        name: 'Argon2',
        description: 'Argon2id 密码哈希算法实现',
        url: 'https://docs.rs/argon2/',
      },
      {
        name: 'zeroize',
        description: '内存安全：敏感数据自动清零',
        url: 'https://docs.rs/zeroize/',
      },
      {
        name: 'rand_core',
        description: '随机数生成核心库',
        url: 'https://docs.rs/rand_core/',
      },
      {
        name: 'base64',
        description: 'Base64 编解码库',
        url: 'https://docs.rs/base64/',
      },
      {
        name: 'chrono',
        description: '日期和时间处理库',
        url: 'https://docs.rs/chrono/',
      },
      {
        name: 'libloading',
        description: '动态库加载器',
        url: 'https://docs.rs/libloading/',
      },
      {
        name: 'dirs',
        description: '跨平台标准目录路径解析（配置/数据目录）',
        url: 'https://docs.rs/dirs/',
      },
    ],
  },
  {
    title: 'Rust 核心库',
    deps: [
      {
        name: 'serde',
        description: 'Rust 序列化/反序列化框架',
        url: 'https://serde.rs/',
      },
      {
        name: 'tracing',
        description: '结构化日志与诊断库',
        url: 'https://docs.rs/tracing/',
      },
      {
        name: 'tracing-subscriber',
        description: '日志订阅与格式化输出层（tracing 的配套）',
        url: 'https://docs.rs/tracing-subscriber/',
      },
      {
        name: 'thiserror',
        description: 'Rust 错误类型派生宏',
        url: 'https://docs.rs/thiserror/',
      },
      {
        name: 'dashmap',
        description: '并发哈希表',
        url: 'https://docs.rs/dashmap/',
      },
      {
        name: 'async-trait',
        description: '异步 trait 方法支持',
        url: 'https://docs.rs/async-trait/',
      },
      {
        name: 'futures',
        description: 'Rust 异步工具库',
        url: 'https://docs.rs/futures/',
      },
      {
        name: 'ts-rs',
        description: 'Rust 类型到 TypeScript 类型绑定生成',
        url: 'https://docs.rs/ts-rs/',
      },
    ],
  },
  {
    title: '平台原生接口',
    deps: [
      {
        name: 'windows-rs',
        description: 'Windows Win32 API 绑定（系统托盘 / Shell / 网络接口）',
        url: 'https://github.com/microsoft/windows-rs',
      },
      {
        name: 'winreg',
        description: 'Windows 注册表读写（开机自启等配置）',
        url: 'https://docs.rs/winreg/',
      },
      {
        name: 'jni',
        description: 'Java Native Interface，Rust ↔ Kotlin 互操作桥（Android）',
        url: 'https://docs.rs/jni/',
      },
      {
        name: 'ndk-context',
        description: 'Android NDK 上下文（JNI 运行环境）',
        url: 'https://docs.rs/ndk-context/',
      },
    ],
  },
  {
    title: 'RAW 调色引擎',
    deps: [
      {
        name: 'RawAlchemyCpp',
        description: 'RAW 调色与神经网络去马赛克引擎（C++ 动态库，FFI 调用），系 Raw-Alchemy 的 C++ 重写版',
        url: 'https://github.com/GoldJohnKing/RawAlchemyCpp',
      },
      {
        name: 'Raw-Alchemy',
        description: 'RawAlchemyCpp 的参考项目（Python 原版），调色管线与色彩科学的源头',
        url: 'https://github.com/shenmintao/Raw-Alchemy',
      },
      {
        name: 'darktable',
        description: 'RawAlchemyCpp 移植了其 X-Trans Markesteijn/RCD 去马赛克及小波降噪算法',
        url: 'https://github.com/darktable-org/darktable',
      },
      {
        name: 'LibRaw',
        description: 'RAW 图像解码库（读取相机原始数据）',
        url: 'https://github.com/LibRaw/LibRaw',
      },
      {
        name: 'lensfun',
        description: '镜头校正数据库与算法库',
        url: 'https://github.com/lensfun/lensfun',
      },
      {
        name: 'pugixml',
        description: '轻量级 XML 解析库（用于 Lensfun 数据库）',
        url: 'https://github.com/zeux/pugixml',
      },
      {
        name: 'libexif',
        description: 'EXIF 元数据解析与序列化库',
        url: 'https://github.com/libexif/libexif',
      },
      {
        name: 'libtiff',
        description: 'TIFF 图像读写库（16-bit 输出）',
        url: 'https://gitlab.com/libtiff/libtiff',
      },
      {
        name: 'libjpeg-turbo',
        description: 'SIMD 加速的 JPEG 编解码库',
        url: 'https://github.com/libjpeg-turbo/libjpeg-turbo',
      },
    ],
  },
  {
    title: '神经网络推理',
    deps: [
      {
        name: 'ONNX Runtime',
        description: '跨平台神经网络推理引擎，加载去马赛克模型',
        url: 'https://onnxruntime.ai/',
      },
      {
        name: 'DirectML',
        description: 'Windows GPU 加速推理后端（DirectX 12）',
        url: 'https://github.com/microsoft/DirectML',
      },
      {
        name: 'Qualcomm QNN',
        description: 'Android 端 Qualcomm 神经网络推理后端（Hexagon HTP 加速）',
        url: 'https://www.qualcomm.com/developer/software/qualcomm-ai-engine-direct',
      },
    ],
  },
  {
    title: '神经网络模型权重',
    deps: [
      {
        name: 'x-veon · bayer.onnx',
        description: 'Bayer 传感器神经网络去马赛克模型（U-Net，FP16，约 1.94M 参数）。⚠️ 上游未声明许可证',
        url: 'https://github.com/naorunaoru/x-veon',
      },
      {
        name: 'x-veon · xtrans.onnx',
        description: 'X-Trans 传感器神经网络去马赛克模型（U-Net，FP16，约 7.76M 参数）。⚠️ 上游未声明许可证',
        url: 'https://github.com/naorunaoru/x-veon',
      },
    ],
  },
];
