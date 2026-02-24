#!/bin/bash
# Android Build Script for Camera FTP Companion
# 图传伴侣 Android 版本编译脚本 (仅 arm64-v8a 架构)

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的信息
info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

# 检查环境
check_environment() {
    info "检查编译环境..."
    
    # 检查 Java
    if ! command -v java &> /dev/null; then
        error "Java 未安装或未在 PATH 中"
        echo "请安装 Java 21:"
        echo "  sudo apt update"
        echo "  sudo apt install openjdk-21-jdk"
        exit 1
    fi
    
    JAVA_VERSION=$(java -version 2>&1 | head -n1 | cut -d'"' -f2)
    info "检测到 Java 版本: $JAVA_VERSION"
    
    # 检查 Android SDK
    if [ -z "$ANDROID_HOME" ] && [ -z "$ANDROID_SDK_ROOT" ]; then
        warn "Android SDK 环境变量未设置"
        echo "请设置 ANDROID_HOME 环境变量:"
        echo "  export ANDROID_HOME=\$HOME/Android/Sdk"
        echo "  export PATH=\$PATH:\$ANDROID_HOME/platform-tools:\$ANDROID_HOME/cmdline-tools/latest/bin"
        exit 1
    fi
    
    SDK_PATH=${ANDROID_HOME:-$ANDROID_SDK_ROOT}
    info "Android SDK 路径: $SDK_PATH"
    
    # 检查 bun
    if ! command -v bun &> /dev/null; then
        error "Bun 未安装"
        echo "请安装 Bun: https://bun.sh"
        exit 1
    fi
    
    # 检查 Rust
    if ! command -v rustc &> /dev/null; then
        error "Rust 未安装"
        echo "请安装 Rust: https://rustup.rs"
        exit 1
    fi
    
    # 检查 NDK
    if [ -z "$NDK_HOME" ]; then
        # 尝试自动查找
        if [ -d "$SDK_PATH/ndk" ]; then
            NDK_VERSION=$(ls $SDK_PATH/ndk | head -1)
            export NDK_HOME="$SDK_PATH/ndk/$NDK_VERSION"
            info "自动设置 NDK_HOME: $NDK_HOME"
        else
            warn "NDK_HOME 环境变量未设置"
        fi
    fi
    
    success "环境检查完成"
}

# 检查或创建签名密钥
check_or_create_keystore() {
    local keystore_path="src-tauri/gen/android/keystore.properties"
    local keystore_file="camera-ftp-companion.keystore"
    
    if [ ! -f "$keystore_path" ]; then
        warn "签名配置不存在，创建新的签名密钥..."
        
        # 生成密钥库
        keytool -genkey -v \
            -keystore "$keystore_file" \
            -alias "cameraftp" \
            -keyalg RSA \
            -keysize 2048 \
            -validity 10000 \
            -dname "CN=Camera FTP Companion, OU=Development, O=GJK, L=Unknown, ST=Unknown, C=CN" \
            -storepass "cameraftp123" \
            -keypass "cameraftp123"
        
        # 移动密钥到 Android 项目目录
        mv "$keystore_file" "src-tauri/gen/android/$keystore_file"
        
        # 创建 keystore.properties
        cat > "$keystore_path" << EOF
storeFile=$keystore_file
storePassword=cameraftp123
keyAlias=cameraftp
keyPassword=cameraftp123
EOF
        
        success "签名密钥已创建: src-tauri/gen/android/$keystore_file"
        info "密钥信息已保存到: $keystore_path"
    else
        info "签名配置已存在"
    fi
}

# 构建 Android 应用
build_android() {
    local build_type=$1
    
    info "开始构建 Android 应用 ($build_type) - 仅 arm64-v8a 架构"
    
    # 设置环境变量
    export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
    export ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
    if [ -z "$NDK_HOME" ]; then
        NDK_VERSION=$(ls $ANDROID_HOME/ndk | head -1)
        export NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
    fi
    export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"
    
    case $build_type in
        "debug")
            # Debug 也使用签名，便于测试
            check_or_create_keystore
            
            bun run tauri android build --debug --target aarch64
            success "Debug APK 构建完成（已签名）"
            info "APK 位置: src-tauri/gen/android/app/build/outputs/apk/"
            ;;
        "release")
            # 检查签名
            check_or_create_keystore
            
            bun run tauri android build --target aarch64
            success "Release APK 构建完成"
            info "APK 位置: src-tauri/gen/android/app/build/outputs/apk/"
            ;;
        "aab")
            check_or_create_keystore
            info "构建 Android App Bundle (AAB) - 仅 arm64-v8a"
            bun run tauri android build --aab --target aarch64
            success "AAB 构建完成"
            info "AAB 位置: src-tauri/gen/android/app/build/outputs/bundle/"
            ;;
    esac
}

# 开发模式运行
dev_mode() {
    info "启动 Android 开发模式 (arm64-v8a)..."
    
    export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
    export ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
    if [ -z "$NDK_HOME" ]; then
        NDK_VERSION=$(ls $ANDROID_HOME/ndk | head -1)
        export NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
    fi
    export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"
    
    bun run tauri android dev --target aarch64
}

# 列出已连接的设备
list_devices() {
    info "已连接的 Android 设备:"
    adb devices -l
}

# 安装 APK 到设备
install_apk() {
    local apk_path=$1
    
    if [ -z "$apk_path" ]; then
        # 自动查找最新构建的 APK
        apk_path=$(find src-tauri/gen/android/app/build/outputs -name "*.apk" -type f | head -1)
    fi
    
    if [ -f "$apk_path" ]; then
        info "安装 APK: $apk_path"
        adb install -r "$apk_path"
        success "安装完成"
    else
        error "未找到 APK 文件: $apk_path"
        exit 1
    fi
}

# 查看 APK 签名信息
show_apk_info() {
    local apk_path=$1
    
    if [ -z "$apk_path" ]; then
        apk_path=$(find src-tauri/gen/android/app/build/outputs -name "*.apk" -type f | head -1)
    fi
    
    if [ -f "$apk_path" ]; then
        info "APK 信息: $apk_path"
        echo ""
        info "包名:"
        $ANDROID_HOME/build-tools/*/aapt dump badging "$apk_path" | grep package
        echo ""
        info "签名信息:"
        keytool -list -printcert -jarfile "$apk_path" 2>/dev/null || jarsigner -verify -verbose -certs "$apk_path" 2>/dev/null || echo "无法读取签名信息"
    else
        error "未找到 APK 文件"
        exit 1
    fi
}

# 主函数
main() {
    echo "=========================================="
    echo "  图传伴侣 (Camera FTP Companion)"
    echo "  Android 版本编译脚本"
    echo "  架构: arm64-v8a (64位 ARM)"
    echo "=========================================="
    echo ""
    
    # 切换到项目根目录
    cd "$(dirname "$0")"
    
    case "${1:-}" in
        "check"|"env")
            check_environment
            ;;
        "debug")
            check_environment
            build_android debug
            ;;
        "release"|"")
            check_environment
            build_android release
            ;;
        "aab")
            check_environment
            build_android aab
            ;;
        "dev")
            check_environment
            dev_mode
            ;;
        "devices")
            list_devices
            ;;
        "install")
            install_apk "${2:-}"
            ;;
        "info"|"apk-info")
            show_apk_info "${2:-}"
            ;;
        "keystore"|"sign")
            check_environment
            check_or_create_keystore
            ;;
        "help"|"-h"|"--help"|"")
            echo "用法: $0 <命令>"
            echo ""
            echo "命令:"
            echo "  check, env    检查编译环境"
            echo "  (无参数)      构建 Release APK (签名，默认)"
            echo "  debug         构建 Debug APK (arm64-v8a)"
            echo "  release       构建 Release APK (签名)"
            echo "  aab           构建 Android App Bundle"
            echo "  dev           开发模式（热重载）"
            echo "  keystore      创建签名密钥"
            echo "  devices       列出已连接的设备"
            echo "  install [apk] 安装 APK 到设备"
            echo "  info [apk]    查看 APK 信息"
            echo "  help          显示此帮助信息"
            echo ""
            echo "示例:"
            echo "  $0 check       # 检查环境"
            echo "  $0 debug       # 构建 Debug 版本"
            echo "  $0 release     # 构建签名 Release 版本"
            echo "  $0 aab         # 构建 AAB (Google Play)"
            echo ""
            echo "输出位置:"
            echo "  APK: src-tauri/gen/android/app/build/outputs/apk/"
            echo "  AAB: src-tauri/gen/android/app/build/outputs/bundle/"
            echo ""
            echo "包名: com.gjk.camera-ftp-companion"
            echo "架构: arm64-v8a (64位 ARM处理器)"
            echo ""
            ;;
        *)
            error "未知命令: $1"
            echo "运行 '$0 help' 查看帮助"
            exit 1
            ;;
    esac
}

main "$@"
