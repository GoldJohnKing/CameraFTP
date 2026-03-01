#!/bin/bash
# Windows Build Script for Camera FTP Companion
# 图传伴侣 Windows 版本编译脚本

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

# 拷贝编译产物到 release 目录
# 用法: copy_to_release <源文件路径> <目标文件名> <构建类型>
copy_to_release() {
    local src_path="$1"
    local dest_name="$2"
    local build_type="$3"
    
    mkdir -p release
    cp "$src_path" "release/$dest_name"
    
    success "Windows $build_type 构建完成"
    info "输出位置: release/$dest_name"
}

# 检查环境
check_environment() {
    info "检查编译环境..."
    
    # 检查 bun
    if ! command -v bun &> /dev/null; then
        error "Bun 未安装"
        echo "请安装 Bun: https://bun.sh"
        exit 1
    fi
    info "Bun 版本: $(bun --version)"
    
    # 检查 Windows cargo
    USERPROFILE=$(wslpath "$(cmd.exe /c "echo %USERPROFILE%" 2>/dev/null | tr -d '\r')")
    CARGO_EXE="$USERPROFILE/.cargo/bin/cargo.exe"
    
    if [ ! -f "$CARGO_EXE" ]; then
        error "cargo.exe 未找到: $CARGO_EXE"
        echo "请确保 Rust 已在 Windows 上安装: https://rustup.rs"
        exit 1
    fi
    info "Cargo 路径: $CARGO_EXE"
    
    success "环境检查完成"
}

# 构建 Windows 应用
build_windows() {
    local BUILD_TYPE="${1:-release}"
    
    info "开始构建 Windows 应用 ($BUILD_TYPE)..."
    
    # 获取 cargo 路径
    USERPROFILE=$(wslpath "$(cmd.exe /c "echo %USERPROFILE%" 2>/dev/null | tr -d '\r')")
    CARGO_EXE="$USERPROFILE/.cargo/bin/cargo.exe"
    
    # 安装前端依赖
    info "安装前端依赖..."
    bun install --no-cache
    
    # 构建前端
    info "构建前端..."
    bun run build
    
    # 终止运行中的进程
    info "终止运行中的进程..."
    if taskkill.exe /F /IM camera-ftp-companion.exe >/dev/null 2>&1; then
        info "已终止 camera-ftp-companion.exe"
    else
        info "没有运行中的实例"
    fi
    
    # 构建 Windows 可执行文件
    info "构建 Windows 可执行文件 ($BUILD_TYPE)..."
    cd src-tauri
    
    local OUTPUT_NAME="camera-ftp-companion.exe"
    local DEST_NAME="$OUTPUT_NAME"
    
    if [ "$BUILD_TYPE" = "debug" ]; then
        "$CARGO_EXE" build --target x86_64-pc-windows-msvc
        local SRC_PATH="target/x86_64-pc-windows-msvc/debug/$OUTPUT_NAME"
        DEST_NAME="camera-ftp-companion-debug.exe"
    else
        "$CARGO_EXE" build --release --target x86_64-pc-windows-msvc
        local SRC_PATH="target/x86_64-pc-windows-msvc/release/$OUTPUT_NAME"
    fi
    
    cd ..
    copy_to_release "src-tauri/$SRC_PATH" "$DEST_NAME" "$BUILD_TYPE"
}

# 主函数
main() {
    echo ""
    echo "=========================================="
    echo "  图传伴侣 (Camera FTP Companion)"
    echo "  Windows 版本编译脚本"
    echo "  目标: x86_64-pc-windows-msvc"
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
            build_windows "debug"
            ;;
        "release"|"")
            check_environment
            build_windows "release"
            ;;
        "help"|"-h"|"--help")
            echo "用法: $0 [命令]"
            echo ""
            echo "命令:"
            echo "  check, env    检查编译环境"
            echo "  (无参数)      构建 Release 可执行文件 (默认)"
            echo "  release       构建 Release 可执行文件"
            echo "  debug         构建 Debug 可执行文件"
            echo "  help          显示此帮助信息"
            echo ""
            echo "示例:"
            echo "  $0 check      # 检查环境"
            echo "  $0 release    # 构建 Release 版本"
            echo "  $0 debug      # 构建 Debug 版本"
            echo ""
            echo "输出位置:"
            echo "  Release: release/camera-ftp-companion.exe"
            echo "  Debug:   release/camera-ftp-companion-debug.exe"
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
