#!/bin/bash
# Fase 0: empacota os APKs de debug com as .so já compiladas.
# Java 17+ no Windows usa socket Unix no TEMP; o caminho curto (HELLSC~1) quebra isso.
export JAVA_HOME='E:/dev/jdk17' ANDROID_HOME='E:/dev/android-sdk' PATH="/e/dev/jdk17/bin:$PATH"
export TEMP='E:/dev/tmp' TMP='E:/dev/tmp'
export JAVA_TOOL_OPTIONS='-Djdk.net.unixdomain.tmpdir=E:/dev/tmp -Djava.io.tmpdir=E:/dev/tmp'
cd /e/projetos/japones-app/app/gen/android
rm -f /e/projetos/japones-app/spikes/gradle.done
./gradlew.bat assembleX86_64Debug assembleArm64Debug -x rustBuildX86_64Debug -x rustBuildArm64Debug --no-daemon > /e/projetos/japones-app/spikes/gradle.log 2>&1
echo "exit=$?" > /e/projetos/japones-app/spikes/gradle.done
