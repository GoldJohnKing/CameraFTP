#!/bin/bash
# build-common.sh - 公共构建函数库

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 输出目录
OUTPUT_DIR="out"

# 读取版本号（从 Cargo.toml）
get_version() {
    local cargo_toml="${SCRIPT_DIR:-.}/../src-tauri/Cargo.toml"
    if [ -f "$cargo_toml" ]; then
        grep -E '^version\s*=\s*"' "$cargo_toml" | head -1 | sed -E 's/^version\s*=\s*"([^"]+)".*/\1/'
    else
        echo "1.0.0"
    fi
}

# 构建目标配置
readonly TARGET_WINDOWS_COLOR="36"
readonly TARGET_ANDROID_COLOR="35"
readonly TARGET_WINDOWS_NAME="Windows"
readonly TARGET_ANDROID_NAME="Android"
readonly TARGET_WINDOWS_TRIPLE="x86_64-pc-windows-msvc"
readonly TARGET_ANDROID_TRIPLE="aarch64-linux-android"

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

# 仅在 --check 模式下输出的信息
debug_info() {
    if [ "${CHECK_ONLY:-false}" = true ]; then
        info "$1"
    fi
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

# 获取工具命令
get_tool_cmd() {
    local tool_name="${1:-}"
    if [ -z "$tool_name" ]; then
        error "参数缺失：tool_name"
        return 1
    fi

    if command -v "${tool_name}.exe" &> /dev/null; then
        echo "${tool_name}.exe"
        return 0
    fi

    if command -v "$tool_name" &> /dev/null; then
        echo "$tool_name"
        return 0
    fi

    return 1
}

# 获取工具所在平台
get_tool_platform() {
    local tool_name="${1:-}"
    if [ -z "$tool_name" ]; then
        return 1
    fi

    if command -v "${tool_name}.exe" &> /dev/null; then
        echo "windows"
        return 0
    fi

    if command -v "$tool_name" &> /dev/null; then
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
    
    if [ "${CHECK_ONLY:-false}" = true ]; then
        platform=$(get_tool_platform "$tool_name")
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
    fi
    
    return 0
}

# 检测 Android SDK 路径
detect_linux_android_sdk() {
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

# 检测 JAVA_HOME
detect_linux_java_home() {
    if [ -n "$JAVA_HOME" ] && [ -d "$JAVA_HOME" ]; then
        echo "$JAVA_HOME"
        return 0
    fi
    
    # 检查常见路径
    local java_base="/usr/lib/jvm"
    local path
    
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
    
    # 尝试自动发现
    for path in "$java_base"/java-*-openjdk; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    
    return 1
}

move_to_out() {
    local src="$1"
    local dest_name="$2"
    local desc="$3"
    local extra_dest="${4:-}"

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
        mv "$src_file" "$OUTPUT_DIR/$dest_name"
        success "$desc 构建完成"
        info "输出位置: $OUTPUT_DIR/$dest_name"

        # 如果指定了额外目标路径，同时拷贝到该路径
        if [ -n "$extra_dest" ]; then
            if [ -d "$(dirname "$extra_dest")" ]; then
                cp "$OUTPUT_DIR/$dest_name" "$extra_dest"
                info "已拷贝到: $extra_dest"
            else
                warn "额外目标目录不存在，跳过拷贝: $(dirname "$extra_dest")"
            fi
        fi
    else
        error "未找到构建产物: $src"
        echo "提示：请检查构建是否成功，或路径模式是否正确"
        return 1
    fi
}

check_bun() {
    local bun_cmd
    
    if ! bun_cmd=$(get_tool_cmd "bun"); then
        error "Bun 未安装"
        echo "提示：请访问 https://bun.sh 安装 Bun 运行时"
        return 1
    fi
    
    if [ "${CHECK_ONLY:-false}" = true ]; then
        local platform
        platform=$(get_tool_platform "bun")
        info "Bun [$platform]: $($bun_cmd --version)"
    fi
    
    return 0
}

generate_ts_types() {
    task "生成 TypeScript 类型绑定..."

    mkdir -p dist

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

    local cargo_cmd
    if cargo_cmd=$(get_tool_cmd "cargo"); then
        info "运行 cargo clean..."
        cd src-tauri && $cargo_cmd clean 2>/dev/null || true && cd ..
    fi

    success "清理完成"
}

show_build_help() {
    local script_name="${1:-build.sh}"
    local VERSION
    VERSION=$(get_version)
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
  Windows: out/CameraFTP_v${VERSION}.exe
  Android: out/CameraFTP_v${VERSION}.apk
EOF
}
