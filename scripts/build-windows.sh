#!/bin/bash
# Windows 构建脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

cd "$SCRIPT_DIR/.."

check_windows_env() {
    if ! command -v cargo.exe &> /dev/null; then
        error "cargo.exe 未找到。请安装 Windows Rust 工具链。"
        return 1
    fi
    if [ "${CHECK_ONLY:-false}" = true ]; then
        info "正在检查 Windows 编译环境..."
    fi
    success "Windows 编译环境检查通过"
}

terminate_running_process() {
    local process_name="$1"
    info "正在检查运行中的进程: $process_name"
    if taskkill.exe /F /IM "$process_name" >/dev/null 2>&1; then
        info "已终止进程: $process_name"
    fi
}

build_windows() {
    local BUILD_TYPE="${1:-release}"
    local OUTPUT_NAME="cameraftp.exe"

    info "开始构建 Windows 应用程序 ($BUILD_TYPE 模式)..."

    terminate_running_process "$OUTPUT_NAME"

    # Build RawAlchemyCpp DLL if available
    local rawalchemy_dir="${RAWALCHEMY_DIR:-$SCRIPT_DIR/../src-tauri/lib/rawalchemy}"
    if [ -d "$rawalchemy_dir" ]; then
        local bt_upper
        if [ "$BUILD_TYPE" = "debug" ]; then
            bt_upper="Debug"
        else
            bt_upper="Release"
        fi
        "$SCRIPT_DIR/build-raw-alchemy.sh" windows "$bt_upper" || {
            warn "RawAlchemyCpp build failed. Color grading will be unavailable."
        }
    else
        warn "RawAlchemyCpp not found. Color grading will be unavailable."
        warn "Set RAWALCHEMY_DIR to enable it."
    fi

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

    local VERSION=$(get_version)
    local DEST_NAME
    local SRC_PATH

    if [ "$BUILD_TYPE" = "debug" ]; then
        SRC_PATH="src-tauri/target/$TARGET_WINDOWS_TRIPLE/debug/$OUTPUT_NAME"
        DEST_NAME="CameraFTP_v${VERSION}-debug.exe"
    else
        SRC_PATH="src-tauri/target/$TARGET_WINDOWS_TRIPLE/release/$OUTPUT_NAME"
        DEST_NAME="CameraFTP_v${VERSION}.exe"
    fi

    terminate_running_process "$DEST_NAME"

    move_to_out "$SRC_PATH" "$DEST_NAME" "Windows $BUILD_TYPE"
}

# 显示帮助信息
show_help() {
    echo "用法: ./build-windows.sh [选项]"
    echo ""
    echo "选项:"
    echo "  --release   构建 Release 版本 (默认)"
    echo "  --debug     构建 Debug 版本"
    echo "  --check     仅检查环境，不编译"
    echo "  --help, -h  显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  ./build-windows.sh          # 构建 Release 版本"
    echo "  ./build-windows.sh --debug  # 构建 Debug 版本"
    echo "  ./build-windows.sh --check  # 检查编译环境"
    echo ""
    local VERSION
    VERSION=$(get_version)
    echo "输出位置:"
    echo "  Release: out/CameraFTP_v${VERSION}.exe (单文件，DLL 已内嵌)"
    echo "  Debug:   out/CameraFTP_v${VERSION}-debug.exe (单文件，DLL 已内嵌)"
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
