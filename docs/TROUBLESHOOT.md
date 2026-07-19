# Sorun Giderme (TROUBLESHOOT)

## Android

**"App not installed" / imzasız APK yüklenmiyor**
- İmzasız (CI'da secret yokken üretilen) APK bazı cihazlara kurulmaz. İmzalı APK üretin
  ([SIGNING.md](SIGNING.md)) veya cihazda "bilinmeyen kaynaklardan yükleme"yi açın.
- Aynı `applicationId` farklı imzayla zaten kuruluysa: önce eskisini kaldırın (imza çakışması).

**Gradle `assembleRelease` başarısız — signingConfig yok**
- `keystore.properties` yoksa build.gradle release'i imzasız üretmeli. Secret'ları tanımlayın ya da
  Capacitor Android projesindeki `signingConfigs` bloğunun dosya yoksa unsigned'a düşmesini sağlayın.

**`cap sync android` "webDir not found"**
- Kabuk canlı URL yükler; yine de bir `webDir` (varsayılan `www`) gerekir. CI bunu `mkdir -p www`
  ile garanti eder; yerelde `capacitor/<key>/www` klasörünün var olduğundan emin olun.

## iOS

**Provisioning / imzalama hataları (`No profiles for '...' were found`)**
- `APPLE_CERT`, `PROVISIONING_PROFILE`, `TEAM_ID` secret'ları eksikse CI zaten imzasız doğrulamaya
  düşer (`CODE_SIGNING_ALLOWED=NO`). IPA için üçünü de tanımlayın.
- Provisioning profile'ın App ID'si (`com.bogahost.<key>`) ile kabuğun bundle identifier'ı bire bir
  eşleşmeli. Her uygulama ayrı profile ister.
- Sertifika süresi dolmuş olabilir — Apple Developer'da yenileyin.

**`pod install` hataları**
- CocoaPods güncel değilse: `sudo gem install cocoapods` (yerel). CI'da `macos-latest` ile gelir.

## Windows (Tauri)

**Uygulama açılmıyor — WebView2 Runtime yok**
- Tauri, Microsoft **WebView2 Runtime**'a ihtiyaç duyar. Windows 11'de yerleşiktir; Windows 10'da
  Evergreen Runtime kurulmalı ya da installer'a "embed bootstrapper" seçeneğiyle paketlenmeli.

**SmartScreen "Bilinmeyen yayımcı" uyarısı**
- İmzasız binary'de normaldir. Authenticode ile imzalayın ([SIGNING.md](SIGNING.md)); ilk itibar
  birikene kadar EV sertifika uyarıyı anında kaldırır.

**`tauri build` — Rust/MSVC bulunamadı**
- Rust stable + "Desktop development with C++" (MSVC build tools) gerekir. CI'da
  `dtolnay/rust-toolchain@stable` sağlar; yerelde `rustup` + Visual Studio Build Tools kurun.

## macOS (Tauri)

**"...doğrulanamadı / kötü amaçlı olabilir" (Gatekeeper)**
- İmzasız/notarize edilmemiş DMG'de olur. Developer ID imza + notarization ekleyin
  ([SIGNING.md](SIGNING.md)). Dahili testte geçici çözüm: sağ tık → **Aç**.

**Notarization takılıyor / reddediliyor**
- `APPLE_ID` / `APPLE_PASSWORD` (app-specific) / `APPLE_TEAM_ID` üçlüsü ve geçerli **Developer ID
  Application** imzası şart. `xcrun notarytool log` ile ret nedenini okuyun (hardened runtime,
  imzasız binary vb.).

## Ortak / WebView

**Sayfa açılmıyor, beyaz ekran**
- Kabuk canlı URL'yi yükler; cihaz internetsizse veya canlı site erişilemezse boş görünür.
  `apps.config.json`'daki URL'yi ve site erişilebilirliğini kontrol edin.

**Cloudflare challenge / server-to-server engeli**
- Bogahost'ta CF, sunucu-sunucu isteklerini challenge'layabilir (auto-memory `bogahost-cf-blocks-bridge`).
  Kabuk normal bir tarayıcı gibi davransa da köprü/SSO çağrıları etkileniyorsa origin-pin gerekebilir.

**CSP / WebSocket bağlanmıyor (Chat sesli/görüntülü, SSE)**
- Canlı sitenin CSP `connect-src`/`media-src` başlıkları WebView origin'ine izin vermeli. Bu backend
  tarafında ayarlanır (bu depo dokunmaz). Chat WebRTC/TURN için `bogahost-livechat-turn` notuna bakın.

**Bildirim gelmiyor**
- WebView içindeki web-push arka planda güvenilmez. Bkz. [PUSH.md](PUSH.md) — PWA kurulumu veya
  native FCM/APNs.

## CI

**Workflow imzalama adımını atladı**
- Beklenen davranış: ilgili secret yoksa imzalama SKIP edilir, `::warning` bırakılır, imzasız
  artifact yine yüklenir. Job FAIL etmez. Secret adları için [SIGNING.md](SIGNING.md).

**`release.yml` Release oluşturmadı**
- Sadece `v*` tag push'unda çalışır ve **taslak (draft)** Release üretir — Releases sekmesinden
  yayınlayın. `workflow_dispatch` ile elle çalıştırmada tag bağlamı yoksa gh-release adımı dosya
  bulamayabilir.

**Artifact boş / `if-no-files-found: warn`**
- İlgili platform projesi (`capacitor/<key>` veya `tauri/<key>`) henüz oluşturulmamışsa build çıktısı
  üretilmez. Kabuk projeleri hazır olduğunda artifact dolar.
