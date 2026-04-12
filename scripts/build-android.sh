#!/bin/bash
# Android 构建脚本
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

cd "$SCRIPT_DIR/.."

DEPLOY_PATH="${DEPLOY_PATH:-/mnt/ext1/shared-files/nginx}"

declare -A SELECTED_TOOLS
declare -A SELECTED_PATHS

detect_linux_toolchain() {
    local -n java_ref="$1"
    local -n javac_ref="$2"
    local -n sdk_ref="$3"
    local -n ndk_ref="$4"
    local -n java_home_ref="$5"

    if command -v java &> /dev/null; then
        java_ref="java"
    fi
    if command -v javac &> /dev/null; then
        javac_ref="javac"
    fi
    
    sdk_ref=$(detect_linux_android_sdk || true)
    
    java_home_ref=$(detect_linux_java_home || true)
    
    if [ -n "$sdk_ref" ]; then
        if [ ! -x "$sdk_ref/platform-tools/adb" ]; then
            sdk_ref=""  # 如果缺少核心工具，清空 SDK 路径
        fi
    fi
    
    # 从 SDK 检测 NDK
    if [ -n "$sdk_ref" ]; then
        ndk_ref=$(detect_ndk_from_sdk "$sdk_ref" || true)
    fi
    
    if [ -n "$java_ref" ] && [ -n "$javac_ref" ] && [ -n "$sdk_ref" ]; then
        return 0
    fi
    return 1
}

# 检查工具链
check_toolchain() {
    debug_info "正在检查 Android 编译环境..."

    local user_java_home="${JAVA_HOME:-}"
    local user_android_home="${ANDROID_HOME:-}"
    local user_config_valid=true
    
    if [ -z "$user_java_home" ]; then
        user_config_valid=false
    elif [ ! -d "$user_java_home" ]; then
        warn "JAVA_HOME 已设置但目录不存在: $user_java_home"
        user_config_valid=false
    fi
    
    if [ -z "$user_android_home" ]; then
        user_config_valid=false
    elif [ ! -d "$user_android_home" ]; then
        warn "ANDROID_HOME 已设置但目录不存在: $user_android_home"
        user_config_valid=false
    fi
    
    if [ "$user_config_valid" = true ]; then
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "检测到用户已配置环境变量，跳过自动检测"
            info "  JAVA_HOME: $user_java_home"
            info "  ANDROID_HOME: $user_android_home"
        fi
        
        SELECTED_PATHS[user_configured]="true"
        
        check_keytool || return 1
        
        success "Android 编译环境检查通过"
        return 0
    fi
    
    debug_info "用户环境变量未完整配置，执行自动检测..."
    
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

    SELECTED_TOOLS[java]="$java"
    SELECTED_TOOLS[javac]="$javac"
    SELECTED_PATHS[android_sdk]="$sdk"
    SELECTED_PATHS[android_ndk]="$ndk"
    SELECTED_PATHS[java_home]="$java_home"

    if [ "${CHECK_ONLY:-false}" = true ]; then
        info "[检测到的工具链]"
        info "  Java:   ${java:-未找到}"
        info "  Javac:  ${javac:-未找到}"
        info "  SDK:    ${sdk:-未找到}"
        info "  NDK:    ${ndk:-未找到}"
        info "  JAVA_HOME: ${java_home:-未找到}"
    fi

    check_keytool || return 1

    success "Android 编译环境检查通过"
}

check_keytool() {
    if command -v keytool &> /dev/null; then
        SELECTED_TOOLS[keytool]="keytool"
        return 0
    fi
    warn "keytool 未找到，签名功能不可用"
    return 1
}

# 更新 PATH
_update_android_path() {
    local new_paths=()
    [ -d "$JAVA_HOME/bin" ] && new_paths+=("$JAVA_HOME/bin")
    [ -d "$ANDROID_HOME/platform-tools" ] && new_paths+=("$ANDROID_HOME/platform-tools")
    [ -d "$ANDROID_HOME/cmdline-tools/latest/bin" ] && new_paths+=("$ANDROID_HOME/cmdline-tools/latest/bin")

    if [ ${#new_paths[@]} -gt 0 ]; then
        local path_prefix
        path_prefix=$(printf "%s:" "${new_paths[@]}")
        path_prefix="${path_prefix%:}"
        export PATH="$path_prefix:$PATH"
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "已更新 PATH: ${new_paths[*]}"
        fi
    fi
}

# 设置 NDK_HOME
_setup_ndk_home() {
    local source_label="$1"

    if [ -z "${NDK_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
        local ndk_version
        for ndk_version in "$ANDROID_HOME/ndk"/*; do
            if [ -d "$ndk_version" ]; then
                export NDK_HOME="$ndk_version"
                if [ "${CHECK_ONLY:-false}" = true ]; then
                    info "NDK_HOME ($source_label): $NDK_HOME"
                fi
                break
            fi
        done
    fi
}

# 环境变量设置
setup_android_env() {
    local user_configured="${SELECTED_PATHS[user_configured]:-false}"
    local ndk_source=""

    if [ "$user_configured" = true ]; then
        if [ ! -d "${JAVA_HOME:-}" ]; then
            error "JAVA_HOME 目录不存在: ${JAVA_HOME:-未设置}"
            return 1
        fi
        if [ ! -d "${ANDROID_HOME:-}" ]; then
            error "ANDROID_HOME 目录不存在: ${ANDROID_HOME:-未设置}"
            return 1
        fi
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "使用用户配置的环境变量"
        fi
        ndk_source="自动检测"
    else
        if [ -n "${SELECTED_PATHS[java_home]:-}" ]; then
            export JAVA_HOME="${SELECTED_PATHS[java_home]}"
            if [ "${CHECK_ONLY:-false}" = true ]; then
                info "JAVA_HOME (自动检测): $JAVA_HOME"
            fi
        else
            export JAVA_HOME="/usr/lib/jvm/java-21-openjdk-amd64"
            warn "JAVA_HOME 未检测到，使用默认值: $JAVA_HOME"
        fi

        if [ -n "${SELECTED_PATHS[android_sdk]:-}" ]; then
            export ANDROID_HOME="${SELECTED_PATHS[android_sdk]}"
            export ANDROID_SDK_ROOT="${SELECTED_PATHS[android_sdk]}"
            if [ "${CHECK_ONLY:-false}" = true ]; then
                info "ANDROID_HOME (自动检测): $ANDROID_HOME"
            fi
        else
            export ANDROID_HOME="$HOME/Android/Sdk"
            warn "ANDROID_HOME 未检测到，使用默认值: $ANDROID_HOME"
        fi
        ndk_source="自动检测"
    fi

    if [ ! -d "${JAVA_HOME:-}" ]; then
        error "JAVA_HOME 目录不存在: ${JAVA_HOME:-未设置}"
        error "请安装 JDK 17 或 21，并设置 JAVA_HOME 环境变量"
        return 1
    fi

    export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"

    if [ ! -d "${ANDROID_HOME:-}" ]; then
        error "ANDROID_HOME 目录不存在: ${ANDROID_HOME:-未设置}"
        error "请安装 Android SDK，并设置 ANDROID_HOME 环境变量"
        return 1
    fi

    if [ "$user_configured" != true ] && [ -n "${SELECTED_PATHS[android_ndk]:-}" ]; then
        export NDK_HOME="${SELECTED_PATHS[android_ndk]}"
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "NDK_HOME (自动检测): $NDK_HOME"
        fi
    else
        _setup_ndk_home "$ndk_source"
    fi

    _update_android_path
    export GRADLE_OPTS="-Dorg.gradle.parallel=true"

    return 0
}

# 签名密钥
check_or_create_keystore() {
    local keystore_path="src-tauri/gen/android/keystore.properties"
    local keystore_file="cameraftp.keystore"

    local key_alias="${KEYSTORE_ALIAS:-cameraftp}"
    local key_store_pass="${KEYSTORE_PASSWORD:-cameraftp123}"
    local key_pass="${KEY_PASSWORD:-$key_store_pass}"
    local key_dname="${KEYSTORE_DNAME:-CN=CameraFTP, OU=Development, O=GJK, L=Unknown, ST=Unknown, C=CN}"

    if [ ! -f "$keystore_path" ]; then
        warn "签名配置不存在，创建新的签名密钥..."

        local keytool_cmd="${SELECTED_TOOLS[keytool]:-keytool}"
        $keytool_cmd -genkey -v \
            -keystore "$keystore_file" \
            -alias "$key_alias" \
            -keyalg RSA \
            -keysize 2048 \
            -validity 10000 \
            -dname "$key_dname" \
            -storepass "$key_store_pass" \
            -keypass "$key_pass"

        mv "$keystore_file" "src-tauri/gen/android/$keystore_file"

        cat > "$keystore_path" << EOF
storeFile=$keystore_file
storePassword=$key_store_pass
keyAlias=$key_alias
keyPassword=$key_pass
EOF

        success "签名密钥已创建: src-tauri/gen/android/$keystore_file"
        info "密钥信息已保存到: $keystore_path"

        if [ "$key_store_pass" = "cameraftp123" ]; then
            warn "使用的是默认密钥密码，建议设置 KEYSTORE_PASSWORD 环境变量"
        fi
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

    local VERSION
    VERSION=$(get_version)

    case $BUILD_TYPE in
        "debug")
            bun run tauri android build --debug --apk --target aarch64 || {
                error "Android debug 构建失败"
                exit 1
            }
            move_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/debug/*.apk" \
                "CameraFTP_v${VERSION}-debug.apk" \
                "Debug APK" \
                "${DEPLOY_PATH:+$DEPLOY_PATH/CameraFTP_v${VERSION}-debug.apk}"
            ;;
        "release")
            bun run tauri android build --apk --target aarch64 || {
                error "Android release 构建失败"
                exit 1
            }
            move_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk" \
                "CameraFTP_v${VERSION}.apk" \
                "Release APK" \
                "${DEPLOY_PATH:+$DEPLOY_PATH/CameraFTP_v${VERSION}.apk}"
            ;;
    esac
}

# 帮助信息
show_help() {
    echo "用法: ./build-android.sh [选项]"
    echo ""
    echo "选项:"
    echo "  --release   构建 Release 版本 (默认)"
    echo "  --debug     构建 Debug 版本"
    echo "  --check     仅检查环境，不编译"
    echo "  --help, -h  显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  ./build-android.sh          # 构建 Release 版本"
    echo "  ./build-android.sh --debug  # 构建 Debug 版本"
    echo "  ./build-android.sh --check  # 检查编译环境"
    echo ""
    local VERSION
    VERSION=$(get_version)
    echo "输出位置:"
    echo "  Release: out/CameraFTP_v${VERSION}.apk"
    echo "  Debug:   out/CameraFTP_v${VERSION}-debug.apk"
    echo ""
    echo "注意: 推荐使用 ./build.sh android 进行构建，会自动生成类型绑定"
}

# 主函数
main() {
    local result=0
    parse_build_args "$@" || result=$?

    if [ $result -eq 1 ]; then
        show_help
        exit 0
    elif [ $result -eq 2 ]; then
        error "未知参数"
        show_help
        exit 1
    fi

    if [ "$CHECK_ONLY" = true ]; then
        check_toolchain
    else
        check_toolchain && build_android "$BUILD_TYPE"
    fi
}

main "$@"
