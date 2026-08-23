#!/bin/bash
# Bogahost Kasa — tarayıcı parola kasasını KAPAT (macOS).
#
# NEDEN GEREKLİ: Kasa uygulaması kimlik bilgisini hedef pencereye yazar. Yazma
# bittiğinde tarayıcı kendi "Şifreyi kaydedeyim mi?" balonunu gösterebilir ve
# kabul edilirse parola tarayıcının kasasına, oradan da kullanıcının Google/
# Microsoft hesabına ve tüm cihazlarına eşitlenir. Bunu uygulama tarafından
# engellemek MÜMKÜN DEĞİL — karar tarayıcının kendisine ait.
#
# Engellemenin desteklenen tek yolu YÖNETİLEN POLİTİKADIR. Aşağıdaki ayarlar
# /Library/Managed Preferences altına yazılır; kullanıcı tarayıcı ayarlarından
# GERİ AÇAMAZ.
#
# Kullanım:  sudo bash macos-chrome-edge.sh
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  echo "Bu betik yönetici hakkı ister:  sudo bash $0" >&2
  exit 1
fi

DIZIN="/Library/Managed Preferences"
mkdir -p "$DIZIN"

yaz() {
  local plist="$1" ad="$2"
  # PasswordManagerEnabled=false → "Şifreyi kaydet" balonu HİÇ çıkmaz.
  # AutofillAddressEnabled / AutofillCreditCardEnabled → diğer otomatik
  #   doldurma depoları da kapanır; amaç hiçbir şeyin saklanmaması.
  defaults write "$plist" PasswordManagerEnabled -bool false
  defaults write "$plist" AutofillAddressEnabled -bool false
  defaults write "$plist" AutofillCreditCardEnabled -bool false
  chmod 644 "$plist.plist" 2>/dev/null || true
  echo "  ✓ $ad"
}

echo "Tarayıcı parola kasaları kapatılıyor:"
yaz "$DIZIN/com.google.Chrome"    "Google Chrome"
yaz "$DIZIN/com.microsoft.Edge"   "Microsoft Edge"
yaz "$DIZIN/com.brave.Browser"    "Brave"

echo
echo "Bitti. Açık tarayıcıları TAMAMEN kapatıp yeniden açın."
echo "Doğrulama: chrome://policy adresinde PasswordManagerEnabled = false görünmeli."
