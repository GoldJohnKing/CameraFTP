#!/bin/bash
# Windows 构建脚本

set -e

# 引入公共函数库
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

# 切换到项目根目录
cd "$SCRIPT_DIR/.."

# 环境检查
check_windows_env() {
    info "正在检查 Windows 编译环境..."
    # 通用工具 (bun, cargo) 已由 build.sh 检查，此处仅做平台特定检查
    success "Windows 编译环境检查通过"
    return 0
}

# 构建
terminate_running_process() {
    info "正在检查运行中的进程..."
    if taskkill.exe /F /IM camera-ftp-companion.exe >/dev/null 2>&1; then
        info "已终止旧进程: camera-ftp-companion.exe"
    else
        info "未检测到运行中的实例"
    fi
}

build_windows() {
    local BUILD_TYPE="${1:-release}"

    info "开始构建 Windows 应用程序 ($BUILD_TYPE 模式)..."

    terminate_running_process

    # 获取 cargo 命令 (支持 WSL 中使用 Windows 版 cargo.exe)
    local cargo_cmd
    cargo_cmd=$(get_tool_cmd "cargo")
    local target="x86_64-pc-windows-msvc"

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

# 帮助
show_standalone_help() {
    cat << EOF
用法: ./build-windows.sh [选项]

选项:
  --release         构建 Release 版本 (默认)
  --debug           构建 Debug 版本
  --check           仅检查环境，不编译
  --help, -h        显示此帮助信息

示例:
  ./build-windows.sh              # 构建 Release 版本
  ./build-windows.sh --debug      # 构建 Debug 版本
  ./build-windows.sh --check      # 检查编译环境

输出位置:
  Release: out/camera-ftp-companion.exe
  Debug:   out/camera-ftp-companion-debug.exe

注意: 推荐使用 ./build.sh windows 进行构建，会自动生成类型绑定
EOF
}

# 主函数
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
        check_windows_env
    else
        check_windows_env && build_windows "$BUILD_TYPE"
    fi
}

main "$@"
