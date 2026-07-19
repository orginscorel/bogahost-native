# Değişiklik Günlüğü

Bu depo Bogahost'un 4 sisteminin (Finans / DCIM / Chat / Görevler) native
kabuklarını içerir: Capacitor 7 (Android/iOS) + Tauri 2 (Windows/macOS).
Sürüm numarası 4 uygulamada ve tüm platformlarda ortaktır.

## [1.1.0] — 2026-07-20

### Eklendi — Uygulamalar arası geçiş
- **Masaüstü (Tauri):** Sistem tepsisi menüsüne ve **macOS menü çubuğuna**
  **"Uygulamalar"** alt menüsü eklendi: *Finans · DCIM · Chat · Görevler*.
  Seçilen uygulama **mevcut pencerede** açılır (`WebviewWindow::navigate`) —
  yeni pencere açılmaz. Aktif uygulama menüde **işaretli ve pasif (gri)** görünür
  ve geçişten sonra işaret güncellenir; pencere başlığı da değişir.
  - macOS'ta varsayılan menü (Uygulama/Düzen/Pencere) korunur, alt menü sonuna eklenir.
  - Windows'ta pencere içinde menü çubuğu yoktur; geçiş tepsi menüsünden yapılır.
- **`tauri.conf.json` CSP genişletildi:** `default-src/connect-src/script-src/style-src/
  img-src/font-src/media-src/frame-src` artık **4 host'u da** (`finans|dcim|chat|task
  .bogahost.com`) ve `wss://` karşılıklarını kapsıyor. Aksi hâlde geçiş sonrası sayfa
  kaynakları engellenirdi.
- **Mobil (Capacitor):** `server.allowNavigation` artık 4 host'u da içeriyor; böylece
  aynı kabukta sistemler arası geçiş mümkün. Android `network_security_config.xml`
  tek host yerine `bogahost.com` (alt alan adları dâhil) için tanımlandı.
  Mobilde ayrı bir menü eklenmedi — geçiş, panellerin kendi arayüzünden yapılır.

### Eklendi — Bildirim izni (macOS/Windows)
- Uygulama açılışında bildirim izni **açıkça** isteniyor
  (`permission_state` → gerekiyorsa `request_permission`).
- İzin alındıktan sonra **yalnızca ilk çalıştırmada** "Bildirimler açıldı" test
  bildirimi gösteriliyor (uygulama veri klasöründeki bir bayrak dosyası ile tekrarı engelleniyor).
  macOS'ta sistem onay penceresini tetikleyen şey bu ilk bildirimdir.
- `capabilities/default.json` dosyalarına `notification:default`,
  `notification:allow-notify`, `notification:allow-is-permission-granted`,
  `notification:allow-request-permission` izinleri eklendi
  (uzak origin izinleri `capabilities/remote.json` içinde zaten vardı).

### Eklendi — Güncelleme denetimi
- Masaüstü uygulamaları açılışta **sessizce** `https://bogahost.com/native/latest.json`
  adresini kontrol eder; yeni sürüm varsa native bildirim gösterir.
- Tepsi menüsüne **"Güncellemeleri denetle"** ve **"İndirme sayfasını aç"** eklendi.
- Tam otomatik (indir-kur) updater **bilinçli olarak eklenmedi**: depo private olduğu
  için release asset'leri anonim indirilemez ve imzalama anahtarı gerekir.
  Gerekçe, `latest.json` biçimi, yayın akışı ve ileride tam updater'a geçiş adımları:
  **`docs/UPDATE.md`**.

### Değişti
- Tüm sürüm alanları `1.0.0` → `1.1.0`
  (kök `package.json`, `apps.config.json`, `tauri/*/package.json`,
  `tauri/*/src-tauri/tauri.conf.json`, `tauri/*/src-tauri/Cargo.toml`,
  `capacitor/*/package.json`).
- `apps.config.json` artık bir `version` alanı taşıyor.
- Tauri kabuklarına `reqwest` bağımlılığı eklendi (sürüm denetimi için;
  platformun kendi TLS'ini kullanır, ek sistem bağımlılığı gerektirmez).

### Bilinen sınırlar / dürüst notlar
- macOS uygulaması **imzasız/notarize edilmemiş** dağıtılıyor. Bildirimler çalışır
  ancak ilk açılışta Gatekeeper uyarısı çıkar (sağ tık → Aç, ya da
  Sistem Ayarları → Gizlilik ve Güvenlik → "Yine de Aç").
  İmzasız uygulamalarda macOS bildirim davranışı sürüme göre değişebilir —
  garanti verilmiyor (bkz. `tauri/README.md`).
- Güncelleme bildirimi, `latest.json` canlı siteye **elle** yüklenene kadar çıkmaz.
- Web-push (Service Worker) masaüstü WebView'lerinde güvenilir çalışmaz; masaüstünde
  bildirim için native köprü kullanılır.

## [1.0.0]

- İlk sürüm: 4 uygulama × Android (APK/AAB), iOS (arşiv), Windows (MSI/NSIS),
  macOS (DMG/.app) kabukları; sistem tepsisi, kapatınca gizle, native bildirim
  köprüsü, CI iş akışları (GitHub Actions) ve PWA iyileştirme parçacıkları.
