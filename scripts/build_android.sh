#!/usr/bin/env bash
#
# SingPanel Android Build Script
#
# Usage:
#   ./scripts/build_android.sh [VERSION] [OPTIONS]
#
# Examples:
#   ./scripts/build_android.sh                  # Build with default stable sing-box (v1.13.19)
#   ./scripts/build_android.sh v1.13.19         # Build with specified version
#   ./scripts/build_android.sh v1.13.19 --release
#   ./scripts/build_android.sh --skip-core      # Skip rebuilding libbox.aar, just assemble APK
#

set -euo pipefail

DEFAULT_SING_BOX_VERSION="v1.13.19"
BUILD_TYPE="debug"
SKIP_CORE=false
SPECIFIED_VERSION=""

# Parse arguments
for arg in "$@"; do
    case "$arg" in
        --release)
            BUILD_TYPE="release"
            ;;
        --debug)
            BUILD_TYPE="debug"
            ;;
        --skip-core)
            SKIP_CORE=true
            ;;
        --help|-h)
            echo "Usage: $0 [SING_BOX_VERSION] [--debug|--release] [--skip-core]"
            echo ""
            echo "Arguments:"
            echo "  SING_BOX_VERSION   Official sing-box git tag/branch (default: ${DEFAULT_SING_BOX_VERSION})"
            echo "  --debug            Build debug APK (default)"
            echo "  --release          Build release APK"
            echo "  --skip-core        Skip gomobile build of libbox.aar if already present"
            exit 0
            ;;
        *)
            if [[ -z "$SPECIFIED_VERSION" && ! "$arg" =~ ^-- ]]; then
                SPECIFIED_VERSION="$arg"
            fi
            ;;
    esac
done

SING_BOX_VERSION="${SPECIFIED_VERSION:-$DEFAULT_SING_BOX_VERSION}"
if [[ ! "$SING_BOX_VERSION" =~ ^v ]]; then
    SING_BOX_VERSION="v${SING_BOX_VERSION}"
fi

# Detect workspace root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
MOBILE_DIR="${ROOT_DIR}/mobile"
LIBS_DIR="${MOBILE_DIR}/app/libs"
BUILD_CACHE_DIR="${ROOT_DIR}/.build-cache/sing-box"

echo "=================================================="
echo " SingPanel Android Packaging"
echo " Target sing-box version : ${SING_BOX_VERSION}"
echo " Build Type              : ${BUILD_TYPE}"
echo " Workspace Root          : ${ROOT_DIR}"
echo "=================================================="

# 1. Environment Detection
if ! command -v go &>/dev/null; then
    echo "[-] Error: Go compiler ('go') not found in PATH." >&2
    exit 1
fi

# Find Android SDK
if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
    POSSIBLE_SDK_DIRS=(
        "$HOME/Library/Android/sdk"
        "$HOME/Android/Sdk"
        "/Users/box/.local/share/mise/installs/android-sdk/22.0"
        "/opt/android-sdk"
    )
    for dir in "${POSSIBLE_SDK_DIRS[@]}"; do
        if [[ -d "$dir" ]]; then
            export ANDROID_HOME="$dir"
            export ANDROID_SDK_ROOT="$dir"
            break
        fi
    done
fi

if [[ -n "${ANDROID_HOME:-}" ]]; then
    echo "[+] Android SDK: ${ANDROID_HOME}"
    # Find Android NDK
    if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
        if [[ -d "${ANDROID_HOME}/ndk" ]]; then
            LATEST_NDK=$(ls -1d "${ANDROID_HOME}/ndk/"* 2>/dev/null | sort -V | tail -n 1 || true)
            if [[ -n "$LATEST_NDK" && -d "$LATEST_NDK" ]]; then
                export ANDROID_NDK_HOME="$LATEST_NDK"
            fi
        fi
    fi
fi

if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then
    echo "[+] Android NDK: ${ANDROID_NDK_HOME}"
fi

# Ensure gomobile is available
export PATH="$(go env GOPATH)/bin:${PATH}"
if ! command -v gomobile &>/dev/null; then
    echo "[*] Installing gomobile..."
    go install golang.org/x/mobile/cmd/gomobile@latest
    gomobile init
fi

# 2. Build libbox.aar if needed
mkdir -p "${LIBS_DIR}"
AAR_PATH="${LIBS_DIR}/libbox.aar"
CORE_VERSION_PATH="${LIBS_DIR}/libbox.version"

if [ "$SKIP_CORE" = false ]; then
    echo ""
    echo "[*] Step 1: Compiling official sing-box (${SING_BOX_VERSION}) into libbox.aar..."
    rm -f "${CORE_VERSION_PATH}"
    mkdir -p "$(dirname "${BUILD_CACHE_DIR}")"
    
    if [ ! -d "${BUILD_CACHE_DIR}/.git" ]; then
        echo "[*] Cloning SagerNet/sing-box..."
        git clone --filter=blob:none https://github.com/SagerNet/sing-box.git "${BUILD_CACHE_DIR}"
    fi

    pushd "${BUILD_CACHE_DIR}" > /dev/null
    git fetch --tags origin
    git checkout "${SING_BOX_VERSION}"

    # Handle Go 1.24 pidfd compatibility if needed
    if [ -f "experimental/libbox/pidfd_android.go" ]; then
        sed -i '' 's/\/\/go:linkname checkPidfdOnce/\/\/go:build ignore\n\/\/go:linkname checkPidfdOnce/g' experimental/libbox/pidfd_android.go 2>/dev/null || true
    fi

    # Feature tags for sing-box
    BUILD_TAGS="with_gvisor,with_quic,with_dhcp,with_wireguard,with_shadowsocksr,with_utls,with_clash_api,with_tailscale"

    TARGET_ABIS="${ANDROID_TARGET_ABIS:-android/arm,android/arm64,android/386,android/amd64}"
    echo "[*] Adding golang.org/x/mobile to the sing-box module..."
    go get golang.org/x/mobile@latest
    echo "[*] Running gomobile bind for targets: ${TARGET_ABIS}..."
    # Upstream gomobile has no -libname. -o already writes libbox.aar.
    gomobile bind \
        -v \
        -androidapi 21 \
        -javapkg io.nekohasekai \
        -tags "${BUILD_TAGS}" \
        -target "${TARGET_ABIS}" \
        -o "${AAR_PATH}" \
        ./experimental/libbox


    popd > /dev/null
    printf '%s\n' "${SING_BOX_VERSION#v}" > "${CORE_VERSION_PATH}.tmp"
    mv "${CORE_VERSION_PATH}.tmp" "${CORE_VERSION_PATH}"
    echo "[+] libbox.aar generated successfully: $(du -h "${AAR_PATH}" | awk '{print $1}')"
else
    echo "[*] Skipping libbox.aar compilation (--skip-core specified)."
fi

if [ "$SKIP_CORE" = false ]; then
    PACKAGED_CORE_VERSION="${SING_BOX_VERSION#v}"
elif [ -s "$CORE_VERSION_PATH" ]; then
    PACKAGED_CORE_VERSION="$(tr -d '\r\n' < "$CORE_VERSION_PATH")"
else
    PACKAGED_CORE_VERSION="unknown"
    echo "[!] Existing libbox.aar has no recorded version; the app will display version unknown."
fi

# 3. Assemble Android APK
echo ""
echo "[*] Step 2: Assembling APK via Gradle..."
pushd "${MOBILE_DIR}" > /dev/null

if [ "$BUILD_TYPE" = "release" ]; then
    ./gradlew :app:assembleRelease -PsingBoxVersion="${PACKAGED_CORE_VERSION}"
    OUTPUT_APK=$(find "${MOBILE_DIR}/app/build/outputs/apk/release" -name "*.apk" | head -n 1)
else
    ./gradlew :app:assembleDebug -PsingBoxVersion="${PACKAGED_CORE_VERSION}"
    OUTPUT_APK=$(find "${MOBILE_DIR}/app/build/outputs/apk/debug" -name "*.apk" | head -n 1)
fi

popd > /dev/null

echo ""
echo "=================================================="
echo " [✓] Build Finished Successfully!"
echo " Output APK: ${OUTPUT_APK}"
echo " File Size : $(du -h "${OUTPUT_APK}" | awk '{print $1}')"
echo "=================================================="
