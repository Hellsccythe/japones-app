#!/bin/bash
# Fase 0: build debug do APK (x86_64 para o emulador + arm64 para celulares).
export JAVA_HOME='E:\dev\jdk17' ANDROID_HOME='E:\dev\android-sdk' NDK_HOME='E:\dev\android-sdk\ndk\28.2.13676358'
export PATH="/e/dev/jdk17/bin:/e/dev/cmake-venv/Scripts:$PATH"
cd /e/projetos/japones-app/app
rm -f /e/projetos/japones-app/spikes/apk.done
npx -y @tauri-apps/cli@2 android build --debug --apk --target x86_64 aarch64 > /e/projetos/japones-app/spikes/apk.log 2>&1
echo "exit=$?" > /e/projetos/japones-app/spikes/apk.done
