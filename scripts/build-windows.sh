#!/bin/bash
# build-windows.sh - Windows 构建脚本
# 由 build.sh 调用，不生成类型绑定
set -e

# 引入公共函数库
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

# 切换到项目根目录（scripts/的父目录）
cd "$SCRIPT_DIR/.."

# ============================================
# 环境检查
# ============================================

check_windows_env() {
    info "检查 Windows 编译环境..."
    local failed=false
    
    # 检查 bun
    if ! check_bun; then
        failed=true
    fi
    
    # 检查 cargo.exe
    if ! command -v cargo.exe &> /dev/null; then
        error "cargo.exe 未找到"
        echo "请确保 Rust 已安装并添加到 PATH: https://rustup.rs"
        failed=true
    else
        info "Cargo: $(cargo.exe --version)"
    fi
    
    if [ "$failed" = true ]; then
        return 1
    fi
    
    success "Windows 环境检查通过"
    return 0
}

# ============================================
# 构建
# ============================================

# 执行 Cargo 构建（内部函数）
run_cargo_build() {
    local build_type="$1"
    local target="x86_64-pc-windows-msvc"

    cd src-tauri
    info "[Rust] 构建中..."

    if [ "$build_type" = "debug" ]; then
        cargo.exe build --target "$target"
    else
        cargo.exe build --release --target "$target"
    fi

    local result=$?
    cd ..
    return $result
}

# 构建前端（内部函数）
run_frontend_build() {
    info "[前端] 安装依赖..."
    bun install

    info "[前端] 构建中..."
    bun run build
}

# 终止运行中的进程
terminate_running_process() {
    info "终止运行中的进程..."
    taskkill.exe /F /IM camera-ftp-companion.exe >/dev/null 2>&1
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        info "已终止 camera-ftp-companion.exe"
    elif [ $exit_code -eq 128 ]; then
        info "没有运行中的实例"
    else
        error "无法终止进程 (exit code: $exit_code)"
        exit 1
    fi
}

build_windows() {
    local BUILD_TYPE="${1:-release}"

    info "开始构建 Windows 应用 ($BUILD_TYPE)..."

    terminate_running_process

    # 检查是否已由 build.sh 统一构建前端
    if [ "${FRONTEND_ALREADY_BUILT}" = "1" ]; then
        info "前端已由 build.sh 统一构建，跳过..."

        run_cargo_build "$BUILD_TYPE" || {
            error "Rust 构建失败"
            exit 1
        }
    else
        # 并行构建前端和 Rust 后端
        info "并行构建前端和后端..."

        # 启动前端构建（后台）
        (run_frontend_build) &
        FRONTEND_PID=$!

        # 启动 Rust 构建
        run_cargo_build "$BUILD_TYPE"
        RUST_RESULT=$?

        # 等待前端构建完成
        wait $FRONTEND_PID
        FRONTEND_RESULT=$?

        if [ $RUST_RESULT -ne 0 ]; then
            error "Rust 构建失败"
            exit 1
        fi

        if [ $FRONTEND_RESULT -ne 0 ]; then
            error "前端构建失败"
            exit 1
        fi
    fi

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

# ============================================
# 帮助
# ============================================

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

# ============================================
# 主函数
# ============================================

main() {
    local BUILD_TYPE="release"
    local CHECK_ONLY=false
    
    # 解析参数
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
                show_standalone_help
                exit 0
                ;;
            *)
                error "未知参数: $1"
                show_standalone_help
                exit 1
                ;;
        esac
    done
    
    if [ "$CHECK_ONLY" = true ]; then
        check_windows_env
    else
        check_windows_env && build_windows "$BUILD_TYPE"
    fi
}

main "$@"
