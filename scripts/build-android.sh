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

# 存储选中的工具和路径
declare -A SELECTED_TOOLS
declare -A SELECTED_PATHS

# 检测工具并存储命令
# 用法: detect_tool <tool_name>
detect_tool() {
    local tool_name="${1:-}"
    if [ -z "$tool_name" ]; then
        return 1
    fi
    local cmd
    if cmd=$(get_tool_cmd "$tool_name"); then
        SELECTED_TOOLS[$tool_name]="$cmd"
        return 0
    fi
    return 1
}

# 检查混合工具链（Windows/Linux 混用）
check_mixed_toolchain() {
    local platforms=()
    local cargo_platform java_platform sdk_platform
    
    # 收集各工具的平台
    cargo_platform=$(get_tool_platform "cargo")
    if [ -n "$cargo_platform" ]; then
        platforms+=("cargo:$cargo_platform")
    fi
    
    java_platform=$(get_tool_platform "java")
    if [ -n "$java_platform" ]; then
        platforms+=("java:$java_platform")
    fi
    
    # 根据 SDK 路径判断平台
    if [ -n "${SELECTED_PATHS[android_sdk]}" ]; then
        if [[ "${SELECTED_PATHS[android_sdk]}" == /mnt/*/ ]]; then
            sdk_platform="windows"
        else
            sdk_platform="linux"
        fi
        platforms+=("sdk:$sdk_platform")
    fi
    
    # 检查是否混合使用
    local has_windows=false
    local has_linux=false
    
    for item in "${platforms[@]}"; do
        if [[ "$item" == *":windows" ]]; then
            has_windows=true
        elif [[ "$item" == *":linux" ]]; then
            has_linux=true
        fi
    done
    
    if [ "$has_windows" = true ] && [ "$has_linux" = true ]; then
        warn "检测到混合工具链 (Windows + Linux):"
        for item in "${platforms[@]}"; do
            warn "  - $item"
        done
        warn "这可能导致兼容性问题，建议统一使用同一平台的工具"
        echo ""
    fi
}

check_android_env() {
    info "检查 Android 编译环境..."
    local failed=false
    
    # 通用工具 (bun, cargo) 已由 build.sh 检查
    
    # 检查 java (使用工具选择层)
    if detect_tool "java"; then
        check_tool "java" "Java" || failed=true
    else
        error "Java 未安装"
        echo "安装: sudo apt install openjdk-21-jdk (Linux)"
        echo "或下载: https://adoptium.net/ (Windows)"
        failed=true
    fi
    
    # 检查 javac (必需，用于编译自动生成的 Java 代码)
    if detect_tool "javac"; then
        check_tool "javac" "Javac" || failed=true
    else
        error "javac 未找到，请安装完整 JDK（不是仅 JRE）"
        echo "安装: sudo apt install openjdk-21-jdk (Linux)"
        echo "或下载: https://adoptium.net/ (Windows)"
        failed=true
    fi
    
    # 检查 keytool (可选，仅警告)
    if detect_tool "keytool"; then
        check_tool "keytool" "Keytool" || warn "keytool 未找到，签名功能可能受影响"
    else
        warn "keytool 未找到，签名功能可能受影响"
    fi
    
    # 检测 Android SDK (优先 Windows，回退 Linux)
    local sdk_path
    if sdk_path=$(detect_windows_android_sdk); then
        SELECTED_PATHS[android_sdk]="$sdk_path"
        info "Android SDK: $sdk_path [windows]"
    elif sdk_path=$(detect_linux_android_sdk); then
        SELECTED_PATHS[android_sdk]="$sdk_path"
        info "Android SDK: $sdk_path [linux]"
    else
        error "Android SDK 未找到"
        echo "设置: export ANDROID_HOME=\$HOME/Android/Sdk (Linux)"
        echo "或安装 Android Studio (Windows)"
        failed=true
    fi
    
    # 检测 NDK
    if [ -n "${SELECTED_PATHS[android_sdk]}" ]; then
        local ndk_path
        if ndk_path=$(detect_ndk_from_sdk "${SELECTED_PATHS[android_sdk]}"); then
            SELECTED_PATHS[android_ndk]="$ndk_path"
            info "NDK: $ndk_path"
        else
            warn "NDK 未找到，首次编译时会自动下载"
        fi
    fi
    
    # 检测 Java Home (根据 SDK 平台选择，而非 java 命令平台)
    # 原因: gradlew 需要与 SDK 在同一平台运行
    local java_home
    local sdk_path="${SELECTED_PATHS[android_sdk]}"
    
    if [[ "$sdk_path" == /mnt/* ]]; then
        # SDK 在 Windows → 使用 Windows JAVA_HOME
        if java_home=$(detect_windows_java_home); then
            SELECTED_PATHS[java_home]="$java_home"
            info "JAVA_HOME: $java_home [windows] (跟随 SDK 平台)"
        else
            warn "Windows JAVA_HOME 未检测到，将使用环境变量"
        fi
    else
        # SDK 在 Linux → 使用 Linux JAVA_HOME
        if java_home=$(detect_linux_java_home); then
            SELECTED_PATHS[java_home]="$java_home"
            info "JAVA_HOME: $java_home [linux] (跟随 SDK 平台)"
        else
            warn "Linux JAVA_HOME 未检测到，将使用环境变量"
        fi
    fi
    
    # 检查混合工具链
    check_mixed_toolchain
    
    if [ "$failed" = true ]; then
        return 1
    fi
    
    success "Android 环境检查通过"
    return 0
}

# 设置 Android 编译环境变量
setup_android_env() {
    # 设置 JAVA_HOME (优先使用检测到的路径)
    if [ -n "${SELECTED_PATHS[java_home]}" ]; then
        export JAVA_HOME="${SELECTED_PATHS[java_home]}"
    else
        export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk-amd64}"
    fi
    
    # 设置 Android SDK
    if [ -n "${SELECTED_PATHS[android_sdk]}" ]; then
        export ANDROID_HOME="${SELECTED_PATHS[android_sdk]}"
        export ANDROID_SDK_ROOT="${SELECTED_PATHS[android_sdk]}"
    else
        export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
        export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
    fi
    
    # 设置 NDK_HOME
    if [ -n "${SELECTED_PATHS[android_ndk]}" ]; then
        export NDK_HOME="${SELECTED_PATHS[android_ndk]}"
    elif [ -z "$NDK_HOME" ] && [ -d "$ANDROID_HOME/ndk" ]; then
        if [ ! "$(ls -A "$ANDROID_HOME/ndk" 2>/dev/null)" ]; then
            warn "NDK 目录存在但为空"
        fi
        local ndk_version
        for ndk_version in "$ANDROID_HOME/ndk"/*; do
            if [ -d "$ndk_version" ]; then
                export NDK_HOME="$ndk_version"
                break
            fi
        done
    fi
    
    # 更新 PATH (仅添加存在的目录)
    local new_paths=()
    [ -d "$JAVA_HOME/bin" ] && new_paths+=("$JAVA_HOME/bin")
    [ -d "$ANDROID_HOME/platform-tools" ] && new_paths+=("$ANDROID_HOME/platform-tools")
    [ -d "$ANDROID_HOME/cmdline-tools/latest/bin" ] && new_paths+=("$ANDROID_HOME/cmdline-tools/latest/bin")
    if [ ${#new_paths[@]} -gt 0 ]; then
        export PATH="$(IFS=:; echo "${new_paths[*]}"):$PATH"
    fi
    
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

    # 设置环境变量
    setup_android_env

    check_or_create_keystore

    case $BUILD_TYPE in
        "debug")
            bun run tauri android build --debug --target aarch64 || {
                error "Android debug 构建失败"
                exit 1
            }
            copy_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/debug/*.apk" \
                "camera-ftp-companion-debug.apk" \
                "Debug APK"
            ;;
        "release")
            bun run tauri android build --target aarch64 || {
                error "Android release 构建失败"
                exit 1
            }
            copy_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk" \
                "camera-ftp-companion.apk" \
                "Release APK"
            ;;
    esac
}

# ============================================
# 帮助
# ============================================

show_standalone_help() {
    cat << EOF
用法: ./build-android.sh [选项]

选项:
  --release         构建 Release 版本 (默认)
  --debug           构建 Debug 版本
  --check           仅检查环境，不编译
  --help, -h        显示此帮助信息

示例:
  ./build-android.sh              # 构建 Release 版本
  ./build-android.sh --debug      # 构建 Debug 版本
  ./build-android.sh --check      # 检查编译环境

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
    # 使用通用参数解析
    local result=0
    parse_build_args "$@" || result=$?

    if [ $result -eq 1 ]; then
        show_standalone_help
        exit 0
    elif [ $result -eq 2 ]; then
        error "未知参数"
        show_standalone_help
        exit 1
    fi

    if [ "$CHECK_ONLY" = true ]; then
        check_android_env
    else
        check_android_env && build_android "$BUILD_TYPE"
    fi
}

main "$@"
