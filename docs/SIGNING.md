# İmzalama (SIGNING)

İmzalama, mağaza dağıtımı için zorunludur. Bu depodaki CI workflow'ları **secret'lar tanımlıysa
imzalar, değilse imzasız artifact üretir** (job FAIL etmez, sadece `::warning`). Secret'ları
GitHub'da **Settings → Secrets and variables → Actions → New repository secret** altında tanımlayın.

> Sırlar **asla depoya commit edilmez**. `.gitignore` `*.keystore`, `*.jks`, `*.p12`, `*.p8`,
> `*.mobileprovision`, `keystore.properties`, `.env*` kalıplarını zaten dışlar.

## Secret → Workflow eşlemesi

| Platform | Workflow | Secret adları | Yoksa davranış |
|----------|----------|---------------|----------------|
| Android | `android.yml` | `SIGNING_KEYSTORE_BASE64`, `KEY_ALIAS`, `STORE_PASSWORD`, `KEY_PASSWORD` | İmzasız APK/AAB + uyarı |
| iOS | `ios.yml` | `APPLE_CERT`, `APPLE_CERT_PASSWORD`, `PROVISIONING_PROFILE_<APP>`, `TEAM_ID` | İmzasız `.xcarchive` (doğrulama), IPA atlanır |
| Windows | `windows.yml` | `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | İmzasız MSI/EXE + uyarı |
| macOS | `macos.yml` | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | İmzasız DMG (Gatekeeper uyarısı) |

`release.yml`, `secrets: inherit` kullandığından bu secret'ları depo düzeyinde tanımlamanız yeterli.

---

## Android — keystore üretimi

```bash
# 1) Kalıcı bir release keystore üret (parolaları güvenli sakla!)
keytool -genkeypair -v \
  -keystore bogahost-release.keystore \
  -alias bogahost \
  -keyalg RSA -keysize 2048 -validity 10000

# 2) base64'e çevir (tek satır) → SIGNING_KEYSTORE_BASE64 secret'ına yapıştır
base64 -w0 bogahost-release.keystore > keystore.b64   # macOS: base64 -i ... | tr -d '\n'
```

GitHub secret'ları:
- `SIGNING_KEYSTORE_BASE64` = `keystore.b64` içeriği
- `KEY_ALIAS` = `bogahost`
- `STORE_PASSWORD` = keystore parolası
- `KEY_PASSWORD` = anahtar parolası

CI, secret varsa `capacitor/<key>/android/keystore.properties` dosyasını yazar; Gradle bu dosya
varsa release `signingConfig`'ini devreye alacak şekilde kurgulanmalıdır (Capacitor Android projesi).

> **Play App Signing:** İlk yüklemede Google'ın "Play App Signing" özelliği önerilir — siz yükleme
> anahtarıyla imzalarsınız, Google dağıtım anahtarını yönetir. Aynı `bogahost-release.keystore`'u
> yükleme anahtarı olarak kaydedin.

---

## iOS — Apple Developer sertifikası + provisioning

> 📘 **Uçtan uca kılavuz: [`IOS.md`](IOS.md)** — hesap açma, App ID, sertifika,
> profil, TestFlight, push (APNs), universal link ve App Store inceleme notları.
> Aşağısı yalnızca özettir.

**Gerekli:** Apple Developer Program üyeliği (**99 USD/yıl**).

1. **Distribution sertifikası** oluştur (Apple Developer → Certificates → Apple Distribution).
   `.p12` olarak dışa aktar (Keychain Access → export, parola belirle).
2. **App ID** kaydet: her uygulama için `com.bogahost.finans`, `.dcim`, `.chat`, `.task`.
   **Push Notifications + Associated Domains** kutularını işaretle.
   ⚠️ Wildcard App ID (`com.bogahost.*`) **kullanma** — bu iki yetenek wildcard'da çalışmaz.
3. **Provisioning profile** (App Store dağıtımı) — her bundle ID için **ayrı** üret ve indir.
4. base64'e çevir:
   ```bash
   base64 -w0 dist.p12                        # → APPLE_CERT           (tek sertifika, 4'ü için ortak)
   base64 -w0 Bogahost_Finans.mobileprovision # → PROVISIONING_PROFILE_FINANS
   base64 -w0 Bogahost_DCIM.mobileprovision   # → PROVISIONING_PROFILE_DCIM
   base64 -w0 Bogahost_Chat.mobileprovision   # → PROVISIONING_PROFILE_CHAT
   base64 -w0 Bogahost_Task.mobileprovision   # → PROVISIONING_PROFILE_TASK
   ```

GitHub secret'ları: `APPLE_CERT`, `APPLE_CERT_PASSWORD` (.p12 parolası),
`PROVISIONING_PROFILE_FINANS|_DCIM|_CHAT|_TASK`, `TEAM_ID` (10 karakterli takım kimliği).
Uygulamaya özel secret yoksa ortak `PROVISIONING_PROFILE` yedeğe düşer.

İsteğe bağlı (TestFlight'a otomatik yükleme + otomatik imzalama):
`APPSTORE_KEY_ID`, `APPSTORE_ISSUER_ID`, `APPSTORE_PRIVATE_KEY`.

> Not: 5 uygulamanın her biri ayrı App ID + ayrı provisioning profile ister; tek
> Distribution sertifikası hepsi için yeterlidir. ASC API anahtarı verilirse
> otomatik, verilmezse manuel imzalama kullanılır — ikisi de kodludur.

---

## Windows — Authenticode

SmartScreen uyarısını kaldırmak için kod imzalama sertifikası gerekir (OV veya tercihen EV, bir CA'dan).

1. Sertifikayı `.pfx` olarak dışa aktar (özel anahtar dahil, parola belirle).
2. base64:
   ```bash
   base64 -w0 codesign.pfx > pfx.b64   # → WINDOWS_CERTIFICATE
   ```

GitHub secret'ları: `WINDOWS_CERTIFICATE` (base64 PFX), `WINDOWS_CERTIFICATE_PASSWORD`.

CI, `signtool` ile SHA256 + zaman damgası kullanarak MSI ve NSIS EXE'yi imzalar.

---

## macOS — Developer ID + notarization

**Gerekli:** Apple Developer Program (**99 USD/yıl**).

1. **Developer ID Application** sertifikası oluştur → `.p12` dışa aktar.
2. **App-specific password** üret (appleid.apple.com → Güvenlik) → notarization için.
3. base64:
   ```bash
   base64 -i developerID.p12 | tr -d '\n'   # → APPLE_CERTIFICATE
   ```

GitHub secret'ları:
- `APPLE_CERTIFICATE` (base64 .p12)
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY` (örn. `Developer ID Application: Bogahost ... (TEAMID)`)
- `APPLE_ID` (Apple hesabı e-postası)
- `APPLE_PASSWORD` (app-specific password)
- `APPLE_TEAM_ID`

Bu env'ler set edildiğinde **Tauri otomatik imzalar + notarize eder** (`macos.yml` içinde
`tauri build` adımına aktarılır). Notarization için hem imza hem `APPLE_ID`/`APPLE_PASSWORD`/
`APPLE_TEAM_ID` üçlüsü gerekir.

---

## Özet: neyin nesi

- İmzasız artifact = **yalnızca dahili test** (Android "bilinmeyen kaynak", macOS Gatekeeper bypass,
  Windows SmartScreen "yine de çalıştır"). Mağazaya yüklenemez.
- İmzalı + (mac için) notarize'lı artifact = **mağaza / doğrudan dağıtıma hazır**. Bkz. [PUBLISH.md](PUBLISH.md).
