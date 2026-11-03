#!/bin/bash
# Like build-release-apk.sh, but copies the library itself instead of the Tauri CLI
# (which needs Windows Developer Mode for its symlink).
LOG=/e/projetos/japones-app/spikes/apk-release.log; exec > $LOG 2>&1
NDKR=E:/dev/android-sdk/ndk/28.2.13676358
NDKB=/e/dev/android-sdk/ndk/28.2.13676358/toolchains/llvm/prebuilt/windows-x86_64/bin
export PATH="/e/dev/jdk17/bin:/e/dev/cmake-venv/Scripts:$NDKB:$PATH" JAVA_HOME='E:/dev/jdk17' ANDROID_HOME='E:/dev/android-sdk'
export TEMP='E:/dev/tmp' TMP='E:/dev/tmp' JAVA_TOOL_OPTIONS='-Djdk.net.unixdomain.tmpdir=E:/dev/tmp -Djava.io.tmpdir=E:/dev/tmp'
export LIBCLANG_PATH=E:/dev/cmake-venv/Lib/site-packages/clang/native
export TAURI_ANDROID_PROJECT_PATH=E:/projetos/japones-app/app/gen/android
# What the Tauri CLI sets for its own cargo build.
export WRY_ANDROID_PACKAGE=com.japonesapp.desktop WRY_ANDROID_LIBRARY=japones_app_lib WRY_ANDROID_KOTLIN_FILES_OUT_DIR=E:/projetos/japones-app/app/gen/android/app/src/main/java/com/japonesapp/desktop/generated
export CMAKE_GENERATOR=Ninja NDK_HOME=$NDKR ANDROID_NDK_HOME=$NDKR ANDROID_NDK_ROOT=$NDKR ANDROID_PLATFORM=24
export CC_aarch64_linux_android=aarch64-linux-android24-clang.cmd CXX_aarch64_linux_android=aarch64-linux-android24-clang++.cmd AR_aarch64_linux_android=llvm-ar.exe CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android24-clang.cmd
cd /e/projetos/japones-app
cargo build -p japones-app --lib --release --target aarch64-linux-android --features tauri/custom-protocol \
  && cp target/aarch64-linux-android/release/libjapones_app_lib.so app/gen/android/app/src/main/jniLibs/arm64-v8a/ \
  && cd app/gen/android && ./gradlew.bat assembleArm64Release -x rustBuildArm64Release --no-daemon
echo "exit=$?"
