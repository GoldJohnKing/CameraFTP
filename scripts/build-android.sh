#!/bin/bash
# build-android.sh - Android 构建脚本
# 由 build.sh 调用，不生成类型绑定
set -e

# 引入公共函数库
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

# 切换到项目根目录（scripts/的父目录）
cd "$SCRIPT_DIR/.."

# ============================================
# 环境检查与设置
# ============================================

check_android_env() {
    info "检查 Android 编译环境..."
    local failed=false
    
    # 检查 bun
    if ! check_bun; then
        failed=true
    fi
    
    # 检查 cargo
    if ! command -v cargo &> /dev/null; then
        error "cargo 未找到"
        echo "请确保 Rust 已安装: https://rustup.rs"
        failed=true
    else
        info "Cargo: $(cargo --version)"
    fi
    
    # 检查 Java
    if ! command -v java &> /dev/null; then
        error "Java 未安装"
        echo "安装: sudo apt install openjdk-21-jdk"
        failed=true
    else
        info "Java: $(java -version 2>&1 | head -n1)"
    fi
    
    # 检查 Android SDK
    if [ -z "$ANDROID_HOME" ] && [ -z "$ANDROID_SDK_ROOT" ]; then
        error "ANDROID_HOME 环境变量未设置"
        echo "设置: export ANDROID_HOME=\$HOME/Android/Sdk"
        failed=true
    else
        info "Android SDK: ${ANDROID_HOME:-$ANDROID_SDK_ROOT}"
    fi
    
    # 设置 NDK_HOME（用于提示）
    if [ -z "$NDK_HOME" ]; then
        local sdk_path="${ANDROID_HOME:-$ANDROID_SDK_ROOT}"
        if [ -d "$sdk_path/ndk" ]; then
            local ndk_version=$(ls "$sdk_path/ndk" | head -1)
            info "NDK: $sdk_path/ndk/$ndk_version (自动检测)"
        else
            warn "NDK 未找到，首次编译时会自动下载"
        fi
    else
        info "NDK: $NDK_HOME"
    fi
    
    if [ "$failed" = true ]; then
        return 1
    fi
    
    success "Android 环境检查通过"
    return 0
}

# 设置 Android 编译环境变量
setup_android_env() {
    export JAVA_HOME=${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk-amd64}
    export ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}

    if [ -z "$NDK_HOME" ] && [ -d "$ANDROID_HOME/ndk" ]; then
        local ndk_version=$(ls "$ANDROID_HOME/ndk" | head -1)
        export NDK_HOME="$ANDROID_HOME/ndk/$ndk_version"
    fi

    export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"

    # Gradle 优化：并行构建 + 按需配置
    export GRADLE_OPTS="-Dorg.gradle.parallel=true -Dorg.gradle.configureondemand=true"
}

# ============================================
# 签名密钥
# ============================================

check_or_create_keystore() {
    local keystore_path="src-tauri/gen/android/keystore.properties"
    local keystore_file="camera-ftp-companion.keystore"
    
    if [ ! -f "$keystore_path" ]; then
        warn "签名配置不存在，创建新的签名密钥..."
        
        # 生成密钥库 (使用选中的 keytool)
        local keytool_cmd="${SELECTED_TOOLS[keytool]:-keytool}"
        $keytool_cmd -genkey -v \
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

# ============================================
# 构建
# ============================================

build_android() {
    local BUILD_TYPE="${1:-release}"
    
    info "开始构建 Android 应用 ($BUILD_TYPE) - 仅 arm64-v8a 架构"
    
    # 检查是否已由 build.sh 统一构建前端
    if [ "${FRONTEND_ALREADY_BUILT}" = "1" ]; then
        info "前端已由 build.sh 统一构建，Android 将复用（Tauri 可能仍会检查）..."
    fi
    
    # 设置环境变量
    setup_android_env
    
    check_or_create_keystore
    
    case $BUILD_TYPE in
        "debug")
            bun run tauri android build --debug --target aarch64
            copy_pattern_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/debug/*.apk" \
                "camera-ftp-companion-debug.apk" \
                "Debug APK"
            ;;
        "release")
            bun run tauri android build --target aarch64
            copy_pattern_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk" \
                "camera-ftp-companion.apk" \
                "Release APK"
            ;;
    esac
}

# ============================================
# 开发模式
# ============================================

dev_mode() {
    info "启动 Android 开发模式 (arm64-v8a)..."
    setup_android_env
    bun run tauri android dev --target aarch64
}

# 列出已连接的设备
list_devices() {
    info "已连接的 Android 设备:"
    local adb_cmd="${SELECTED_TOOLS[adb]:-adb}"
    $adb_cmd devices -l
}

# 安装 APK 到设备
install_apk() {
    local apk_path=$1
    
    if [ -z "$apk_path" ]; then
        # 优先从 out 目录查找
        if [ -f "$OUTPUT_DIR/camera-ftp-companion.apk" ]; then
            apk_path="$OUTPUT_DIR/camera-ftp-companion.apk"
        elif [ -f "$OUTPUT_DIR/camera-ftp-companion-debug.apk" ]; then
            apk_path="$OUTPUT_DIR/camera-ftp-companion-debug.apk"
        else
            # 回退到构建目录
            apk_path=$(find src-tauri/gen/android/app/build/outputs -name "*.apk" -type f | head -1)
        fi
    fi
    
    if [ -f "$apk_path" ]; then
        info "安装 APK: $apk_path"
        local adb_cmd="${SELECTED_TOOLS[adb]:-adb}"
        $adb_cmd install -r "$apk_path"
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
        # 优先从 out 目录查找
        if [ -f "$OUTPUT_DIR/camera-ftp-companion.apk" ]; then
            apk_path="$OUTPUT_DIR/camera-ftp-companion.apk"
        elif [ -f "$OUTPUT_DIR/camera-ftp-companion-debug.apk" ]; then
            apk_path="$OUTPUT_DIR/camera-ftp-companion-debug.apk"
        else
            apk_path=$(find src-tauri/gen/android/app/build/outputs -name "*.apk" -type f | head -1)
        fi
    fi
    
    if [ -f "$apk_path" ]; then
        info "APK 信息: $apk_path"
        echo ""
        info "包名:"
        $ANDROID_HOME/build-tools/*/aapt dump badging "$apk_path" 2>/dev/null | grep package || true
        echo ""
        info "签名信息:"
        local keytool_cmd="${SELECTED_TOOLS[keytool]:-keytool}"
        $keytool_cmd -list -printcert -jarfile "$apk_path" 2>/dev/null || \
            jarsigner -verify -verbose -certs "$apk_path" 2>/dev/null || \
            echo "无法读取签名信息"
    else
        error "未找到 APK 文件"
        exit 1
    fi
}

# ============================================
# 帮助
# ============================================

show_standalone_help() {
    cat << EOF
用法: ./build-android.sh [选项] [命令]

选项:
  --release         构建 Release 版本 (默认)
  --debug           构建 Debug 版本
  --check           仅检查环境，不编译
  --help, -h        显示此帮助信息

命令:
  dev               开发模式（热重载）
  devices           列出已连接的设备
  install [apk]     安装 APK 到设备
  info [apk]        查看 APK 信息
  keystore          创建签名密钥

示例:
  ./build-android.sh                 # 构建 Release 版本
  ./build-android.sh --debug         # 构建 Debug 版本
  ./build-android.sh --check         # 检查编译环境
  ./build-android.sh dev             # 开发模式

输出位置:
  Release: out/camera-ftp-companion.apk
  Debug:   out/camera-ftp-companion-debug.apk

包名: com.gjk.camera-ftp-companion
架构: arm64-v8a (64位 ARM处理器)

注意: 推荐使用 ./build.sh android 进行构建，会自动生成类型绑定
EOF
}

# ============================================
# 主函数
# ============================================

main() {
    local BUILD_TYPE="release"
    local CHECK_ONLY=false
    local COMMAND=""
    
    # 解析参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            --release)
                BUILD_TYPE="release"
                shift
                ;;
            --debug)
                BUILD_TYPE="debug"
                shift
                ;;
            --check)
                CHECK_ONLY=true
                shift
                ;;
            --help|-h)
                show_standalone_help
                exit 0
                ;;
            dev|devices|install|info|keystore)
                COMMAND="$1"
                shift
                ;;
            *)
                error "未知参数: $1"
                show_standalone_help
                exit 1
                ;;
        esac
    done
    
    # 处理 --check
    if [ "$CHECK_ONLY" = true ]; then
        check_android_env
        exit $?
    fi
    
    # 处理命令
    case "$COMMAND" in
        dev)
            check_android_env && dev_mode
            ;;
        devices)
            list_devices
            ;;
        install)
            install_apk
            ;;
        info)
            show_apk_info
            ;;
        keystore)
            check_android_env && check_or_create_keystore
            ;;
        *)
            # 默认构建
            check_android_env && build_android "$BUILD_TYPE"
            ;;
    esac
}

main "$@"
