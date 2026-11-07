#!/usr/bin/env bash
# Compila o APK sem Gradle: aapt2 + javac + R8 + zipalign + apksigner.
# Requer: JDK 17+ e Android SDK com build-tools e platforms/android-35.
#   ANDROID_HOME=/caminho/do/sdk ./build.sh
set -euo pipefail
cd "$(dirname "$0")"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/opt/android-sdk}}"
BT="${BUILD_TOOLS:-$(ls -d "$SDK"/build-tools/* 2>/dev/null | sort -V | tail -1)}"
[ -x "$BT/aapt2" ] || BT="$SDK/build-tools"
PLAT="${ANDROID_JAR:-$SDK/platforms/android-35/android.jar}"
OUT=build
APK=dist/CorreLula.apk
KS="${KEYSTORE:-keystore/corre-lula.jks}"
KS_PASS="${KEYSTORE_PASS:-correlula}"

for f in "$BT/aapt2" "$BT/zipalign" "$BT/apksigner" "$BT/lib/d8.jar" "$PLAT"; do
  [ -e "$f" ] || { echo "faltando: $f"; exit 1; }
done

rm -rf "$OUT"
mkdir -p "$OUT/classes" "$OUT/dex" dist

echo "> recursos"
"$BT/aapt2" compile --dir res -o "$OUT/res.zip"
"$BT/aapt2" link -o "$OUT/base.apk" -I "$PLAT" --manifest AndroidManifest.xml \
  --min-sdk-version 14 --target-sdk-version 34 --version-code 1 --version-name 1.0 \
  "$OUT/res.zip"

echo "> java"
javac -source 8 -target 8 -bootclasspath "$PLAT" -nowarn -Xlint:-options -encoding UTF-8 \
  -d "$OUT/classes" $(find src -name '*.java')

echo "> r8 (encolhe e otimiza)"
java -cp "$BT/lib/d8.jar" com.android.tools.r8.R8 --release --min-api 14 \
  --lib "$PLAT" --pg-conf proguard.pro --output "$OUT/dex" \
  $(find "$OUT/classes" -name '*.class')

echo "> empacota"
cp "$OUT/base.apk" "$OUT/unsigned.apk"
(cd "$OUT/dex" && zip -q -X -9 ../unsigned.apk classes.dex)
"$BT/zipalign" -f -p 4 "$OUT/unsigned.apk" "$OUT/aligned.apk"

if [ ! -f "$KS" ]; then
  echo "> criando chave de assinatura em $KS (guarde-a para publicar atualizacoes)"
  mkdir -p "$(dirname "$KS")"
  keytool -genkeypair -keystore "$KS" -storepass "$KS_PASS" -keypass "$KS_PASS" \
    -alias corre -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Corre Lula, OU=Parodia, O=Fan Game, C=BR" >/dev/null 2>&1
fi

echo "> assina"
"$BT/apksigner" sign --ks "$KS" --ks-pass "pass:$KS_PASS" --key-pass "pass:$KS_PASS" \
  --ks-key-alias corre --out "$APK" "$OUT/aligned.apk"
"$BT/apksigner" verify "$APK"
rm -f "$APK.idsig"
ls -l "$APK"
echo "pronto: $APK"
