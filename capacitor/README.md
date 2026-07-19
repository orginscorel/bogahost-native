# Bogahost Native — Capacitor Kabukları

4 canlı sistemin (Finans / DCIM / Chat / Görevler) **native mobil kabukları**.
Her uygulama, canlı PWA URL'sini bir WebView içinde yükleyen ince bir Capacitor 7
kabuğudur. **Backend/frontend koduna dokunulmaz.**

## Mimari — neden WebView + `server.url`

```
[ Android/iOS uygulaması ]
        │  server.url = https://<app>.bogahost.com/admin
        ▼
[ WKWebView / Android WebView ]  ──HTTPS──►  CANLI PWA (sunucu)
        ▲
        └─ www/index.html  (yalnızca uzak yüklenene kadar / çevrimdışı bootstrap)
```

`capacitor.config.ts` içinde:
- `server.url` → canlı URL (uygulama açılışta doğrudan buraya gider).
- `server.allowNavigation` → yalnızca kendi host'u.
- `server.cleartext: false` + `network_security_config` → sadece HTTPS.
- `webDir: 'www'` → uzak sunucu gelene kadar gösterilen yerel bootstrap.

Böylece yeni özellik = sitede yayınlanır, uygulama otomatik alır; mağaza güncellemesi
yalnızca kabuk/izin/ikon değişince gerekir.

## Uygulamalar

| key    | appId                 | URL                                   | Özel |
| ------ | --------------------- | ------------------------------------- | ---- |
| finans | com.bogahost.finans   | https://finans.bogahost.com/admin     | — |
| dcim   | com.bogahost.dcim     | https://dcim.bogahost.com/admin       | — |
| chat   | com.bogahost.chat     | https://chat.bogahost.com/admin       | WebRTC (kamera/mikrofon) |
| task   | com.bogahost.task     | https://task.bogahost.com/admin       | — |

## Klasör düzeni (her uygulama)

```
capacitor/<key>/
  capacitor.config.ts        Capacitor 7 yapılandırması (server.url = canlı)
  package.json               bağımlılıklar + sync/open/build scriptleri
  www/index.html             yerel bootstrap (yükleniyor/çevrimdışı)
  android-overrides/         cap add android SONRASI kopyalanacak dosyalar
    AndroidManifest.xml      izinler + autoVerify App Link intent-filter
    res/xml/network_security_config.xml
    res/mipmap-anydpi-v26/   adaptive icon XML
    res/values/colors.xml    icon/splash renkleri
    assetlinks.json          Digital Asset Links (SHA256 placeholder)
    README.md
  ios-overrides/             cap add ios SONRASI uygulanacak dosyalar
    Info.plist.partial.xml   kullanım açıklamaları + UIBackgroundModes
    App.entitlements         associated domains + aps-environment
    apple-app-site-association
    README.md
  resources/                 gen-icons.mjs çıktısı (ikon/splash — CI üretir)
  android/ , ios/            CI'da `cap add` ile üretilir (repoda YOK)
```

## Derleme (CI'da)

Bu üretim web sunucusunda toolchain **kurulu değil**; derleme GitHub Actions'ta yapılır.
Her platform runner'ında akış:

```bash
npm ci                                   # kök araçlar (@capacitor/cli, sharp)
npm run icons                            # sharp ile ikon/splash üret (resources/)
# CI: her app için  npm --prefix capacitor/<key> ci   (kabuk bağımlılıkları — workspace yok)
# CI: her app için  npx cap add android / ios          (platform klasörünü üretir)
# CI: android-overrides/ ve ios-overrides/ dosyalarını üretilen ağaca kopyalar
node scripts/build.mjs --platform android --app all
node scripts/build.mjs --platform ios --app all
```

Kök `package.json` script'leri:

| Komut | Ne yapar |
| --- | --- |
| `npm run build:android` | Tüm app'ler için `cap sync android` + `gradlew assembleRelease bundleRelease` (imzasız/CI-imzalı APK+AAB) |
| `npm run build:ios` | Tüm app'ler için `cap sync ios` + `xcodebuild archive` (imzasız arşiv; imzalama ayrı adım) |
| `npm run build:windows` | `tauri/<key>` projesine dispatch (`npm run tauri:build`) — Tauri ayrı ajan |
| `npm run build:mac` | `tauri/<key>` projesine dispatch |
| `npm run build:all` | android+ios+windows+mac tüm hedefler |
| `npm run build` | `--pwa` — PWA zaten canlı; bilgi basar |
| `npm run icons` | Tüm app ikon/splash boyutları (sharp) |
| `npm run doctor` | Ortam/toolchain VAR-YOK raporu |
| `npm run clean` | Üretilen platform/build klasörlerini temizler (www korunur) |

Tek app: `node scripts/build.mjs --platform android --app chat`

Per-app doğrudan komutlar:

```bash
npm --prefix capacitor/finans run build:android   # cap sync + gradlew
npm --prefix capacitor/finans run build:ios       # cap sync + xcodebuild
npm --prefix capacitor/finans run open:android    # Android Studio aç
```

---

## MANUEL adımlar (kod DIŞI, insan yapar)

### 1. İmzalama
- **Android**: release `keystore` → CI secret. `assembleRelease/bundleRelease` imzalı
  APK/AAB üretir. Keystore repoya **konmaz** (.gitignore korur).
- **iOS**: Apple dağıtım sertifikası + provisioning profile → CI secret.
  `xcodebuild archive` burada **imzasız** üretir; imzalı IPA export ayrı adımdır.

### 2. Native Push — FCM + APNs (ÖNEMLİ, gerçekçi durum)
Capacitor WebView içinde **tarayıcı/PWA web-push GENELDE çalışmaz**. Native push için:

- **Android (FCM)**: Firebase projesi aç → `google-services.json` indir →
  `capacitor/<key>/android/app/google-services.json` olarak **ELLE** yerleştir
  (CI secret veya güvenli artifact). `@capacitor/push-notifications` FCM token'ını alır.
- **iOS (APNs)**: Apple Developer'da APNs Auth Key (`.p8`) oluştur → Key ID + Team ID
  ile push sağlayıcına tanımla. `aps-environment` gerçek dağıtımda `production`.

Bu kimlik bilgileri sağlanana kadar native push **kurulmamıştır**; `@capacitor/push-notifications`
yalnızca iskelet olarak eklidir. Mevcut PWA web-push tarayıcı/PWA bağlamında çalışmaya
devam eder — kabuk onu değiştirmez.

> Bridge notu: Sunucu tarafı (WHMCS/Laravel push servisleri) hâlihazırda VAPID web-push
> gönderiyor. Native tokenlar farklı bir kanaldır (FCM/APNs); bunları sunucudaki push
> gönderimine bağlamak ayrı bir entegrasyon adımıdır ve bu depoda **yapılmamıştır**.

### 3. Deep / Universal Links — `.well-known` dosyaları
App Link / Universal Link doğrulaması için canlı siteye iki dosya konmalı:

- **Android**: `https://<host>/.well-known/assetlinks.json`
  → `android-overrides/assetlinks.json` şablonundaki
  `REPLACE_WITH_RELEASE_SIGNING_SHA256_FINGERPRINT` yerine release imzasının SHA256
  parmak izini yaz (`keytool -list -v -keystore <release.keystore>`).
- **iOS**: `https://<host>/.well-known/apple-app-site-association` (uzantısız,
  `Content-Type: application/json`) → `ios-overrides/apple-app-site-association`
  şablonundaki `TEAMID` yerine Apple Team ID.

Bu dosyalar **canlı siteye** konur (bu depo canlı koda dokunmaz) — dağıtımı insan yapar.

### 4. WebRTC (yalnız chat)
- Android: `CAMERA`/`RECORD_AUDIO`/`MODIFY_AUDIO_SETTINGS` manifestte verildi. WebView
  `getUserMedia` izni için `onPermissionRequest` grant'i gerekebilir — bkz.
  `chat/android-overrides/README.md`.
- iOS: `NSCameraUsageDescription` + `NSMicrophoneUsageDescription` mevcut; WKWebView
  iOS 14.3+ üzerinde izni kendisi ister.

---

## Ne CI üretir, ne repoda durur

| Repoda (kaynak) | CI üretir (gitignore) |
| --- | --- |
| capacitor.config.ts, www/, *-overrides/, package.json | `android/`, `ios/` (cap add) |
| apps.config.json, scripts/ | `resources/` (gen-icons) |
| | `*.apk`, `*.aab`, `*.ipa` (imzalı çıktı) |
