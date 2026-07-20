## 1.2.1

### Düzeltildi
- **macOS: pencere kapatılınca Dock'ta kalıyor ama tıklayınca açılmıyordu.** Uygulama
  kapatmada tepsiye gizleniyor (doğru davranış), ancak macOS'un Dock tıklamasında
  gönderdiği `Reopen` olayı hiç işlenmiyordu; bu yüzden pencere bir daha geri gelmiyordu.
  Artık Dock ikonuna tıklayınca pencere gösterilip öne getiriliyor.

# Değişiklik Günlüğü

Bu depo Bogahost'un 4 sisteminin (Finans / DCIM / Chat / Görevler) native
kabuklarını içerir: Capacitor 7 (Android/iOS) + Tauri 2 (Windows/macOS).
Sürüm numarası 4 uygulamada ve tüm platformlarda ortaktır.

## [1.2.0] — 2026-07-20

Kullanıcı geri bildirimi üzerine masaüstü (Tauri) kabuklarına odaklanan sürüm:
**indirmeler çalışmıyordu, uygulama donuyordu ve "uygulama gibi" davranmıyordu.**
Dört uygulamada da (Finans / DCIM / Chat / Görevler) `lib.rs` birebir aynıdır.

### Eklendi — İndirme desteği (en kritik)
- **Panelden üretilen PDF/CSV dosyaları artık gerçekten iniyor.** Ana pencere
  `WebviewWindowBuilder` ile kuruluyor ve `on_download` olayına bağlanıyor:
  dosya **İndirilenler** klasörüne yazılır, ad çakışmasında `-1`, `-2` … eklenir.
  (Rapor PDF/CSV, teklif PDF, hediye listesi CSV, Paraşüt fatura PDF vb.)
- **İndirme bitince native bildirim:** "İndirildi: `<dosya>`". Tepsi menüsüne
  **"İndirilenler klasörünü aç"** ve **"Son indirilen dosyayı göster"** eklendi
  (dosyanın bulunduğu klasör Finder/Gezgin'de açılır).
- **`blob:` / `data:` indirmeleri** (tarayıcıda üretilen dosyalar) için sayfaya
  enjekte edilen köprü: dosya `bogahost_save_file` komutuyla diske yazılır.
  Köprü çalışmazsa WebView'in kendi indirme akışına düşülür.
- **WebView'de gösterilemeyen türler** (ör. `window.open` ile açılan PDF)
  kaydedilip **sistem uygulamasında** açılır.
- **`target="_blank"` linkleri artık sessizce yutulmuyor:** aynı pencerede açılır;
  adres kendi alan adımız (`*.bogahost.com`) dışındaysa **sistem tarayıcısına**
  yönlendirilir (`on_navigation`).

### Düzeltildi — Donma / takılma
- **Açılışta hiçbir ağ çağrısı `setup()`'ı bloklamıyor.** Sürüm denetimi 5 sn
  gecikmeyle, bildirim izni 2 sn gecikmeyle arka planda çalışır.
- **Ağ işlemlerine 7 sn zaman aşımı** (updater + yedek manifest); başarısızsa sessizce geçilir.
- **Bildirimler ana thread'e kuyruklanıyor** (`run_on_main_thread`) — macOS
  bildirim API'sinin arka thread'den çağrılması kaynaklı takılmalar giderildi.
- **Beyaz/donuk açılış karesi yok:** pencere gizli başlar, sayfa yüklenince
  gösterilir; sayfa hiç yüklenmezse en geç 8 sn sonra yine gösterilir.
- Menü/tepsi işleyicilerinde uzun süren iş yapılmaz.

### Eklendi — "Uygulama gibi" davranış
- **Standart macOS menüleri artık açıkça kuruluyor** (kaybolma riski yok):
  Uygulama (Hakkında, Hizmetler, Gizle **Cmd+H**, Çıkış **Cmd+Q**),
  **Düzen** (Geri Al/Yinele, Kes **Cmd+X**, Kopyala **Cmd+C**, Yapıştır **Cmd+V**,
  Tümünü Seç **Cmd+A**), **Pencere** (Küçült **Cmd+M**, Kapat **Cmd+W**).
- **Yeni "Görünüm" menüsü:** Yenile **Cmd+R**, Geri **Cmd+[**, İleri **Cmd+]**,
  Yakınlaştır **Cmd+=**, Uzaklaştır **Cmd+-**, Gerçek Boyut **Cmd+0**, Tam Ekran.
  Aynı öğeler Windows için tepsi menüsüne de eklendi.
- **Pencere boyutu/konumu hatırlanıyor** (uygulama yapılandırma klasöründe
  `window-state.json`).
- Klavye ile yakınlaştırma (`zoom_hotkeys_enabled`) açık.

### Eklendi — Bildirim izni
- Tepsi menüsünde **"Bildirimler: açık / kapalı"** göstergesi; kapalıyken
  tıklandığında **sistem bildirim ayarları** açılır (macOS/Windows).
- İlk açılıştaki tek seferlik tanıtım bildirimi (izin penceresini tetikler) korundu,
  artık arka planda ve bloklamadan çalışır.

### Değişti
- Ana pencere tanımı `tauri.conf.json > app.windows` yerine **Rust tarafında**
  (`build_main_window`) oluşturuluyor — `on_download` / `on_navigation` /
  `on_page_load` yalnızca bu yolla bağlanabiliyor.
- Uygulamalar arası geçişte **"Yükleniyor…" katmanı** gösteriliyor; geçiş sonrası
  pencere başlığı ve menü işaretleri güncelleniyor (mevcut davranış korundu).
- Tüm sürüm alanları **1.2.0**.

### Korundu
- Tepsi ikonu ve kapatınca tepsiye gizlenme, otomatik güncelleme akışı
  (imzasızken çökmeme dahil), `remote.json` ile yalnızca bildirim izni,
  eski manifest yolu (`https://bogahost.com/native/latest.json`), CI yapısı.

---

## [Yayınlanmamış]

### Eklendi — Tam otomatik güncelleme (masaüstü)
- **`tauri-plugin-updater` 4 uygulamada da devrede.** Açılışta sessiz denetim →
  güncelleme varsa **onay diyaloğu** ("Yeni sürüm X hazır. Şimdi kurulsun mu?") →
  onaylanırsa **indir + imza doğrula + kur + yeniden başlat**. Tepsideki
  "Güncellemeleri denetle" aynı akışı elle tetikler. "Daha sonra" denirse kalıcı
  kayıt tutulmaz; bir sonraki açılışta tekrar sorulur.
- **Endpoint şeması:** `https://native.bogahost.com/updates/<app>/{{target}}/{{arch}}/latest.json`
  (yedek: `.../updates/<app>/latest.json`). Per-target/arch dosyaları birincildir —
  `windows.yml` ve `macos.yml` ayrı koştuğu için ortak dosyada çakışma olmaz.
- **`tauri-plugin-dialog`** onay diyaloğu için eklendi.
- **Dayanıklılık:** Updater eklentisi `setup()` içinde kayıt edilir; `pubkey`
  PLACEHOLDER/bozuksa eklenti yüklenmez, hata yutulur ve uygulama **eski manifest
  denetimine** düşer. `https://bogahost.com/native/latest.json` yolu (v1.1.0
  istemcileri için) **korunmuştur**. Hiçbir hata uygulamayı kilitlemez/çökertmez.

### Eklendi — CI: imzalama + yayınlama
- **`.github/workflows/generate-updater-key.yml`** (yalnız `workflow_dispatch`):
  minisign anahtar çifti üretir; **public key** iş özetine yazılır, **private key**
  yalnızca artifact olarak çıkar — log'a asla yazılmaz. Depoya anahtar konmaz.
- **`scripts/ci/apply-updater-config.mjs`:** build öncesi `pubkey` enjeksiyonu +
  `createUpdaterArtifacts`. Secret yoksa dosyaya DOKUNMAZ → imzasız build yeşil kalır.
- **`scripts/ci/publish-updates.mjs`:** installer + `.sig` + `latest.json` +
  `index.html` üretir; sunucudaki kopyayı okuyup birleştirir (okunamazsa üzerine
  yazmaz). Android APK `downloads/`'a yüklenir (otomatik güncelleme yok).
- **FTPS yayın** (`lftp`, `ftp:ssl-force true`) — `DEPLOY_FTP_HOST/USER/PASS`.
  Secret yoksa adım ATLANIR (uyarı verir, CI kırmızıya dönmez).
- **Boş env tuzağına karşı:** `TAURI_SIGNING_*` env'leri yalnızca DOLUYSA export
  edilir (macOS'taki "failed to import keychain certificate" ile aynı sınıf hata).
- **Authenticode + updater çakışması çözüldü:** Authenticode installer baytlarını
  değiştirdiği için `.sig` sonrasında `tauri signer sign` ile yeniden üretilir.

### Değişti
- `docs/UPDATE.md` tam otomatik akışa göre **yeniden yazıldı** (mimari, şema,
  secret tablosu, anahtar üretimi, **Cloudflare grey-cloud zorunluluğu**, sorun giderme).
- `docs/latest.json.example` Tauri updater şemasına güncellendi;
  `docs/latest-legacy.json.example` eski şema için eklendi.

### Gerekli manuel adımlar
- Anahtar üretme workflow'unu bir kez çalıştırıp `TAURI_SIGNING_*` secret'larını ekleyin.
- `public_html/native` köküne işaret eden bir FTP hesabı açıp `DEPLOY_FTP_*` ekleyin.
- `native.bogahost.com` için Cloudflare'i **DNS-only (gri bulut)** yapın veya bypass
  kuralı tanımlayın — aksi hâlde updater 403 alır.

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
