#!/bin/bash
# build.sh - 统一构建入口
# 支持多目标并行编译
set -e

# 引入公共函数库
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/scripts/build-common.sh"

# 切换到项目根目录
cd "$SCRIPT_DIR"

# 构建目标列表
TARGETS=()
SERIAL_MODE=false
NEED_GEN_TYPES=false
BUILD_TYPE="release"
CHECK_ONLY=false

# 解析参数
while [[ $# -gt 0 ]]; do
    case $1 in
        --serial|-s)
            SERIAL_MODE=true
            shift
            ;;
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
            show_build_help "build.sh"
            exit 0
            ;;
        windows|android)
            TARGETS+=("$1")
            shift
            ;;
        frontend)
            TARGETS+=("$1")
            shift
            ;;
        gen-types)
            NEED_GEN_TYPES=true
            shift
            ;;
        clean)
            clean_build_cache
            exit 0
            ;;
        *)
            error "未知参数: $1"
            echo ""
            show_build_help "build.sh"
            exit 1
            ;;
    esac
done

# 如果没有目标，显示帮助
if [ ${#TARGETS[@]} -eq 0 ] && [ "$NEED_GEN_TYPES" = false ]; then
    show_build_help "build.sh"
    exit 0
fi

# ============================================
# 构建函数
# ============================================

# 构建前端
build_frontend() {
    task "[前端] 构建中..."
    bun install
    bun run build
    success "[前端] 构建完成"
}

# 构建 Windows
build_windows() {
    local build_type="$1"
    local check_only="$2"
    local check_arg=""
    
    if [ "$check_only" = true ]; then
        task "[Windows] 检查环境..."
        check_arg="--check"
    else
        task "[Windows] 开始构建 ($build_type)..."
    fi
    
    ./scripts/build-windows.sh "--$build_type" $check_arg
}

# 构建 Android
build_android() {
    local build_type="$1"
    local check_only="$2"
    local check_arg=""
    
    if [ "$check_only" = true ]; then
        task "[Android] 检查环境..."
        check_arg="--check"
    else
        task "[Android] 开始构建 ($build_type)..."
    fi
    
    ./scripts/build-android.sh "--$build_type" $check_arg
}

# ============================================
# 主流程
# ============================================

echo ""
echo "=========================================="
echo "  图传伴侣 (Camera FTP Companion)"
echo "  统一构建脚本"
echo "=========================================="
echo ""

# 如果只是生成类型绑定（无编译目标）
if [ ${#TARGETS[@]} -eq 0 ]; then
    generate_ts_types
    success "类型绑定生成完成"
    exit 0
fi

# 显示编译计划
info "编译目标: ${TARGETS[*]}"
info "编译模式: $BUILD_TYPE"
if [ "$CHECK_ONLY" = true ]; then
    info "操作: 环境检查"
else
    if [ "$SERIAL_MODE" = true ]; then
        info "执行方式: 串行"
    else
        info "执行方式: 并行"
    fi
fi
echo ""

# 记录开始时间
START_TIME=$(date +%s)

# 分离前端构建
FRONTEND_TARGET=""
BUILD_TARGETS=()

for target in "${TARGETS[@]}"; do
    if [ "$target" = "frontend" ]; then
        FRONTEND_TARGET="frontend"
    else
        BUILD_TARGETS+=("$target")
    fi
done

# 构建前端（如果有且不是仅检查）
if [ -n "$FRONTEND_TARGET" ] && [ "$CHECK_ONLY" = false ]; then
    # 先生成类型绑定
    generate_ts_types
    build_frontend
fi

# 如果没有其他构建目标，退出
if [ ${#BUILD_TARGETS[@]} -eq 0 ]; then
    success "构建完成"
    exit 0
fi

# 如果不是仅检查，先生成类型绑定和前端
if [ "$CHECK_ONLY" = false ]; then
    generate_ts_types
    
    # 如果有多个构建目标，统一构建前端一次
    if [ ${#BUILD_TARGETS[@]} -gt 1 ]; then
        task "统一构建前端（多目标共享）..."
        bun install
        bun run build
        export FRONTEND_ALREADY_BUILT=1
        success "前端构建完成"
    fi
fi

# 并行或串行编译
if [ "$SERIAL_MODE" = true ] || [ "$CHECK_ONLY" = true ]; then
    # 串行编译或检查
    for target in "${BUILD_TARGETS[@]}"; do
        case $target in
            windows) build_windows "$BUILD_TYPE" "$CHECK_ONLY" ;;
            android) build_android "$BUILD_TYPE" "$CHECK_ONLY" ;;
        esac
    done
else
    # 并行编译
    PIDS=()
    PID_MAP=()
    
    # 判断是否需要前缀：多目标时需要，单目标时不需要
    use_prefix=false
    if [ ${#BUILD_TARGETS[@]} -gt 1 ]; then
        use_prefix=true
        info "多目标并行编译，启用输出前缀区分"
    fi
    
    for target in "${BUILD_TARGETS[@]}"; do
        if [ "$use_prefix" = true ]; then
            # 多目标：带颜色前缀 [TARGET]
            color=""
            target_display=""
            case "$target" in
                windows) color="36"; target_display="Windows" ;;  # 青色
                android) color="35"; target_display="Android" ;;  # 紫色
            esac
            
            (
                case $target in
                    windows) build_windows "$BUILD_TYPE" false ;;
                    android) build_android "$BUILD_TYPE" false ;;
                esac
            ) 2>&1 | sed "s/^/\x1b[${color}m[${target_display}]\x1b[0m /" &
        else
            # 单目标：无前缀
            (
                case $target in
                    windows) build_windows "$BUILD_TYPE" false ;;
                    android) build_android "$BUILD_TYPE" false ;;
                esac
            ) &
        fi
        
        PID=$!
        PIDS+=($PID)
        PID_MAP+=("$PID:$target")
        info "启动后台任务 [PID=$PID]: $target"
    done
    
    # 等待所有任务完成
    FAILED_TARGETS=()
    for i in "${!PIDS[@]}"; do
        PID=${PIDS[$i]}
        wait $PID || FAILED_TARGETS+=("${PID_MAP[$i]}")
    done
    
    # 检查失败的任务
    if [ ${#FAILED_TARGETS[@]} -gt 0 ]; then
        error "以下目标构建失败:"
        for failed in "${FAILED_TARGETS[@]}"; do
            echo "  - $failed"
        done
        exit 1
    fi
fi

# 计算耗时
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
if [ "$CHECK_ONLY" = true ]; then
    success "环境检查完成! 耗时: ${DURATION}s"
else
    success "所有构建完成! 耗时: ${DURATION}s"
    info "输出目录: $OUTPUT_DIR/"
fi
