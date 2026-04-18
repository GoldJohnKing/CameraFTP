#!/bin/bash
# build.sh - 统一构建入口
set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/scripts/build-common.sh"

cd "$SCRIPT_DIR"

TARGETS=()
SERIAL_MODE=false
NEED_GEN_TYPES=false
BUILD_TYPE="release"
CHECK_ONLY=false

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
        windows|android|frontend)
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
            echo "使用 --help 查看可用选项"
            echo ""
            show_build_help "build.sh"
            exit 1
            ;;
    esac
done

if [ ${#TARGETS[@]} -eq 0 ] && [ "$NEED_GEN_TYPES" = false ]; then
    show_build_help "build.sh"
    exit 0
fi

# 构建函数
build_target() {
    local target="$1"
    local build_type="$2"
    local check_only="$3"
    local check_arg=""

    if [ "$check_only" = true ]; then
        task "[$target] 正在检查编译环境..."
        check_arg="--check"
    else
        task "[$target] 开始构建（$build_type 模式）..."
    fi

    ./scripts/build-$target.sh "--$build_type" $check_arg
}

# 主流程
echo ""
echo "=========================================="
echo "  图传伴侣 (CameraFTP)"
echo "  统一构建脚本"
echo "=========================================="
echo ""

if [ ${#TARGETS[@]} -eq 0 ]; then
    generate_ts_types
    success "类型绑定生成完成"
    exit 0
fi

info "编译目标：${TARGETS[*]}"
info "编译模式：$BUILD_TYPE"

START_TIME=$(date +%s)

FRONTEND_TARGET=""
BUILD_TARGETS=()

for target in "${TARGETS[@]}"; do
    if [ "$target" = "frontend" ]; then
        FRONTEND_TARGET="frontend"
    else
        BUILD_TARGETS+=("$target")
    fi
done

check_common_tools() {
    if [ "${CHECK_ONLY:-false}" = true ]; then
        info "检查通用编译环境..."
    fi
    local failed=false
    
    if ! check_bun; then
        failed=true
    fi
    
    if ! check_tool "cargo" "Cargo"; then
        failed=true
    fi
    
    if [ "$failed" = true ]; then
        exit 1
    fi
    
    success "通用环境检查通过"
}

check_common_tools

NEED_BUILD_FRONTEND=false
if [ "$CHECK_ONLY" = false ] && [ ${#BUILD_TARGETS[@]} -gt 0 -o -n "$FRONTEND_TARGET" ]; then
    NEED_BUILD_FRONTEND=true
fi

if [ "$NEED_BUILD_FRONTEND" = true ]; then
    generate_ts_types
    ./scripts/build-frontend.sh
fi

if [ ${#BUILD_TARGETS[@]} -eq 0 ]; then
    success "构建完成"
    exit 0
fi

export FRONTEND_ALREADY_BUILT=1

FAILED_TARGETS=()

if [ "$SERIAL_MODE" = true ] || [ "$CHECK_ONLY" = true ]; then
    for target in "${BUILD_TARGETS[@]}"; do
        if ! build_target "$target" "$BUILD_TYPE" "$CHECK_ONLY"; then
            FAILED_TARGETS+=("$target")
        fi
    done
else
    PIDS=()

    use_prefix=false
    if [ ${#BUILD_TARGETS[@]} -gt 1 ]; then
        use_prefix=true
        info "多目标并行编译，根据前缀区分编译目标"
    fi

    for target in "${BUILD_TARGETS[@]}"; do
        if [ "$use_prefix" = true ]; then
            case "$target" in
                windows)
                    (
                        set -o pipefail
                        build_target "$target" "$BUILD_TYPE" false 2>&1 | sed "s/^/\x1b[${TARGET_WINDOWS_COLOR}m[${TARGET_WINDOWS_NAME}]\x1b[0m /"
                    ) &
                    ;;
                android)
                    (
                        set -o pipefail
                        build_target "$target" "$BUILD_TYPE" false 2>&1 | sed "s/^/\x1b[${TARGET_ANDROID_COLOR}m[${TARGET_ANDROID_NAME}]\x1b[0m /"
                    ) &
                    ;;
            esac
        else
            build_target "$target" "$BUILD_TYPE" false &
        fi

        PIDS+=($!)
    done

    for i in "${!PIDS[@]}"; do
        if ! wait "${PIDS[$i]}"; then
            FAILED_TARGETS+=("${BUILD_TARGETS[$i]}")
        fi
    done
fi

# 汇总并显示构建结果
if [ ${#FAILED_TARGETS[@]} -gt 0 ]; then
    echo ""
    error "以下目标构建失败:"
    for failed in "${FAILED_TARGETS[@]}"; do
        echo "  - $failed"
    done
    exit 1
fi

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
if [ "$CHECK_ONLY" = true ]; then
    success "环境检查完成! 耗时: ${DURATION}s"
else
    success "所有构建完成! 耗时: ${DURATION}s"
    info "输出目录: $OUTPUT_DIR/"
fi
