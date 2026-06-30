#!/bin/bash
# Android 构建脚本
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

cd "$SCRIPT_DIR/.."

DEPLOY_PATH="${DEPLOY_PATH:-/mnt/ext1/shared-files/nginx}"

declare -A SELECTED_TOOLS
declare -A SELECTED_PATHS

detect_linux_toolchain() {
    local -n java_ref="$1"
    local -n javac_ref="$2"
    local -n sdk_ref="$3"
    local -n ndk_ref="$4"
    local -n java_home_ref="$5"

    if command -v java &> /dev/null; then
        java_ref="java"
    fi
    if command -v javac &> /dev/null; then
        javac_ref="javac"
    fi
    
    sdk_ref=$(detect_linux_android_sdk || true)
    
    java_home_ref=$(detect_linux_java_home || true)
    
    if [ -n "$sdk_ref" ]; then
        if [ ! -x "$sdk_ref/platform-tools/adb" ]; then
            sdk_ref=""  # 如果缺少核心工具，清空 SDK 路径
        fi
    fi
    
    # 从 SDK 检测 NDK
    if [ -n "$sdk_ref" ]; then
        ndk_ref=$(detect_ndk_from_sdk "$sdk_ref" || true)
    fi
    
    if [ -n "$java_ref" ] && [ -n "$javac_ref" ] && [ -n "$sdk_ref" ]; then
        return 0
    fi
    return 1
}

# 检查工具链
check_toolchain() {
    debug_info "正在检查 Android 编译环境..."

    local user_java_home="${JAVA_HOME:-}"
    local user_android_home="${ANDROID_HOME:-}"
    local user_config_valid=true
    
    if [ -z "$user_java_home" ]; then
        user_config_valid=false
    elif [ ! -d "$user_java_home" ]; then
        warn "JAVA_HOME 已设置但目录不存在: $user_java_home"
        user_config_valid=false
    fi
    
    if [ -z "$user_android_home" ]; then
        user_config_valid=false
    elif [ ! -d "$user_android_home" ]; then
        warn "ANDROID_HOME 已设置但目录不存在: $user_android_home"
        user_config_valid=false
    fi
    
    if [ "$user_config_valid" = true ]; then
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "检测到用户已配置环境变量，跳过自动检测"
            info "  JAVA_HOME: $user_java_home"
            info "  ANDROID_HOME: $user_android_home"
        fi
        
        SELECTED_PATHS[user_configured]="true"
        
        check_keytool || return 1
        
        success "Android 编译环境检查通过"
        return 0
    fi
    
    debug_info "用户环境变量未完整配置，执行自动检测..."
    
    local java="" javac="" sdk="" ndk="" java_home=""
    
    if ! detect_linux_toolchain java javac sdk ndk java_home; then
        error "未找到完整的 Android 编译工具链"
        error "请安装:"
        error "  1. JDK 17 或 21 (apt install openjdk-21-jdk)"
        error "  2. Android SDK (https://developer.android.com/studio#command-tools)"
        error "或手动设置环境变量:"
        error "  export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64"
        error "  export ANDROID_HOME=$HOME/Android/Sdk"
        return 1
    fi

    SELECTED_TOOLS[java]="$java"
    SELECTED_TOOLS[javac]="$javac"
    SELECTED_PATHS[android_sdk]="$sdk"
    SELECTED_PATHS[android_ndk]="$ndk"
    SELECTED_PATHS[java_home]="$java_home"

    if [ "${CHECK_ONLY:-false}" = true ]; then
        info "[检测到的工具链]"
        info "  Java:   ${java:-未找到}"
        info "  Javac:  ${javac:-未找到}"
        info "  SDK:    ${sdk:-未找到}"
        info "  NDK:    ${ndk:-未找到}"
        info "  JAVA_HOME: ${java_home:-未找到}"
    fi

    check_keytool || return 1

    success "Android 编译环境检查通过"
}

check_keytool() {
    if command -v keytool &> /dev/null; then
        SELECTED_TOOLS[keytool]="keytool"
        return 0
    fi
    warn "keytool 未找到，签名功能不可用"
    return 1
}

# 更新 PATH
_update_android_path() {
    local new_paths=()
    [ -d "$JAVA_HOME/bin" ] && new_paths+=("$JAVA_HOME/bin")
    [ -d "$ANDROID_HOME/platform-tools" ] && new_paths+=("$ANDROID_HOME/platform-tools")
    [ -d "$ANDROID_HOME/cmdline-tools/latest/bin" ] && new_paths+=("$ANDROID_HOME/cmdline-tools/latest/bin")

    if [ ${#new_paths[@]} -gt 0 ]; then
        local path_prefix
        path_prefix=$(printf "%s:" "${new_paths[@]}")
        path_prefix="${path_prefix%:}"
        export PATH="$path_prefix:$PATH"
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "已更新 PATH: ${new_paths[*]}"
        fi
    fi
}

# 设置 NDK_HOME
_setup_ndk_home() {
    local source_label="$1"

    if [ -z "${NDK_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
        local ndk_version
        for ndk_version in "$ANDROID_HOME/ndk"/*; do
            if [ -d "$ndk_version" ]; then
                export NDK_HOME="$ndk_version"
                if [ "${CHECK_ONLY:-false}" = true ]; then
                    info "NDK_HOME ($source_label): $NDK_HOME"
                fi
                break
            fi
        done
    fi
}

# 环境变量设置
setup_android_env() {
    local user_configured="${SELECTED_PATHS[user_configured]:-false}"
    local ndk_source=""

    if [ "$user_configured" = true ]; then
        if [ ! -d "${JAVA_HOME:-}" ]; then
            error "JAVA_HOME 目录不存在: ${JAVA_HOME:-未设置}"
            return 1
        fi
        if [ ! -d "${ANDROID_HOME:-}" ]; then
            error "ANDROID_HOME 目录不存在: ${ANDROID_HOME:-未设置}"
            return 1
        fi
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "使用用户配置的环境变量"
        fi
        ndk_source="自动检测"
    else
        if [ -n "${SELECTED_PATHS[java_home]:-}" ]; then
            export JAVA_HOME="${SELECTED_PATHS[java_home]}"
            if [ "${CHECK_ONLY:-false}" = true ]; then
                info "JAVA_HOME (自动检测): $JAVA_HOME"
            fi
        else
            export JAVA_HOME="/usr/lib/jvm/java-21-openjdk-amd64"
            warn "JAVA_HOME 未检测到，使用默认值: $JAVA_HOME"
        fi

        if [ -n "${SELECTED_PATHS[android_sdk]:-}" ]; then
            export ANDROID_HOME="${SELECTED_PATHS[android_sdk]}"
            export ANDROID_SDK_ROOT="${SELECTED_PATHS[android_sdk]}"
            if [ "${CHECK_ONLY:-false}" = true ]; then
                info "ANDROID_HOME (自动检测): $ANDROID_HOME"
            fi
        else
            export ANDROID_HOME="$HOME/Android/Sdk"
            warn "ANDROID_HOME 未检测到，使用默认值: $ANDROID_HOME"
        fi
        ndk_source="自动检测"
    fi

    if [ ! -d "${JAVA_HOME:-}" ]; then
        error "JAVA_HOME 目录不存在: ${JAVA_HOME:-未设置}"
        error "请安装 JDK 17 或 21，并设置 JAVA_HOME 环境变量"
        return 1
    fi

    export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"

    if [ ! -d "${ANDROID_HOME:-}" ]; then
        error "ANDROID_HOME 目录不存在: ${ANDROID_HOME:-未设置}"
        error "请安装 Android SDK，并设置 ANDROID_HOME 环境变量"
        return 1
    fi

    if [ "$user_configured" != true ] && [ -n "${SELECTED_PATHS[android_ndk]:-}" ]; then
        export NDK_HOME="${SELECTED_PATHS[android_ndk]}"
        if [ "${CHECK_ONLY:-false}" = true ]; then
            info "NDK_HOME (自动检测): $NDK_HOME"
        fi
    else
        _setup_ndk_home "$ndk_source"
    fi

    _update_android_path
    export GRADLE_OPTS="-Dorg.gradle.parallel=true"

    return 0
}

# 签名密钥
check_or_create_keystore() {
    local keystore_path="src-tauri/gen/android/keystore.properties"
    local keystore_file="cameraftp.keystore"

    local key_alias="${KEYSTORE_ALIAS:-cameraftp}"
    local key_store_pass="${KEYSTORE_PASSWORD:-cameraftp123}"
    local key_pass="${KEY_PASSWORD:-$key_store_pass}"
    local key_dname="${KEYSTORE_DNAME:-CN=CameraFTP, OU=Development, O=GJK, L=Unknown, ST=Unknown, C=CN}"

    if [ ! -f "$keystore_path" ]; then
        warn "签名配置不存在，创建新的签名密钥..."

        local keytool_cmd="${SELECTED_TOOLS[keytool]:-keytool}"
        $keytool_cmd -genkey -v \
            -keystore "$keystore_file" \
            -alias "$key_alias" \
            -keyalg RSA \
            -keysize 2048 \
            -validity 10000 \
            -dname "$key_dname" \
            -storepass "$key_store_pass" \
            -keypass "$key_pass"

        mv "$keystore_file" "src-tauri/gen/android/$keystore_file"

        cat > "$keystore_path" << EOF
storeFile=$keystore_file
storePassword=$key_store_pass
keyAlias=$key_alias
keyPassword=$key_pass
EOF

        success "签名密钥已创建: src-tauri/gen/android/$keystore_file"
        info "密钥信息已保存到: $keystore_path"

        if [ "$key_store_pass" = "cameraftp123" ]; then
            warn "使用的是默认密钥密码，建议设置 KEYSTORE_PASSWORD 环境变量"
        fi
    fi
}

# Find libomp.so from the NDK (OpenMP runtime needed by RawAlchemyCpp)
find_ndk_libomp() {
    local ndk_home="${1:-$NDK_HOME}"
    if [ -z "$ndk_home" ] || [ ! -d "$ndk_home" ]; then
        return 1
    fi
    # NDK path pattern: $NDK/toolchains/llvm/prebuilt/<host>/lib/clang/<ver>/lib/linux/aarch64/libomp.so
    local omp_so
    omp_so="$(find "$ndk_home/toolchains/llvm/prebuilt" -path "*/aarch64/libomp.so" 2>/dev/null | head -1)"
    if [ -n "$omp_so" ] && [ -f "$omp_so" ]; then
        echo "$omp_so"
        return 0
    fi
    return 1
}

# Package the NN runtime (ONNX Runtime + Qualcomm QNN HTP backend) into the
# APK via extra-jniLibs. Only arm64-v8a is targeted, and only when the
# nn-cache is populated (./scripts/fetch-nn-deps.sh). libonnxruntime.so
# comes from the ORT AAR; libQnnSystem.so / libQnnHtp.so / the per-Hexagon
# libQnnHtpV*Skel.so (DSP-side) + libQnnHtpV*Stub.so (CPU-side transport)
# come from the qnn-runtime AAR. Without these, the NN
# demosaic path degrades gracefully to the classical algorithm.
package_nn_android() {
    local nn_cache="src-tauri/lib/rawalchemy/third_party/nn-cache"
    local ort_dir="$nn_cache/onnxruntime-android-qnn-1.24.1/jni/arm64-v8a"
    local qnn_dir="$nn_cache/qnn-runtime-2.42.0/jni/arm64-v8a"
    local nn_jni_dir="src-tauri/gen/android/app/extra-jniLibs/arm64-v8a"

    if [ ! -d "$ort_dir" ] || [ ! -d "$qnn_dir" ]; then
        warn "nn-cache not populated (ORT/QNN). Run ./scripts/fetch-nn-deps.sh. NN demosaic will be unavailable."
        return 0
    fi

    mkdir -p "$nn_jni_dir"
    # Clear stale Skels/Stubs so pruned versions (V68/V69) don't linger.
    rm -f "$nn_jni_dir"/libQnnHtpV*Skel.so
    rm -f "$nn_jni_dir"/libQnnHtpV*Stub.so
    local copied=0

    # ORT runtime — required by the QNN execution provider.
    if [ -f "$ort_dir/libonnxruntime.so" ]; then
        cp "$ort_dir/libonnxruntime.so" "$nn_jni_dir/"
        copied=$((copied + 1))
    else
        warn "libonnxruntime.so missing in $ort_dir/ — NN demosaic will be unavailable"
    fi

    # QNN HTP backend essentials: libQnnHtp.so (backend) + libQnnSystem.so
    # (system) + libQnnHtpPrepare.so (~80MB, CPU-side graph compile / op
    # validation for the online workflow). Without Prepare, QNN aborts at
    # graph-build: "Failed loading libQnnHtpPrepare.so" → op validate error
    # 0xfa0/4000 → "HTP Prepare backend loading failed". Arch-agnostic
    # (single copy; no V68/V69 variant). Deps: libc/m/dl/log only.
    for lib in libQnnSystem.so libQnnHtp.so libQnnHtpPrepare.so; do
        if [ -f "$qnn_dir/$lib" ]; then
            cp "$qnn_dir/$lib" "$nn_jni_dir/"
            copied=$((copied + 1))
        fi
    done

    # Skip V68 (SD 865) and V69 (SD 888) Skels — those chips predate
    # minSdk=35 (Android 15+) hardware. V73+ covers the target tier.
    local skel_count=0
    local skipped_skels=0
    for skel in "$qnn_dir"/libQnnHtpV*Skel.so; do
        [ -f "$skel" ] || continue
        case "$(basename "$skel")" in
            libQnnHtpV68Skel.so|libQnnHtpV69Skel.so)
                skipped_skels=$((skipped_skels + 1))
                continue
                ;;
        esac
        cp "$skel" "$nn_jni_dir/"
        skel_count=$((skel_count + 1))
    done
    copied=$((copied + skel_count))

    # HTP transport also dlopens the matching CPU-side Stub (libQnnHtpV*Stub.so)
    # per arch to create the FastRPC transport instance. Without it QNN fails:
    # "Failed in loading stub: ... libQnnHtpV73Stub.so not found" → 4000 →
    # INVALID_CONFIG. Same skip set as Skels (V73+ only for minSdk=35 tier).
    local stub_count=0
    for stub in "$qnn_dir"/libQnnHtpV*Stub.so; do
        [ -f "$stub" ] || continue
        case "$(basename "$stub")" in
            libQnnHtpV68Stub.so|libQnnHtpV69Stub.so) continue ;;
        esac
        cp "$stub" "$nn_jni_dir/"
        stub_count=$((stub_count + 1))
    done
    copied=$((copied + stub_count))

    if [ "$skipped_skels" -gt 0 ]; then
        info "Skipped V68/V69 Htp Skels ($skipped_skels file(s)) — targets below minSdk=35 hardware"
    fi

    if [ "$copied" -gt 0 ]; then
        success "NN runtime packaged: $copied .so(s) → extra-jniLibs/arm64-v8a/ (ORT + QNN HTP)"
    else
        warn "nn-cache present but no NN .so copied — NN demosaic will be unavailable"
    fi
}

# 构建单个 variant:
#   neural — 含 NN 推理库 (ORT/QNN) + 模型，体积大，面向骁龙8 Gen2+ 设备
#   legacy — 仅传统算法，不含 NN 库/模型，体积约小 150MB，面向其它设备
build_android() {
    local BUILD_TYPE="${1:-release}"
    local variant="${2:-neural}"

    info "开始构建 Android 应用 ($BUILD_TYPE, $variant) - 仅 arm64-v8a 架构"

    if ! setup_android_env; then
        error "环境变量设置失败，无法继续构建"
        exit 1
    fi
    check_or_create_keystore

    # NN demosaic 总开关：导出给 Rust build.rs。
    # neural=1 启用并打包模型；legacy=0 关闭且 build.rs 跳过模型压缩。
    # 未设置时 Rust 默认启用，故 Windows/单元测试不受影响。
    local nn_flag
    if [ "$variant" = "neural" ]; then
        nn_flag="1"
        export CAMERAFTP_NN_DEMOSAIC=1
    else
        nn_flag="0"
        export CAMERAFTP_NN_DEMOSAIC=0
    fi
    info "Variant=$variant  CAMERAFTP_NN_DEMOSAIC=$nn_flag"

    # Build RawAlchemyCpp .so if available (variant 透传给 CMake 与 build 子目录)
    local rawalchemy_dir="${RAWALCHEMY_DIR:-$SCRIPT_DIR/../src-tauri/lib/rawalchemy}"
    if [ -d "$rawalchemy_dir" ]; then
        local bt_upper
        if [ "$BUILD_TYPE" = "debug" ]; then
            bt_upper="Debug"
        else
            bt_upper="Release"
        fi
        "$SCRIPT_DIR/build-raw-alchemy.sh" android "$bt_upper" "$variant" || {
            error "RawAlchemyCpp Android build FAILED. Aborting — cannot produce valid APK without core library."
            exit 1
        }

        # variant 对应的 build 子目录（与 build-raw-alchemy.sh 保持一致）
        local build_subdir="build-android-arm64"
        [ "$variant" = "legacy" ] && build_subdir="build-android-arm64-legacy"
        local abs_dir
        abs_dir="$(cd "$rawalchemy_dir" && pwd)"
        local rawalchemy_so="$abs_dir/$build_subdir/libraw_alchemy.so"
        if [ -f "$rawalchemy_so" ]; then
            # Copy to extra-jniLibs (included in APK via build.gradle.kts).
            # 关键：清空所有旧 .so，避免上一个 variant 的 QNN/ORT .so 残留污染本 variant。
            local jni_dir="src-tauri/gen/android/app/extra-jniLibs/arm64-v8a"
            mkdir -p "$jni_dir"
            rm -f "$jni_dir"/*.so
            cp "$rawalchemy_so" "$jni_dir/libraw_alchemy_core.so"
            # Also copy libomp.so (OpenMP runtime required by libraw_alchemy_core.so)
            local omp_so
            omp_so="$(find_ndk_libomp "$NDK_HOME")" || true
            if [ -n "$omp_so" ]; then
                cp "$omp_so" "$jni_dir/libomp.so"
                success "RawAlchemyCpp .so + libomp.so ready ($variant)"
            else
                warn "libomp.so not found in NDK — OpenMP may fail at runtime"
                success "RawAlchemyCpp .so ready ($variant): $rawalchemy_so"
            fi
        else
            error "RawAlchemyCpp .so not found at $rawalchemy_so"
            exit 1
        fi
    else
        warn "RawAlchemyCpp not found. LUT filter feature will be unavailable."
        warn "Set RAWALCHEMY_DIR to enable it."
    fi

    # NN runtime (ORT + QNN HTP) 仅 neural variant 打包
    if [ "$variant" = "neural" ]; then
        package_nn_android
    fi

    local VERSION
    VERSION=$(get_version)

    # neural variant 通过 --config 覆盖 productName 以区分启动器；legacy 用基础配置
    local config_arg=""
    if [ "$variant" = "neural" ]; then
        config_arg="--config src-tauri/tauri.neural.conf.json"
    fi

    case $BUILD_TYPE in
        "debug")
            npx tauri android build --debug --apk --target aarch64 $config_arg || {
                error "Android debug ($variant) 构建失败"
                exit 1
            }
            move_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/debug/*.apk" \
                "CameraFTP_v${VERSION}-${variant}-debug.apk" \
                "Debug APK ($variant)" \
                "${DEPLOY_PATH:+$DEPLOY_PATH/CameraFTP_v${VERSION}-${variant}-debug.apk}"
            ;;
        "release")
            npx tauri android build --apk --target aarch64 $config_arg || {
                error "Android release ($variant) 构建失败"
                exit 1
            }
            move_to_out \
                "src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk" \
                "CameraFTP_v${VERSION}-${variant}.apk" \
                "Release APK ($variant)" \
                "${DEPLOY_PATH:+$DEPLOY_PATH/CameraFTP_v${VERSION}-${variant}.apk}"
            ;;
    esac
}

# 依次构建 neural 与 legacy 两个 variant，各产出一个 APK
build_all_variants() {
    local build_type="${1:-release}"
    build_android "$build_type" neural
    build_android "$build_type" legacy
}

# 帮助信息
show_help() {
    echo "用法: ./build-android.sh [选项]"
    echo ""
    echo "选项:"
    echo "  --release   构建 Release 版本 (默认)"
    echo "  --debug     构建 Debug 版本"
    echo "  --check     仅检查环境，不编译"
    echo "  --help, -h  显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  ./build-android.sh          # 构建 Release 版本"
    echo "  ./build-android.sh --debug  # 构建 Debug 版本"
    echo "  ./build-android.sh --check  # 检查编译环境"
    echo ""
    local VERSION
    VERSION=$(get_version)
    echo "输出位置 (每个 variant 一份 APK):"
    echo "  Release: out/CameraFTP_v${VERSION}-neural.apk / out/CameraFTP_v${VERSION}-legacy.apk"
    echo "  Debug:   out/CameraFTP_v${VERSION}-neural-debug.apk / out/CameraFTP_v${VERSION}-legacy-debug.apk"
    echo ""
    echo "注意: 推荐使用 ./build.sh android 进行构建，会自动生成类型绑定"
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
        check_toolchain
    else
        # Build first, then test: `npx tauri android build` runs cargo build
        # for the android target, which generates tauri.settings.gradle and
        # app/tauri.build.gradle.kts via tauri-build's build.rs. These files
        # are gitignored and required by `./gradlew test` — running tests
        # before the build fails on the missing settings file.
        check_toolchain && build_all_variants "$BUILD_TYPE" && run_android_tests
    fi
}

main "$@"
