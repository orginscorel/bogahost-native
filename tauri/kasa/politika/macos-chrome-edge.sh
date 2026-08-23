#!/bin/bash
# Bogahost Kasa — Chrome yönetilen politikası (macOS).
#
# İKİ İŞ YAPAR:
#   1) Tarayıcının KENDİ parola kasasını kapatır. Doldurma bittiğinde çıkan
#      "Şifreyi kaydedeyim mi?" balonu bir daha hiç çıkmaz. Kullanıcı bunu
#      tarayıcı ayarlarından GERİ AÇAMAZ.
#   2) Bogahost Kasa eklentisini ZORUNLU kurar. Kullanıcı silse bile Chrome
#      update.xml'i periyodik okuyup yeniden kurar; kaldırma düğmesi pasif olur.
#
# NEDEN POLİTİKA: bunların ikisi de tarayıcının kararıdır. Bir masaüstü
# uygulaması dışarıdan ne parola kasasını kapatabilir ne de eklenti kurabilir —
# desteklenen tek yol yönetilen politikadır.
#
# Kullanım:  sudo bash macos-chrome-edge.sh
set -euo pipefail

EKLENTI_ID="nfoohkianbefpbobbbgfikiiicnghjeh"
GUNCELLEME="https://native.bogahost.com/eklenti/update.xml"

if [ "$(id -u)" -ne 0 ]; then
  echo "Bu betik yönetici hakkı ister:  sudo bash $0" >&2
  exit 1
fi

DIZIN="/Library/Managed Preferences"
mkdir -p "$DIZIN"

yaz() {
  local plist="$1" ad="$2" eklenti="$3"
  defaults write "$plist" PasswordManagerEnabled -bool false
  defaults write "$plist" AutofillAddressEnabled -bool false
  defaults write "$plist" AutofillCreditCardEnabled -bool false
  if [ "$eklenti" = "evet" ]; then
    defaults write "$plist" ExtensionInstallForcelist -array "${EKLENTI_ID};${GUNCELLEME}"
    # Kendi barındırdığımız .crx'in kurulabilmesi için kaynak izni.
    defaults write "$plist" ExtensionInstallSources -array "https://native.bogahost.com/*"
  fi
  chmod 644 "$plist.plist" 2>/dev/null || true
  echo "  ✓ $ad"
}

echo "Politika uygulanıyor:"
yaz "$DIZIN/com.google.Chrome"  "Google Chrome (parola kasası kapalı + eklenti zorunlu)" evet
yaz "$DIZIN/com.microsoft.Edge" "Microsoft Edge (parola kasası kapalı)"                  hayir
yaz "$DIZIN/com.brave.Browser"  "Brave (parola kasası kapalı)"                           hayir

# macOS yönetilen tercihleri ÖNBELLEKLER; cfprefsd yeniden başlatılmazsa
# politika saatler sonra devreye girebilir ya da hiç girmez.
killall cfprefsd 2>/dev/null || true

echo
echo "Bitti. Chrome'u TAMAMEN kapatıp (Cmd+Q) yeniden açın."
echo "Doğrulama: chrome://policy → PasswordManagerEnabled=false ve"
echo "           ExtensionInstallForcelist içinde ${EKLENTI_ID} görünmeli."
