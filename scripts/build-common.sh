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
)

declare -A TOOL_LINUX_CMDS=(
    [cargo]="cargo"
    [java]="java"
    [javac]="javac"
    [keytool]="keytool"
)

# 功能：获取工具命令 (优先 Windows .exe，回退到 Linux)
# 参数：
#   $1 - 工具名称 (如 cargo, java, javac, keytool)
# 返回：命令名称 (如 cargo.exe 或 cargo)，失败返回 1
# 示例：cmd=$(get_tool_cmd "cargo")
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

# 功能：获取工具所在平台
# 参数：
#   $1 - 工具名称 (如 cargo, java, javac, keytool)
# 返回：平台标识 ("windows" 或 "linux")，未找到返回 1
# 示例：platform=$(get_tool_platform "cargo")
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

# 功能：检查工具是否存在并打印版本信息
# 参数：
#   $1 - 工具名称 (如 cargo, java, javac, keytool)
#   $2 - 显示名称 (可选，默认为工具名称)
# 返回：0 表示已安装，1 表示未安装
# 示例：check_tool cargo "Cargo"
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
        info "$display_name: $version_info"
    else
        info "$display_name: 已安装"
    fi
    
    return 0
}

# 功能：检测 Windows Android SDK 路径
# 参数：无
# 返回：SDK 路径，未找到返回 1
# 示例：sdk=$(detect_windows_android_sdk)
detect_windows_android_sdk() {
    # 支持 WSL 中 Windows 用户名与 Linux 不同的情况
    local win_user="${WIN_USER:-$USER}"
    local sdk_paths=(
        "/mnt/c/Users/$win_user/AppData/Local/Android/Sdk"
        "/mnt/c/Users/$win_user/AppData/Local/android-sdk"
        "/mnt/c/Android/Sdk"
        "/mnt/c/android-sdk"
    )
    
    for path in "${sdk_paths[@]}"; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    
    return 1
}

# 功能：检测 Linux Android SDK 路径
# 参数：无
# 返回：SDK 路径，未找到返回 1
# 示例：sdk=$(detect_linux_android_sdk)
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

# 功能：从 SDK 路径检测 NDK 路径 (自动选择最新版本)
# 参数：
#   $1 - SDK 路径
# 返回：NDK 路径，未找到返回 1
# 示例：ndk=$(detect_ndk_from_sdk "$ANDROID_HOME")
detect_ndk_from_sdk() {
    if [ -z "$1" ]; then
        error "参数缺失：sdk_path"
        echo "提示：请提供 SDK 路径，如 detect_ndk_from_sdk /path/to/sdk"
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

# 功能：检测 Windows JAVA_HOME 路径 (优先 JDK 17/21)
# 参数：无
# 返回：JAVA_HOME 路径，未找到返回 1
# 示例：java_home=$(detect_windows_java_home)
detect_windows_java_home() {
    # 优先检查常见安装位置
    local java_dirs=(
        "/mnt/c/Program Files/Java"
        "/mnt/c/Program Files/Eclipse Adoptium"
        "/mnt/c/Program Files/Microsoft"
        "/mnt/c/Program Files/AdoptOpenJDK"
        "/mnt/c/Program Files/Zulu"
    )
    
    for dir in "${java_dirs[@]}"; do
        if [ -d "$dir" ]; then
            # 查找 JDK 目录 (优先 JDK 17 或 21，使用 glob 避免解析 ls)
            local jdk_item
            for jdk_item in "$dir"/jdk-*17* "$dir"/jdk-*21* "$dir"/jdk-17* "$dir"/jdk-21*; do
                if [ -d "$jdk_item" ]; then
                    echo "$jdk_item"
                    return 0
                fi
            done
            # 回退到任意 JDK
            for jdk_item in "$dir"/jdk* "$dir"/JDK*; do
                if [ -d "$jdk_item" ]; then
                    echo "$jdk_item"
                    return 0
                fi
            done
        fi
    done
    
    return 1
}

# 功能：检测 Linux JAVA_HOME 路径 (优先 JDK 17/21)
# 参数：无
# 返回：JAVA_HOME 路径，未找到返回 1
# 示例：java_home=$(detect_linux_java_home)
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
    
    # 检查固定路径 (无架构后缀)
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

# ============================================
# 通用参数解析
# ============================================

# 功能：解析构建参数 (供子脚本使用)
# 参数：
#   $@ - 命令行参数
# 返回：0=成功, 1=需要显示帮助, 2=未知参数
# 设置变量：BUILD_TYPE, CHECK_ONLY
# 示例：parse_build_args "$@"
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

# 功能：拷贝文件到 out 目录 (支持确定路径和 glob 模式)
# 参数：
#   $1 - 源文件路径或 glob 模式
#   $2 - 目标文件名
#   $3 - 构建产物描述
# 返回：0=成功, 1=未找到文件
# 示例：copy_to_out "target/release/*.exe" "app.exe" "Windows 可执行文件"
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

# 功能：检查 Bun 是否已安装
# 参数：无
# 返回：0=已安装, 1=未安装
# 示例：check_bun
check_bun() {
    if ! command -v bun &> /dev/null; then
        error "Bun 未安装"
        echo "提示：请访问 https://bun.sh 安装 Bun 运行时"
        return 1
    fi
    info "Bun: $(bun --version)"
    return 0
}

# 功能：生成 TypeScript 类型绑定 (使用选中的 cargo)
# 参数：无
# 返回：0=成功, 1=Cargo 未找到
# 示例：generate_ts_types
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

# 功能：清理所有构建缓存
# 参数：无
# 返回：0=成功
# 示例：clean_build_cache
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

# 功能：显示构建脚本使用帮助
# 参数：
#   $1 - 脚本名称 (可选，默认为 build.sh)
# 返回：无
# 示例：show_build_help "build.sh"
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
