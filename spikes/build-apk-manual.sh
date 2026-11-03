#!/bin/bash
# Fase 0: contorna o symlink do `tauri android build` (exige Modo de Desenvolvedor no Windows):
# compila as .so em release com o NDK, copia para jniLibs e empacota com o Gradle.
set -x
LOG=/e/projetos/japones-app/spikes/apk-manual.log
exec > $LOG 2>&1
NDKR=E:/dev/android-sdk/ndk/28.2.13676358
NDKB=/e/dev/android-sdk/ndk/28.2.13676358/toolchains/llvm/prebuilt/windows-x86_64/bin
export PATH="/e/dev/jdk17/bin:/e/dev/cmake-venv/Scripts:$NDKB:$PATH" JAVA_HOME='E:\dev\jdk17' ANDROID_HOME='E:\dev\android-sdk'
export CMAKE_GENERATOR=Ninja ANDROID_NDK_HOME=$NDKR ANDROID_NDK_ROOT=$NDKR ANDROID_PLATFORM=24
cd /e/projetos/japones-app
for pair in "x86_64-linux-android x86_64 X86_64" "aarch64-linux-android arm64-v8a Arm64"; do
  set -- $pair; T=$1; ABI=$2; U=$(echo $T | tr '-' '_'); UP=$(echo $U | tr a-z A-Z)
  export CC_$U=$(echo $T)24-clang.cmd CXX_$U=$(echo $T)24-clang++.cmd AR_$U=llvm-ar.exe CARGO_TARGET_${UP}_LINKER=$(echo $T)24-clang.cmd
  cargo build -p japones-app --lib --release --target $T || { echo "FAILED $T"; echo fail > /e/projetos/japones-app/spikes/apk-manual.done; exit 1; }
  mkdir -p app/gen/android/app/src/main/jniLibs/$ABI
  cp target/$T/release/libjapones_app_lib.so app/gen/android/app/src/main/jniLibs/$ABI/
done
cd app/gen/android
cmd //c "gradlew.bat assembleX86_64Debug -x rustBuildX86_64Debug --no-daemon"
cmd //c "gradlew.bat assembleArm64Debug -x rustBuildArm64Debug --no-daemon"
find app/build/outputs/apk -name "*.apk" -exec ls -la {} \;
echo ok > /e/projetos/japones-app/spikes/apk-manual.done
