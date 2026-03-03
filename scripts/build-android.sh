#!/bin/bash
# Android 构建脚本 (由 build.sh 调用) - 仅支持 Linux 平台
set -e

# 引入公共函数库
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

# 切换到项目根目录
cd "$SCRIPT_DIR/.."

# 环境检查与设置
declare -A SELECTED_TOOLS
declare -A SELECTED_PATHS

# ============================================
# Linux 工具链检测
# ============================================

# 检测 Linux 平台的完整工具链
# 返回: 0=完整, 1=不完整
detect_linux_toolchain() {
    local -n java_ref="$1"
    local -n javac_ref="$2"
    local -n sdk_ref="$3"
    local -n ndk_ref="$4"
    local -n java_home_ref="$5"

    # 检测 Linux Java
    if command -v java &> /dev/null; then
        java_ref="java"
    fi
    if command -v javac &> /dev/null; then
        javac_ref="javac"
    fi
    
    # 检测 Linux SDK
    sdk_ref=$(detect_linux_android_sdk || true)
    
    # 检测 Linux JAVA_HOME
    java_home_ref=$(detect_linux_java_home || true)
    
    # 验证 SDK 的可执行文件
    if [ -n "$sdk_ref" ]; then
        # 检查 platform-tools/adb
        if [ ! -x "$sdk_ref/platform-tools/adb" ]; then
            sdk_ref=""  # 如果缺少核心工具，清空 SDK 路径
        fi
    fi
    
    # 从 SDK 检测 NDK
    if [ -n "$sdk_ref" ]; then
        ndk_ref=$(detect_ndk_from_sdk "$sdk_ref" || true)
    fi
    
    # 检查是否完整（Java + Javac + SDK + 可执行文件）
    if [ -n "$java_ref" ] && [ -n "$javac_ref" ] && [ -n "$sdk_ref" ]; then
        return 0
    fi
    return 1
}

# 检查工具链
check_toolchain() {
    info "正在检查 Android 编译环境..."

    # ============================================
    # 步骤1：检查用户是否已配置完整的环境变量
    # ============================================
    local user_java_home="${JAVA_HOME:-}"
    local user_android_home="${ANDROID_HOME:-}"
    local user_config_valid=true
    
    # 验证 JAVA_HOME
    if [ -z "$user_java_home" ]; then
        user_config_valid=false
    elif [ ! -d "$user_java_home" ]; then
        warn "JAVA_HOME 已设置但目录不存在: $user_java_home"
        user_config_valid=false
    fi
    
    # 验证 ANDROID_HOME
    if [ -z "$user_android_home" ]; then
        user_config_valid=false
    elif [ ! -d "$user_android_home" ]; then
        warn "ANDROID_HOME 已设置但目录不存在: $user_android_home"
        user_config_valid=false
    fi
    
    # ============================================
    # 步骤2：如果用户配置完整，直接使用
    # ============================================
    if [ "$user_config_valid" = true ]; then
        info "检测到用户已配置环境变量，跳过自动检测"
        info "  JAVA_HOME: $user_java_home"
        info "  ANDROID_HOME: $user_android_home"
        
        # 标记为用户配置模式
        SELECTED_PATHS[user_configured]="true"
        
        # 检查 keytool
        check_keytool
        
        success "Android 编译环境检查通过"
        return 0
    fi
    
    # ============================================
    # 步骤3：用户未配置完整，执行自动检测
    # ============================================
    info "用户环境变量未完整配置，执行自动检测..."
    
    local java="" javac="" sdk="" ndk="" java_home=""
    
    if ! detect_linux_toolchain java javac sdk ndk java_home; then
        error "未找到完整的 Android 编译工具链"
        error "请安装:"
        error "  1. JDK 17 或 21 (apt install openjdk-21-jdk)"
        error "  2. Android SDK (https://developer.android.com/studio#command-tools)"
        error "或手动设置环境变量:"
        error "  export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64"
        error "  export ANDROID_HOME=$HOME/Android/Sdk"
        return 1
    fi

    # 保存检测结果
    SELECTED_TOOLS[java]="$java"
    SELECTED_TOOLS[javac]="$javac"
    SELECTED_PATHS[android_sdk]="$sdk"
    SELECTED_PATHS[android_ndk]="$ndk"
    SELECTED_PATHS[java_home]="$java_home"

    # 如果是 --check 模式，显示详细情况
    if [ "${CHECK_ONLY:-false}" = true ]; then
        info "[检测到的工具链]"
        info "  Java:   ${java:-未找到}"
        info "  Javac:  ${javac:-未找到}"
        info "  SDK:    ${sdk:-未找到}"
        info "  NDK:    ${ndk:-未找到}"
        info "  JAVA_HOME: ${java_home:-未找到}"
    fi

    # 检查 keytool
    check_keytool

    success "Android 编译环境检查通过"
    return 0
}

# 检查 keytool
check_keytool() {
    if command -v keytool &> /dev/null; then
        SELECTED_TOOLS[keytool]="keytool"
        info "Keytool: keytool"
        return 0
    fi
    warn "keytool 未找到，签名功能不可用"
    return 1
}

# 环境变量设置
setup_android_env() {
    local user_configured="${SELECTED_PATHS[user_configured]:-false}"
    
    # ============================================
    # 用户配置模式：直接使用用户设置的环境变量
    # ============================================
    if [ "$user_configured" = true ]; then
        # 验证用户配置仍然有效
        if [ ! -d "${JAVA_HOME:-}" ]; then
            error "JAVA_HOME 目录不存在: ${JAVA_HOME:-未设置}"
            return 1
        fi
        if [ ! -d "${ANDROID_HOME:-}" ]; then
            error "ANDROID_HOME 目录不存在: ${ANDROID_HOME:-未设置}"
            return 1
        fi
        
        # 设置 NDK_HOME（如果未设置）
        if [ -z "${NDK_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
            local ndk_version
            for ndk_version in "$ANDROID_HOME/ndk"/*; do
                if [ -d "$ndk_version" ]; then
                    export NDK_HOME="$ndk_version"
                    info "NDK_HOME (自动检测): $NDK_HOME"
                    break
                fi
            done
        fi
        
        # 更新 PATH
        local new_paths=()
        [ -d "$JAVA_HOME/bin" ] && new_paths+=("$JAVA_HOME/bin")
        [ -d "$ANDROID_HOME/platform-tools" ] && new_paths+=("$ANDROID_HOME/platform-tools")
        [ -d "$ANDROID_HOME/cmdline-tools/latest/bin" ] && new_paths+=("$ANDROID_HOME/cmdline-tools/latest/bin")
        if [ ${#new_paths[@]} -gt 0 ]; then
            # 使用 printf 安全处理包含空格的路径
            local path_prefix
            path_prefix=$(printf "%s:" "${new_paths[@]}")
            path_prefix="${path_prefix%:}"  # 移除末尾的冒号
            export PATH="$path_prefix:$PATH"
            info "已更新 PATH: ${new_paths[*]}"
        fi
        
        # Gradle 优化
        export GRADLE_OPTS="-Dorg.gradle.parallel=true -Dorg.gradle.configureondemand=true"
        
        info "使用用户配置的环境变量:"
        info "  JAVA_HOME=$JAVA_HOME"
        info "  ANDROID_HOME=$ANDROID_HOME"
        [ -n "${NDK_HOME:-}" ] && info "  NDK_HOME=$NDK_HOME"
        
        return 0
    fi
    
    # ============================================
    # 自动检测模式：使用检测到的路径
    # ============================================
    
    # 设置 JAVA_HOME
    if [ -n "${SELECTED_PATHS[java_home]:-}" ]; then
        export JAVA_HOME="${SELECTED_PATHS[java_home]}"
        info "JAVA_HOME (自动检测): $JAVA_HOME"
    else
        # 使用默认路径
        export JAVA_HOME="/usr/lib/jvm/java-21-openjdk-amd64"
        warn "JAVA_HOME 未检测到，使用默认值: $JAVA_HOME"
        warn "如果编译失败，请手动设置 JAVA_HOME 环境变量"
    fi
    
    # 验证 JAVA_HOME 有效性
    if [ ! -d "${JAVA_HOME:-}" ]; then
        error "JAVA_HOME 目录不存在: ${JAVA_HOME:-未设置}"
        error "请安装 JDK 17 或 21，并设置 JAVA_HOME 环境变量"
        return 1
    fi
    
    # 设置 Android SDK
    if [ -n "${SELECTED_PATHS[android_sdk]:-}" ]; then
        export ANDROID_HOME="${SELECTED_PATHS[android_sdk]}"
        export ANDROID_SDK_ROOT="${SELECTED_PATHS[android_sdk]}"
        info "ANDROID_HOME (自动检测): $ANDROID_HOME"
    else
        export ANDROID_HOME="$HOME/Android/Sdk"
        warn "ANDROID_HOME 未检测到，使用默认值: $ANDROID_HOME"
        warn "如果编译失败，请手动设置 ANDROID_HOME 环境变量"
    fi
    
    export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
    
    # 验证 ANDROID_HOME 有效性
    if [ ! -d "${ANDROID_HOME:-}" ]; then
        error "ANDROID_HOME 目录不存在: ${ANDROID_HOME:-未设置}"
        error "请安装 Android SDK，并设置 ANDROID_HOME 环境变量"
        return 1
    fi
    
    # 设置 NDK_HOME
    if [ -n "${SELECTED_PATHS[android_ndk]:-}" ]; then
        export NDK_HOME="${SELECTED_PATHS[android_ndk]}"
        info "NDK_HOME (自动检测): $NDK_HOME"
    elif [ -z "${NDK_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
        local ndk_version
        for ndk_version in "$ANDROID_HOME/ndk"/*; do
            if [ -d "$ndk_version" ]; then
                export NDK_HOME="$ndk_version"
                info "NDK_HOME (自动检测): $NDK_HOME"
                break
            fi
        done
    fi
    
    # 更新 PATH
    local new_paths=()
    [ -d "$JAVA_HOME/bin" ] && new_paths+=("$JAVA_HOME/bin")
    [ -d "$ANDROID_HOME/platform-tools" ] && new_paths+=("$ANDROID_HOME/platform-tools")
    [ -d "$ANDROID_HOME/cmdline-tools/latest/bin" ] && new_paths+=("$ANDROID_HOME/cmdline-tools/latest/bin")
    if [ ${#new_paths[@]} -gt 0 ]; then
        local path_prefix
        path_prefix=$(printf "%s:" "${new_paths[@]}")
        path_prefix="${path_prefix%:}"  # 移除末尾的冒号
        export PATH="$path_prefix:$PATH"
        info "已更新 PATH: ${new_paths[*]}"
    fi
    
    # Gradle 优化
    export GRADLE_OPTS="-Dorg.gradle.parallel=true -Dorg.gradle.configureondemand=true"
    
    info "环境变量已设置:"
    info "  JAVA_HOME=$JAVA_HOME"
    info "  ANDROID_HOME=$ANDROID_HOME"
    info "  ANDROID_SDK_ROOT=$ANDROID_SDK_ROOT"
    [ -n "${NDK_HOME:-}" ] && info "  NDK_HOME=$NDK_HOME"
    
    return 0
}

# 签名密钥
check_or_create_keystore() {
    local keystore_path="src-tauri/gen/android/keystore.properties"
    local keystore_file="camera-ftp-companion.keystore"
    
    if [ ! -f "$keystore_path" ]; then
        warn "签名配置不存在，创建新的签名密钥..."
        
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
        
        mv "$keystore_file" "src-tauri/gen/android/$keystore_file"
        
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

# 构建
build_android() {
    local BUILD_TYPE="${1:-release}"

    info "开始构建 Android 应用 ($BUILD_TYPE) - 仅 arm64-v8a 架构"

    if ! setup_android_env; then
        error "环境变量设置失败，无法继续构建"
        exit 1
    fi
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

# 帮助信息
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

环境变量:
  如果设置了有效的 JAVA_HOME 和 ANDROID_HOME，将直接使用而不进行自动检测

输出位置:
  Release: out/camera-ftp-companion.apk
  Debug:   out/camera-ftp-companion-debug.apk

包名: com.gjk.camera-ftp-companion
架构: arm64-v8a (64位 ARM处理器)

注意: 推荐使用 ./build.sh android 进行构建，会自动生成类型绑定
EOF
}

# 主函数
main() {
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
        check_toolchain
    else
        check_toolchain && build_android "$BUILD_TYPE"
    fi
}

main "$@"
