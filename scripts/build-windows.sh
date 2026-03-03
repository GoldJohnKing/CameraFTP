#!/bin/bash
# Windows 构建脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

cd "$SCRIPT_DIR/.."

check_windows_env() {
    if [ "${CHECK_ONLY:-false}" = true ]; then
        info "正在检查 Windows 编译环境..."
    fi
    success "Windows 编译环境检查通过"
    return 0
}

terminate_running_process() {
    info "正在检查运行中的进程..."
    if taskkill.exe /F /IM camera-ftp-companion.exe >/dev/null 2>&1; then
        info "已终止进程: camera-ftp-companion.exe"
    fi
}

build_windows() {
    local BUILD_TYPE="${1:-release}"

    info "开始构建 Windows 应用程序 ($BUILD_TYPE 模式)..."

    terminate_running_process

    local cargo_cmd
    cargo_cmd=$(get_tool_cmd "cargo")
    local target="$TARGET_WINDOWS_TRIPLE"

    cd src-tauri
    info "[Rust] 正在编译..."

    if [ "$BUILD_TYPE" = "debug" ]; then
        $cargo_cmd build --target "$target"
    else
        $cargo_cmd build --release --target "$target"
    fi

    cd ..

    # 复制输出
    local OUTPUT_NAME="camera-ftp-companion.exe"
    local DEST_NAME="$OUTPUT_NAME"
    local SRC_PATH

    if [ "$BUILD_TYPE" = "debug" ]; then
        SRC_PATH="src-tauri/target/x86_64-pc-windows-msvc/debug/$OUTPUT_NAME"
        DEST_NAME="camera-ftp-companion-debug.exe"
    else
        SRC_PATH="src-tauri/target/x86_64-pc-windows-msvc/release/$OUTPUT_NAME"
    fi

    copy_to_out "$SRC_PATH" "$DEST_NAME" "Windows $BUILD_TYPE"
}

# 显示帮助信息
show_help() {
    echo "用法: ./build-windows.sh [选项]"
    echo ""
    echo "选项:"
    echo "  --release         构建 Release 版本 (默认)"
    echo "  --debug           构建 Debug 版本"
    echo "  --check-toolchain 仅检查环境，不编译"
    echo "  --help, -h        显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  ./build-windows.sh              # 构建 Release 版本"
    echo "  ./build-windows.sh --debug      # 构建 Debug 版本"
    echo "  ./build-windows.sh --check-toolchain  # 检查编译环境"
    echo ""
    echo "输出位置:"
    echo "  Release: out/camera-ftp-companion.exe"
    echo "  Debug:   out/camera-ftp-companion-debug.exe"
    echo ""
    echo "注意: 推荐使用 ./build.sh windows 进行构建，会自动生成类型绑定"
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
        check_windows_env
    else
        check_windows_env && build_windows "$BUILD_TYPE"
    fi
}

main "$@"
