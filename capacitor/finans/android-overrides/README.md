# Android overrides — Bogahost Finans

`npx cap add android` platform klasörünü ürettikten SONRA, CI bu dosyaları
üretilen `android/` ağacına kopyalar:

| Kaynak (bu klasör) | Hedef (üretilen android/) |
| --- | --- |
| `AndroidManifest.xml` | `android/app/src/main/AndroidManifest.xml` |
| `res/xml/network_security_config.xml` | `android/app/src/main/res/xml/network_security_config.xml` |
| `res/mipmap-anydpi-v26/*.xml` | `android/app/src/main/res/mipmap-anydpi-v26/` |
| `res/values/colors.xml` | `android/app/src/main/res/values/colors.xml` (birleştir) |
| `assetlinks.json` | CANLI siteye: `https://finans.bogahost.com/.well-known/assetlinks.json` |

Adaptive icon PNG'leri (`ic_launcher_foreground` vb.) `npm run icons`
(scripts/gen-icons.mjs) ile `../resources/android/` altına üretilir.

## MANUEL adımlar
1. **İmzalama**: release keystore CI secret'ı olarak sağlanır. `gradlew ... assembleRelease bundleRelease` imzalı AAB/APK üretir.
2. **assetlinks.json**: `REPLACE_WITH_RELEASE_SIGNING_SHA256_FINGERPRINT`
   yerine release imzasının SHA256 parmak izini yaz
   (`keytool -list -v -keystore release.keystore` çıktısından) ve dosyayı
   `finans.bogahost.com/.well-known/assetlinks.json` altında `application/json`
   olarak yayınla. App Link `autoVerify` bunu doğrular.
3. **Native push (FCM)**: `google-services.json` dosyasını `android/app/` altına
   ELLE ekle (Firebase konsolundan). Ayrıntı: `../../README.md`.
