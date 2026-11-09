#!/bin/bash
# Builds the Android APK without the Tauri CLI, which needs Windows Developer
# Mode for the symlink it makes to the Rust library: compile the library with
# the NDK, copy it into jniLibs and package with Gradle.
#
#   scripts/build-apk.sh aarch64 release   # phones (signed if server/app/gen/android/keystore.properties exists)
#   scripts/build-apk.sh x86_64 debug      # the emulator (debug key, installs as is)
#
# The Rust library is always built in release mode; "debug" only changes the
# APK signing and the debuggable WebView. Set JAVA_HOME, ANDROID_HOME, NDK_HOME
# and LIBCLANG_PATH if your tools live elsewhere.
set -euo pipefail

ARCH=${1:-aarch64}
TYPE=${2:-release}
case "$ARCH" in
  aarch64) TRIPLE=aarch64-linux-android; ABI=arm64-v8a; FLAVOR=Arm64 ;;
  x86_64) TRIPLE=x86_64-linux-android; ABI=x86_64; FLAVOR=X86_64 ;;
  *) echo "arquitetura: aarch64 ou x86_64" >&2; exit 1 ;;
esac
case "$TYPE" in
  release) VARIANT=Release ;;
  debug) VARIANT=Debug ;;
  *) echo "tipo: release ou debug" >&2; exit 1 ;;
esac

# Git Bash on Windows: `pwd -W` gives E:/... paths that Gradle and cargo accept.
ROOT=$(cd "$(dirname "$0")/.." && pwd -W)
ANDROID=$ROOT/server/app/gen/android

export JAVA_HOME=${JAVA_HOME:-E:/dev/jdk17}
export ANDROID_HOME=${ANDROID_HOME:-E:/dev/android-sdk}
export NDK_HOME=${NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}
export ANDROID_NDK_HOME=$NDK_HOME ANDROID_NDK_ROOT=$NDK_HOME ANDROID_PLATFORM=24
export LIBCLANG_PATH=${LIBCLANG_PATH:-E:/dev/cmake-venv/Lib/site-packages/clang/native}
NDK_BIN=$NDK_HOME/toolchains/llvm/prebuilt/windows-x86_64/bin
export PATH="$(cygpath -u "$JAVA_HOME")/bin:$(cygpath -u "$NDK_BIN"):/e/dev/cmake-venv/Scripts:$PATH"

# Java 17+ on Windows puts a Unix socket in TEMP, and the short 8.3 path breaks it.
mkdir -p /e/dev/tmp
export TEMP=E:/dev/tmp TMP=E:/dev/tmp
export JAVA_TOOL_OPTIONS="-Djdk.net.unixdomain.tmpdir=E:/dev/tmp -Djava.io.tmpdir=E:/dev/tmp"

# What the Tauri CLI sets for its own cargo build.
export TAURI_ANDROID_PROJECT_PATH=$ANDROID
export WRY_ANDROID_PACKAGE=com.japonesapp.desktop WRY_ANDROID_LIBRARY=japones_app_lib
export WRY_ANDROID_KOTLIN_FILES_OUT_DIR=$ANDROID/app/src/main/java/com/japonesapp/desktop/generated
export CMAKE_GENERATOR=Ninja

# CMake (llama.cpp, Open JTalk) doesn't find the NDK compilers by itself.
T=${TRIPLE//-/_}
export "CC_$T=${TRIPLE}24-clang.cmd" "CXX_$T=${TRIPLE}24-clang++.cmd" "AR_$T=llvm-ar.exe"
export "CARGO_TARGET_${T^^}_LINKER=${TRIPLE}24-clang.cmd"

cd "$ROOT"
cargo build -p japones-app --lib --release --target "$TRIPLE" --features tauri/custom-protocol
mkdir -p "$ANDROID/app/src/main/jniLibs/$ABI"
cp "target/$TRIPLE/release/libjapones_app_lib.so" "$ANDROID/app/src/main/jniLibs/$ABI/"

cd "$ANDROID"
./gradlew.bat "assemble$FLAVOR$VARIANT" -x "rustBuild$FLAVOR$VARIANT" --no-daemon
find "$ANDROID/app/build/outputs/apk" -name "*.apk" -newer "$ANDROID/app/src/main/jniLibs/$ABI/libjapones_app_lib.so"
