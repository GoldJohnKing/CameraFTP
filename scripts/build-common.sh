#!/bin/bash
# build-common.sh - 公共构建函数库
# 所有构建脚本通过 source 引入此文件

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 输出目录
OUTPUT_DIR="out"

# 日志函数
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

task() {
    echo -e "${CYAN}[TASK]${NC} $1"
}

# ============================================
# 工具选择模块 (支持 Windows/Linux 混合构建)
# ============================================

# 工具注册表 - Windows 和 Linux 命令映射
declare -A TOOL_WINDOWS_CMDS=(
    [cargo]="cargo.exe"
    [java]="java.exe"
    [javac]="javac.exe"
    [keytool]="keytool.exe"
    [bun]="bun.exe"
)

declare -A TOOL_LINUX_CMDS=(
    [cargo]="cargo"
    [java]="java"
    [javac]="javac"
    [keytool]="keytool"
    [bun]="bun"
)

# 获取工具命令 (优先 Windows 版本)
get_tool_cmd() {
    if [ -z "$1" ]; then
        error "参数缺失：tool_name"
        echo "提示：请提供工具名称，如 get_tool_cmd cargo"
        return 1
    fi
    local tool_name="$1"
    local windows_cmd="${TOOL_WINDOWS_CMDS[$tool_name]}"
    local linux_cmd="${TOOL_LINUX_CMDS[$tool_name]}"
    
    # 优先尝试 Windows 版本
    if [ -n "$windows_cmd" ] && command -v "$windows_cmd" &> /dev/null; then
        echo "$windows_cmd"
        return 0
    fi
    
    # 回退到 Linux 版本
    if [ -n "$linux_cmd" ] && command -v "$linux_cmd" &> /dev/null; then
        echo "$linux_cmd"
        return 0
    fi
    
    # 工具未找到
    return 1
}

# 获取工具所在平台
get_tool_platform() {
    if [ -z "$1" ]; then
        error "参数缺失：tool_name"
        echo "提示：请提供工具名称，如 get_tool_platform cargo"
        return 1
    fi
    local tool_name="$1"
    local windows_cmd="${TOOL_WINDOWS_CMDS[$tool_name]}"
    local linux_cmd="${TOOL_LINUX_CMDS[$tool_name]}"
    
    if [ -n "$windows_cmd" ] && command -v "$windows_cmd" &> /dev/null; then
        echo "windows"
        return 0
    fi
    
    if [ -n "$linux_cmd" ] && command -v "$linux_cmd" &> /dev/null; then
        echo "linux"
        return 0
    fi
    
    return 1
}

# 检查工具是否存在
check_tool() {
    if [ -z "$1" ]; then
        error "参数缺失：tool_name"
        echo "提示：请提供工具名称，如 check_tool cargo \"Cargo\""
        return 1
    fi
    local tool_name="$1"
    local display_name="${2:-$tool_name}"
    local cmd
    local platform
    
    cmd=$(get_tool_cmd "$tool_name") || {
        error "$display_name 未安装"
        case "$tool_name" in
            cargo)
                echo "提示：请安装 Rust 工具链，访问 https://rustup.rs"
                ;;
            java|javac|keytool)
                echo "提示：请安装 JDK 17 或 21，推荐 Eclipse Adoptium 或 Microsoft Build of OpenJDK"
                ;;
            *)
                echo "提示：请安装 $display_name 后重试"
                ;;
        esac
        return 1
    }
    
    platform=$(get_tool_platform "$tool_name")
    
    # 获取版本信息
    local version_info=""
    case "$tool_name" in
        cargo|java|javac|keytool)
            version_info=$("$cmd" --version 2>/dev/null | head -1)
            ;;
    esac
    
    if [ -n "$version_info" ]; then
        info "$display_name [$platform]: $version_info"
    else
        info "$display_name [$platform]: 已安装"
    fi
    
    return 0
}

# 检测 Linux Android SDK 路径
detect_linux_android_sdk() {
    # 优先检查环境变量
    if [ -n "$ANDROID_HOME" ] && [ -d "$ANDROID_HOME" ]; then
        echo "$ANDROID_HOME"
        return 0
    fi
    
    if [ -n "$ANDROID_SDK_ROOT" ] && [ -d "$ANDROID_SDK_ROOT" ]; then
        echo "$ANDROID_SDK_ROOT"
        return 0
    fi
    
    # 检查常见路径
    local sdk_paths=(
        "$HOME/Android/Sdk"
        "$HOME/android-sdk"
        "/opt/android-sdk"
        "/usr/local/android-sdk"
    )
    
    for path in "${sdk_paths[@]}"; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    
    return 1
}

# 从 Android SDK 路径下检测 NDK
detect_ndk_from_sdk() {
    if [ -z "$1" ]; then
        error "参数缺失：sdk_path"
        echo "提示：请提供 Android SDK 路径，如 detect_ndk_from_sdk /path/to/sdk"
        return 1
    fi
    local sdk_path="$1"
    local ndk_dir="$sdk_path/ndk"

    if [ ! -d "$ndk_dir" ]; then
        return 1
    fi

    # 收集所有 NDK 版本，取最后一个（通常是最新的）
    local ndk_versions=()
    local v
    for v in "$ndk_dir"/*; do
        [ -d "$v" ] && ndk_versions+=("$v")
    done

    if [ ${#ndk_versions[@]} -gt 0 ]; then
        echo "${ndk_versions[-1]}"
        return 0
    fi

    return 1
}

# 检测 Linux JAVA_HOME
detect_linux_java_home() {
    # 优先检查环境变量
    if [ -n "$JAVA_HOME" ] && [ -d "$JAVA_HOME" ]; then
        echo "$JAVA_HOME"
        return 0
    fi
    
    # 检查常见路径 (使用 glob 模式避免硬编码架构)
    local java_base="/usr/lib/jvm"
    local path
    
    # 优先查找 Java 21 和 17
    for path in "$java_base"/java-21-openjdk-*; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    
    for path in "$java_base"/java-17-openjdk-*; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    
    local java_paths=(
        "$java_base/java-21-openjdk"
        "$java_base/java-17-openjdk"
        "$java_base/default-java"
    )
    
    for path in "${java_paths[@]}"; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    
    # 尝试自动发现 (使用 glob 避免解析 ls)
    for path in "$java_base"/java-*-openjdk; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    
    return 1
}

# 解析构建参数
parse_build_args() {
    BUILD_TYPE="release"
    CHECK_ONLY=false

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
                return 1
                ;;
            *)
                return 2
                ;;
        esac
    done
    return 0
}

copy_to_out() {
    local src="$1"
    local dest_name="$2"
    local desc="$3"
    
    mkdir -p "$OUTPUT_DIR"
    
    # 支持确定路径和 glob 模式
    local src_file=""
    for f in $src; do
        if [ -f "$f" ]; then
            src_file="$f"
            break
        fi
    done
    
    if [ -n "$src_file" ]; then
        cp "$src_file" "$OUTPUT_DIR/$dest_name"
        success "$desc 构建完成"
        info "输出位置: $OUTPUT_DIR/$dest_name"
    else
        error "未找到构建产物: $src"
        echo "提示：请检查构建是否成功，或路径模式是否正确"
        return 1
    fi
}

check_bun() {
    local bun_cmd
    local platform
    
    if ! bun_cmd=$(get_tool_cmd "bun"); then
        error "Bun 未安装"
        echo "提示：请访问 https://bun.sh 安装 Bun 运行时"
        return 1
    fi
    
    platform=$(get_tool_platform "bun")
    info "Bun [$platform]: $($bun_cmd --version)"
    return 0
}

generate_ts_types() {
    task "生成 TypeScript 类型绑定..."
    
    # 确保 dist 目录存在
    mkdir -p dist
    
    # 获取 cargo 命令
    local cargo_cmd
    cargo_cmd=$(get_tool_cmd "cargo")
    
    if [ -z "$cargo_cmd" ]; then
        error "Cargo 未找到，无法生成类型绑定"
        echo "提示：请安装 Rust 工具链，访问 https://rustup.rs"
        return 1
    fi
    
    cd src-tauri
    $cargo_cmd test --quiet 2>/dev/null || true
    cd ..
    
    success "TypeScript 类型绑定已生成到 src-tauri/bindings/"
}

clean_build_cache() {
    info "清理构建缓存..."

    local clean_list=(
        "src-tauri/target"
        "src-tauri/bindings"
        "dist"
        "$OUTPUT_DIR"
        "src-tauri/gen/android/app/build"
        "src-tauri/gen/android/.gradle"
    )

    for dir in "${clean_list[@]}"; do
        if [ -d "$dir" ]; then
            info "删除 $dir"
            rm -rf "$dir"
        fi
    done

    # 运行 cargo clean (使用工具选择层)
    local cargo_cmd
    if cargo_cmd=$(get_tool_cmd "cargo"); then
        info "运行 cargo clean..."
        cd src-tauri && $cargo_cmd clean 2>/dev/null || true && cd ..
    fi

    success "清理完成"
}

show_build_help() {
    local script_name="${1:-build.sh}"
    cat << EOF
用法: ./$script_name <targets...> [options]

目标 (可多个):
  windows           构建 Windows 可执行文件
  android           构建 Android APK

命令:
  gen-types         生成 TypeScript 类型绑定
  clean             清理所有构建缓存
  frontend          仅构建前端

选项:
  --release         构建 Release 版本 (默认)
  --debug           构建 Debug 版本
  --check           仅检查环境，不编译
  --serial, -s      串行编译 (默认并行)
  --help, -h        显示此帮助信息

示例:
  ./$script_name windows                      # 编译 Windows (release)
  ./$script_name windows --debug              # 编译 Windows (debug)
  ./$script_name windows --check              # 检查 Windows 编译环境
  ./$script_name windows android              # 并行编译 (release)
  ./$script_name windows android --debug      # 并行编译 (debug)
  ./$script_name windows android --check      # 并行检查环境
  ./$script_name windows android --serial     # 串行编译
  ./$script_name gen-types                    # 仅生成类型绑定

输出位置:
  Windows: out/camera-ftp-companion.exe
  Android: out/camera-ftp-companion.apk
EOF
}
