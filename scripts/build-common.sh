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

# 获取项目根目录
get_project_root() {
    cd "$(dirname "${BASH_SOURCE[0]}")" && pwd
}

# 拷贝编译产物到 out 目录
# 用法: copy_to_out <源文件> <目标文件名> <构建类型描述>
copy_to_out() {
    local src_path="$1"
    local dest_name="$2"
    local build_type="$3"
    
    mkdir -p "$OUTPUT_DIR"
    
    if [ -f "$src_path" ]; then
        cp "$src_path" "$OUTPUT_DIR/$dest_name"
        success "$build_type 构建完成"
        info "输出位置: $OUTPUT_DIR/$dest_name"
    else
        error "未找到构建产物: $src_path"
        return 1
    fi
}

# 拷贝匹配模式的文件到 out 目录
# 用法: copy_pattern_to_out <源文件模式> <目标文件名> <文件类型描述>
copy_pattern_to_out() {
    local src_pattern="$1"
    local dest_name="$2"
    local file_type="$3"
    
    mkdir -p "$OUTPUT_DIR"
    
    # 使用 glob 模式而非 ls 解析
    local src_file=""
    for f in $src_pattern; do
        if [ -f "$f" ]; then
            src_file="$f"
            break
        fi
    done
    
    if [ -n "$src_file" ] && [ -f "$src_file" ]; then
        cp "$src_file" "$OUTPUT_DIR/$dest_name"
        success "$file_type 构建完成"
        info "输出位置: $OUTPUT_DIR/$dest_name"
    else
        warn "未找到 $file_type 文件: $src_pattern"
        return 1
    fi
}

# 检查通用工具 (bun)
check_bun() {
    if ! command -v bun &> /dev/null; then
        error "Bun 未安装"
        echo "安装地址: https://bun.sh"
        return 1
    fi
    info "Bun: $(bun --version)"
    return 0
}

# 生成 TypeScript 类型绑定 (使用 Windows cargo.exe)
generate_ts_types() {
    task "生成 TypeScript 类型绑定..."
    
    # 确保 dist 目录存在（tauri 编译需要）
    mkdir -p dist
    
    cd src-tauri
    cargo.exe test --quiet 2>/dev/null || true
    cd ..
    
    success "TypeScript 类型绑定已生成到 src-tauri/bindings/"
}

# 清理构建缓存
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
    
    # 运行 cargo clean
    if command -v cargo.exe &> /dev/null; then
        info "运行 cargo clean (Windows)..."
        cd src-tauri && cargo.exe clean 2>/dev/null || true && cd ..
    fi
    
    if command -v cargo &> /dev/null; then
        info "运行 cargo clean (Linux)..."
        cd src-tauri && cargo clean 2>/dev/null || true && cd ..
    fi
    
    success "清理完成"
}

# 显示使用帮助
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
