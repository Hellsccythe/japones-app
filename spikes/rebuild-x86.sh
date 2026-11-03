#!/bin/bash
# Recompila a .so x86_64 e reempacota o APK do emulador. llama.cpp e VOICEVOX são C++
# e precisam da libc++_shared.so do NDK dentro do APK. FEATURES="--features devtools" liga o inspetor.
LOG=/e/projetos/japones-app/spikes/rebuild-x86.log; exec > $LOG 2>&1
NDKR=E:/dev/android-sdk/ndk/28.2.13676358
NDKB=/e/dev/android-sdk/ndk/28.2.13676358/toolchains/llvm/prebuilt/windows-x86_64/bin
export PATH="/e/dev/jdk17/bin:/e/dev/cmake-venv/Scripts:$NDKB:$PATH" JAVA_HOME='E:/dev/jdk17' ANDROID_HOME='E:/dev/android-sdk'
export TEMP='E:/dev/tmp' TMP='E:/dev/tmp' JAVA_TOOL_OPTIONS='-Djdk.net.unixdomain.tmpdir=E:/dev/tmp -Djava.io.tmpdir=E:/dev/tmp'
export LIBCLANG_PATH=E:/dev/cmake-venv/Lib/site-packages/clang/native
# Lets tauri-build list the plugins' Android modules in gen/android/tauri.settings.gradle.
export TAURI_ANDROID_PROJECT_PATH=E:/projetos/japones-app/app/gen/android
export WRY_ANDROID_PACKAGE=com.japonesapp.desktop WRY_ANDROID_LIBRARY=japones_app_lib WRY_ANDROID_KOTLIN_FILES_OUT_DIR=E:/projetos/japones-app/app/gen/android/app/src/main/java/com/japonesapp/desktop/generated
export CMAKE_GENERATOR=Ninja ANDROID_NDK_HOME=$NDKR ANDROID_NDK_ROOT=$NDKR ANDROID_PLATFORM=24
export CC_x86_64_linux_android=x86_64-linux-android24-clang.cmd CXX_x86_64_linux_android=x86_64-linux-android24-clang++.cmd AR_x86_64_linux_android=llvm-ar.exe CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=x86_64-linux-android24-clang.cmd
cd /e/projetos/japones-app
rm -f spikes/rebuild-x86.done
cargo build -p japones-app --lib --release --target x86_64-linux-android $FEATURES && cp target/x86_64-linux-android/release/libjapones_app_lib.so app/gen/android/app/src/main/jniLibs/x86_64/ \
  && cp $NDKR/toolchains/llvm/prebuilt/windows-x86_64/sysroot/usr/lib/x86_64-linux-android/libc++_shared.so app/gen/android/app/src/main/jniLibs/x86_64/ \
  && cd app/gen/android && ./gradlew.bat assembleX86_64Debug -x rustBuildX86_64Debug --no-daemon
echo "exit=$?" > /e/projetos/japones-app/spikes/rebuild-x86.done
