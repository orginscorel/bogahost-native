#!/usr/bin/env bash
# ============================================================================
# Bogahost Native — Mac'te YEREL derleme + native.bogahost.com'a yayinlama
# (GitHub Actions'a hic bagimli olmadan tum surecin karsiligi.)
#
# KULLANIM:
#   export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/bogahost-updater.key)"
#   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""      # parolasiz uretildiyse bos
#   export DEPLOY_FTP_HOST=46.224.208.126
#   export DEPLOY_FTP_USER='deploy@native.bogahost.com'
#   export DEPLOY_FTP_PASS='...'
#   ./scripts/mac-publish.sh              # 4 uygulama
#   ./scripts/mac-publish.sh finans       # tek uygulama
#
# NE YAPAR:
#   1) Secilen uygulamalari derler (npm run tauri:build:mac)
#   2) Artifact'lari CI ile ayni duzene toplar (dist/macos-<app>/)
#   3) scripts/ci/publish-updates.mjs ile downloads/ + latest.json uretir
#   4) FTPS ile native.bogahost.com'a yukler
# ============================================================================
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
APPS_ALL=(finans dcim chat task)
APPS=("${@:-}")
[ -z "${APPS[0]:-}" ] && APPS=("${APPS_ALL[@]}")

VERSION="$(node -p "require('./package.json').version")"
echo "▶ Surum: $VERSION | Uygulamalar: ${APPS[*]}"

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "⚠ TAURI_SIGNING_PRIVATE_KEY yok — imzasiz derlenecek, OTOMATIK GUNCELLEME YAYINLANAMAZ."
  echo "  (Sadece DMG uretmek istiyorsaniz sorun degil.)"
  read -r -p "  Devam edilsin mi? [e/H] " a; [ "${a:-H}" = "e" ] || exit 1
fi

rm -rf dist out && mkdir -p dist

# ---------------------------------------------------------------- 1) derle
for app in "${APPS[@]}"; do
  echo "══ Derleniyor: $app ══"
  ( cd "tauri/$app" && npm install --silent && npm run tauri:build:mac )
done

# ------------------------------------------------- 2) artifact'lari topla
for app in "${APPS[@]}"; do
  B="tauri/$app/src-tauri/target/release/bundle"
  D="dist/macos-$app"; mkdir -p "$D"
  cp "$B"/dmg/*.dmg                "$D"/ 2>/dev/null || true
  cp "$B"/macos/*.app.tar.gz       "$D"/ 2>/dev/null || true
  cp "$B"/macos/*.app.tar.gz.sig   "$D"/ 2>/dev/null || true

  # Mimari tespiti (updater manifesti dogru anahtari yazsin)
  APPDIR="$(find "$B/macos" -maxdepth 1 -name '*.app' 2>/dev/null | head -1)"
  ARCHS="aarch64"
  if [ -n "$APPDIR" ]; then
    EXE="$(find "$APPDIR/Contents/MacOS" -type f 2>/dev/null | head -1)"
    [ -n "$EXE" ] && ARCHS="$(lipo -archs "$EXE" 2>/dev/null | tr ' ' '\n' | sed 's/^arm64$/aarch64/' | tr '\n' ' ' | xargs || echo aarch64)"
  fi
  echo "$ARCHS" > "$D/updater-archs.txt"
  echo "  $app → $(ls "$D" | tr '\n' ' ')"
done

# --------------------------------------------- 3) yayin agacini uret
node scripts/ci/publish-updates.mjs \
  --artifacts dist --out out --platform macos --version "$VERSION"
echo "▶ Uretilen yayin agaci:"; find out -type f | sort | sed 's/^/   /'

# ------------------------------------------------------------ 4) yukle
if [ -z "${DEPLOY_FTP_HOST:-}" ] || [ -z "${DEPLOY_FTP_USER:-}" ] || [ -z "${DEPLOY_FTP_PASS:-}" ]; then
  echo "⚠ DEPLOY_FTP_* degiskenleri yok — yukleme ATLANDI."
  echo "  Dosyalar hazir: $ROOT/out  (elle yukleyebilirsiniz)"
  exit 0
fi

echo "▶ FTPS ile yukleniyor → native.bogahost.com"
( cd out && find . -type f | while read -r f; do
    rel="${f#./}"
    curl -sS --ssl-reqd --ftp-create-dirs -k \
         -T "$f" "ftp://${DEPLOY_FTP_HOST}/${rel}" \
         --user "${DEPLOY_FTP_USER}:${DEPLOY_FTP_PASS}" \
      && echo "   ✓ $rel" || echo "   ✗ $rel"
  done )

echo
echo "✅ Tamamlandi. Kontrol: https://native.bogahost.com/latest.json"
echo "   Kurulu uygulamalar bir sonraki acilista guncellemeyi gorecek."
