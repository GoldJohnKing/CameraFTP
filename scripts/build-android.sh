#!/bin/bash
# Android 构建脚本 (由 build.sh 调用)
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
# 平台工具链检测
# ============================================

# 检测指定平台的完整工具链
# 返回: 0=完整, 1=不完整
detect_platform_toolchain() {
    local platform="$1"
    local -n java_ref="$2"
    local -n javac_ref="$3"
    local -n sdk_ref="$4"
    local -n ndk_ref="$5"
    local -n java_home_ref="$6"

    if [ "$platform" = "windows" ]; then
        # 检测 Windows Java
        if command -v java.exe &> /dev/null; then
            java_ref="java.exe"
        fi
        if command -v javac.exe &> /dev/null; then
            javac_ref="javac.exe"
        fi
        
        # 检测 Windows SDK
        sdk_ref=$(detect_windows_android_sdk || true)
        
        # 检测 Windows JAVA_HOME
        java_home_ref=$(detect_windows_java_home || true)
    else
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
    fi
    
    # 从 SDK 检测 NDK
    if [ -n "$sdk_ref" ]; then
        ndk_ref=$(detect_ndk_from_sdk "$sdk_ref" || true)
    fi
    
    # 检查是否完整（Java + Javac + SDK）
    if [ -n "$java_ref" ] && [ -n "$javac_ref" ] && [ -n "$sdk_ref" ]; then
        return 0
    fi
    return 1
}

# 声明全局变量存储两个平台的检测结果
declare -A WIN_TOOLS
declare -A WIN_PATHS
declare -A LINUX_TOOLS
declare -A LINUX_PATHS
WIN_COMPLETE=false
LINUX_COMPLETE=false

# 选择最佳平台工具链
select_platform_toolchain() {
    # 清空之前的检测结果
    WIN_TOOLS=()
    WIN_PATHS=()
    LINUX_TOOLS=()
    LINUX_PATHS=()
    WIN_COMPLETE=false
    LINUX_COMPLETE=false
    
    local win_java="" win_javac="" win_sdk="" win_ndk="" win_java_home=""
    local linux_java="" linux_javac="" linux_sdk="" linux_ndk="" linux_java_home=""
    
    # 检测 Windows 工具链
    if detect_platform_toolchain "windows" win_java win_javac win_sdk win_ndk win_java_home; then
        WIN_COMPLETE=true
    fi
    
    # 保存 Windows 检测结果到全局变量
    WIN_TOOLS[java]="$win_java"
    WIN_TOOLS[javac]="$win_javac"
    WIN_PATHS[android_sdk]="$win_sdk"
    WIN_PATHS[android_ndk]="$win_ndk"
    WIN_PATHS[java_home]="$win_java_home"

    # 调试：检测失败时输出详细信息
    if [ "${DEBUG:-false}" = true ] && [ "$WIN_COMPLETE" = false ]; then
        warn "Windows SDK 检测详情："
        warn "  WIN_USER: ${WIN_USER:-未设置}"
        warn "  USER: $USER"
        warn "  ANDROID_HOME: ${ANDROID_HOME:-未设置}"
        warn "  尝试路径: /mnt/c/Users/${WIN_USER:-$USER}/AppData/Local/Android/Sdk"
        if [ -n "${ANDROID_HOME:-}" ]; then
            warn "  ANDROID_HOME 存在: $([ -d "$ANDROID_HOME" ] && echo '是' || echo '否')"
            warn "  是否为 Windows 路径: $([[ "$ANDROID_HOME" == /mnt/* ]] && echo '是' || echo '否')"
        fi
    fi

    # 检测 Linux 工具链
    if detect_platform_toolchain "linux" linux_java linux_javac linux_sdk linux_ndk linux_java_home; then
        LINUX_COMPLETE=true
    fi
    
    # 保存 Linux 检测结果到全局变量
    LINUX_TOOLS[java]="$linux_java"
    LINUX_TOOLS[javac]="$linux_javac"
    LINUX_PATHS[android_sdk]="$linux_sdk"
    LINUX_PATHS[android_ndk]="$linux_ndk"
    LINUX_PATHS[java_home]="$linux_java_home"
    
    # 决定使用哪个平台
    local selected_platform=""
    
    if [ "$WIN_COMPLETE" = true ] && [ "$LINUX_COMPLETE" = true ]; then
        # 两个平台都完整，检查用户是否指定了偏好
        if [ -n "${ANDROID_HOME:-}" ]; then
            # 用户设置了 ANDROID_HOME，跟随它
            if [[ "$ANDROID_HOME" == /mnt/* ]]; then
                selected_platform="windows"
                info "检测到 ANDROID_HOME 指向 Windows 路径，选择 Windows 平台"
            else
                selected_platform="linux"
                info "检测到 ANDROID_HOME 指向 Linux 路径，选择 Linux 平台"
            fi
        elif [ -n "${JAVA_HOME:-}" ]; then
            # 用户设置了 JAVA_HOME，跟随它
            if [[ "$JAVA_HOME" == /mnt/* ]]; then
                selected_platform="windows"
                info "检测到 JAVA_HOME 指向 Windows 路径，选择 Windows 平台"
            else
                selected_platform="linux"
                info "检测到 JAVA_HOME 指向 Linux 路径，选择 Linux 平台"
            fi
        else
            # 默认选择 Windows（兼容性更好）
            selected_platform="windows"
            info "Windows 和 Linux 平台都有完整工具链，默认选择 Windows"
            info "如需指定平台，请设置 ANDROID_HOME 或 JAVA_HOME 环境变量"
        fi
    elif [ "$LINUX_COMPLETE" = true ]; then
        selected_platform="linux"
    elif [ "$WIN_COMPLETE" = true ]; then
        selected_platform="windows"
    else
        # 没有完整工具链，报告错误
        error "未找到完整的 Android 编译工具链"
        echo ""
        return 1
    fi
    
    # 设置选定的工具链
    if [ "$selected_platform" = "windows" ]; then
        SELECTED_TOOLS[java]="$win_java"
        SELECTED_TOOLS[javac]="$win_javac"
        SELECTED_PATHS[android_sdk]="$win_sdk"
        SELECTED_PATHS[android_ndk]="$win_ndk"
        SELECTED_PATHS[java_home]="$win_java_home"
        SELECTED_PATHS[platform]="windows"
    else
        SELECTED_TOOLS[java]="$linux_java"
        SELECTED_TOOLS[javac]="$linux_javac"
        SELECTED_PATHS[android_sdk]="$linux_sdk"
        SELECTED_PATHS[android_ndk]="$linux_ndk"
        SELECTED_PATHS[java_home]="$linux_java_home"
        SELECTED_PATHS[platform]="linux"
    fi
    
    return 0
}

# 显示两个平台的工具链检测情况
show_all_platform_status() {
    info "[Windows 平台]"
    info "  Java:   ${WIN_TOOLS[java]:-未找到}"
    info "  Javac:  ${WIN_TOOLS[javac]:-未找到}"
    info "  SDK:    ${WIN_PATHS[android_sdk]:-未找到}"
    info "  NDK:    ${WIN_PATHS[android_ndk]:-未找到}"
    info "  JAVA_HOME: ${WIN_PATHS[java_home]:-未找到}"
    info "[Linux 平台]"
    info "  Java:   ${LINUX_TOOLS[java]:-未找到}"
    info "  Javac:  ${LINUX_TOOLS[javac]:-未找到}"
    info "  SDK:    ${LINUX_PATHS[android_sdk]:-未找到}"
    info "  NDK:    ${LINUX_PATHS[android_ndk]:-未找到}"
    info "  JAVA_HOME: ${LINUX_PATHS[java_home]:-未找到}"
}

# 检查 keytool
check_keytool() {
    local platform="${SELECTED_PATHS[platform]}"
    if [ "$platform" = "windows" ]; then
        if command -v keytool.exe &> /dev/null; then
            SELECTED_TOOLS[keytool]="keytool.exe"
            info "Keytool: keytool.exe"
            return 0
        fi
    else
        if command -v keytool &> /dev/null; then
            SELECTED_TOOLS[keytool]="keytool"
            info "Keytool: keytool"
            return 0
        fi
    fi
    warn "keytool 未找到，签名功能不可用"
    return 1
}

check_android_env() {
    info "正在检查 Android 编译环境..."

    # 选择平台工具链
    if ! select_platform_toolchain; then
        return 1
    fi

    # 如果是 --check 模式，显示两个平台的详细情况
    if [ "${CHECK_ONLY:-false}" = true ]; then
        show_all_platform_status
    fi

    # 显示选定的工具链平台
    info "选定平台: ${SELECTED_PATHS[platform]}"

    # 检查 keytool
    check_keytool

    success "Android 编译环境检查通过"
    return 0
}

# 环境变量设置
setup_android_env() {
    # 设置 JAVA_HOME
    if [ -n "${SELECTED_PATHS[java_home]:-}" ]; then
        export JAVA_HOME="${SELECTED_PATHS[java_home]}"
    else
        # 回退到默认
        export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk-amd64}"
    fi
    
    # 设置 Android SDK
    if [ -n "${SELECTED_PATHS[android_sdk]:-}" ]; then
        export ANDROID_HOME="${SELECTED_PATHS[android_sdk]}"
        export ANDROID_SDK_ROOT="${SELECTED_PATHS[android_sdk]}"
    else
        export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
        export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
    fi
    
    # 设置 NDK_HOME
    if [ -n "${SELECTED_PATHS[android_ndk]:-}" ]; then
        export NDK_HOME="${SELECTED_PATHS[android_ndk]}"
    elif [ -z "${NDK_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
        local ndk_version
        for ndk_version in "$ANDROID_HOME/ndk"/*; do
            if [ -d "$ndk_version" ]; then
                export NDK_HOME="$ndk_version"
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
        export PATH="$(IFS=:; echo "${new_paths[*]}"):$PATH"
    fi
    
    # Gradle 优化
    export GRADLE_OPTS="-Dorg.gradle.parallel=true -Dorg.gradle.configureondemand=true"
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

平台选择:
  当 Windows 和 Linux 都有完整工具链时，默认选择 Linux。
  如需指定平台，请设置环境变量：
    export ANDROID_HOME=/path/to/sdk    # 跟随 SDK 平台
    export JAVA_HOME=/path/to/jdk       # 跟随 Java 平台

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
        check_android_env
    else
        check_android_env && build_android "$BUILD_TYPE"
    fi
}

main "$@"
