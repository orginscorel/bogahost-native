# Android overrides — Bogahost Chat

`npx cap add android` SONRASI CI bu dosyaları üretilen `android/` ağacına kopyalar:

| Kaynak | Hedef |
| --- | --- |
| `AndroidManifest.xml` | `android/app/src/main/AndroidManifest.xml` |
| `res/xml/network_security_config.xml` | `android/app/src/main/res/xml/` |
| `res/mipmap-anydpi-v26/*.xml` | `android/app/src/main/res/mipmap-anydpi-v26/` |
| `res/values/colors.xml` | `android/app/src/main/res/values/colors.xml` (birleştir) |
| `assetlinks.json` | `https://chat.bogahost.com/.well-known/assetlinks.json` |

Adaptive icon PNG'leri `npm run icons` ile `../resources/android/` altına üretilir.

## WebRTC / getUserMedia (ÖNEMLİ)
Manifest'te `CAMERA` + `RECORD_AUDIO` + `MODIFY_AUDIO_SETTINGS` verildi. Ancak
Android WebView'de `getUserMedia`, WebView'in `WebChromeClient.onPermissionRequest`
çağrısını **grant** etmesini gerektirir. Capacitor 7 köprüsü çoğu durumda bunu
runtime izinleri verildiğinde otomatik ele alır; alamazsa küçük bir yerel köprü
(BridgeActivity alt sınıfı + `onPermissionRequest` → `request.grant(...)`) CI
şablonuna eklenmelidir. Bu, kod DEĞİL yapılandırma sınırında kaldığı için burada
belge olarak bırakıldı; "otomatik çalışıyor" garantisi verilmez.

## MANUEL adımlar
1. **İmzalama**: release keystore CI secret'ı.
2. **assetlinks.json**: SHA256 parmak izini yaz ve
   `chat.bogahost.com/.well-known/assetlinks.json` altında yayınla.
3. **Native push (FCM)**: `google-services.json` → `android/app/`. Bkz. `../../README.md`.
