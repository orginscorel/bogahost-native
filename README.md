# Bogahost Native

Bogahost'un iç sistemlerinin **native kabukları** ve bir tam native uygulama.

| Anahtar | Uygulama | Tür | Canlı URL | App ID |
|---------|----------|-----|-----------|--------|
| `finans` | Bogahost Finans | Kabuk | https://finans.bogahost.com/admin | `com.bogahost.finans` |
| `dcim` | Bogahost DCIM | Kabuk | https://dcim.bogahost.com/admin | `com.bogahost.dcim` |
| `chat` | Bogahost Chat | Kabuk | https://chat.bogahost.com/admin | `com.bogahost.chat` |
| `task` | Bogahost Görevler | Kabuk | https://task.bogahost.com/admin | `com.bogahost.task` |
| `muh` | Bogahost Muhasebe Arşivi | Kabuk | https://muh.bogahost.com/admin | `com.bogahost.muh` |
| `kasa` | Bogahost Kasa | **Yerel** | — (arayüzü depoda) | `com.bogahost.kasa` |

## Mimari

**Beş kabuk** (`finans` · `dcim` · `chat` · `task` · `muh`), canlı PWA URL'sini yükleyen
WebView'lerdir. İçlerinde HTML/CSS/JS bulunmaz; kabuk yalnızca yukarıdaki canlı adresi açar.
Uygulama davranışı canlı siteden gelir.

- **Android + iOS** → [Capacitor 7](https://capacitorjs.com/) (`capacitor/<key>/`)
- **Windows + macOS** → [Tauri 2](https://tauri.app/) (`tauri/<key>/`)

**Kasa bunlardan farklıdır ve karıştırılmamalıdır.** Bir kabuk değil, arayüzü depoda duran
(`tauri/kasa/dist/`) gerçek bir masaüstü uygulamasıdır. İşini işletim sistemi düzeyinde yapar:
küresel kısayol, öndeki pencereyi okuma, başka bir programa tuş gönderme, `127.0.0.1` üzerinde
tarayıcı eklentisiyle konuşan yerel bir köprü ve zero-knowledge şifreleme (Rust). Bu yüzden
`apps.config.json` içinde `url` alanı yoktur, `yerel: true` taşır ve **yalnızca Windows +
macOS** için üretilir — mobil karşılığı yoktur (`capacitor/kasa/` yoktur).

```
bogahost-native/
├── apps.config.json         # 6 uygulamanın tek doğruluk kaynağı (URL, appId, renk, ikon)
├── package.json             # kök npm komutları (aşağıdaki tablo)
├── scripts/*.mjs            # dev / build / icons / doctor / clean orkestratörleri
├── scripts/ci/              # sürüm denetimi, güncelleyici manifesti, yayın ağacı
├── assets/icons/            # her uygulama için kaynak ikon (icon-source-<key>.png)
├── capacitor/<key>/         # Android + iOS projesi (CI'da regenerate edilir) — kasa YOK
├── tauri/<key>/             # Windows + macOS projesi
│   └── kasa/                #   ↳ tek gerçek uygulama: dist/ (arayüz) + src-tauri/src/ (Rust)
├── eklenti/                 # Kasa'nın Chrome eklentisi (MV3) — aşağıya bakın
├── .github/workflows/       # CI: android / ios / windows / macos / pwa-lighthouse / release
└── docs/                    # yapı, imzalama, yayın, güncelleme, sorun giderme
```

> **Önemli:** Bu depo kabukları ve Kasa'yı üretir. Backend/frontend uygulama koduna
> (`/home/finansboga`, `/home/dcimboga`, `/home/bogahost`, `/home/taskboga`, `/home/muhboga`)
> **dokunmaz**.

## Bogahost Kasa

Kurumsal parola kasası. Üç parçadan oluşur ve üçü birlikte çalışır:

| Parça | Yer | Görevi |
|-------|-----|--------|
| Masaüstü uygulaması | `tauri/kasa/` | Arayüz, şifre çözme, hedef pencereye yazma, yerel köprü |
| Chrome eklentisi | `eklenti/` | Sayfayı **görerek** doldurur (uygulama sayfanın içini göremez) |
| Sunucu | DCIM (`dcim.bogahost.com`) | Kayıtlar, kasalar, roller, eşleşme kararı |

**Eklentinin kendi girişi yoktur.** Kimliğini aynı bilgisayardaki Kasa uygulamasından alır:
uygulama `127.0.0.1:17321-17330` arasında küçük bir köprü açar (`src-tauri/src/kopru.rs`),
eklenti onu bulur. Ayrıntı: [`eklenti/README.md`](eklenti/README.md).

**Eşleşme kararının tek sahibi sunucudur.** Hangi kaydın hangi sayfada/programda önerileceğini
`VaultItem::sayfayaUyar()` ve `VaultItem::uygulamayaUyar()` belirler; istemci ikinci bir kural
işletmez. İki tarafın ayrı kural hesaplaması sessiz açık üreten şeydi. Kayıt bazında kural:

- `alan` (varsayılan) — ana alan ve alt alanları
- `host` — yalnız o adres, alt alanlar hariç
- `yol` — adres **ve** yolu; ana alanı hem halka açık site hem yönetim paneli olan kurumlarda şart
- `kapali` — otomatik doldurmada hiç çıkmaz
- `uygulama` — masaüstü program desenleri (`winbox, winbox64`); kullanıcı elle doldurduğunda
  bağ kendiliğinden öğrenilir

Rust tarafı bu Linux sunucusunda **derlenemez**. Saf mantık (yol/eşleşme/kripto) bağımsız bir
cargo kasasına çıkarılıp orada test edilir; tam derleme CI'da yapılır.

## Hızlı başlangıç

```bash
# Node 20+ gerekli
npm ci              # bağımlılıklar
npm run doctor      # ortam/SDK kontrolü
npm run icons       # apps.config.json + assets/icons'tan tüm platform ikonlarını üret
npm run build:all   # tüm platformlar (yerelde kurulu SDK'lar gerektirir)
```

## `npm run` komutları

| Komut | Açıklama |
|-------|----------|
| `npm run dev` | Geliştirme kabuğunu başlatır (`scripts/dev.mjs`) |
| `npm run build` | PWA/kabuk hazırlığı (`--pwa`) |
| `npm run build:android` | Android APK/AAB (Capacitor) |
| `npm run build:ios` | iOS archive/IPA (Capacitor, macOS gerekir) |
| `npm run build:windows` | Windows MSI/EXE (Tauri) |
| `npm run build:mac` | macOS DMG/App (Tauri, macOS gerekir) |
| `npm run build:all` | Tüm platformlar |
| `npm run icons` | Kaynak ikonlardan platform ikonları üretir (sharp) |
| `npm run doctor` | Gerekli araç/SDK varlığını denetler |
| `npm run clean` | Üretilen çıktıları temizler |

## CI / dağıtım

Varsayılan dal **`native-apps`** (`main` değil). İş akışları bu dala push ile ve `v*`
etiketleriyle çalışır.

| İş akışı | Dal push'unda | Notu |
|----------|---------------|------|
| Windows (MSI + EXE) | ✅ çalışır | |
| Android (APK + AAB) | ✅ çalışır | `kasa` üretmez — mobil karşılığı yok |
| macOS (DMG + App) | ❌ **çalışmaz** | Runner dakikası pahalı; elle tetiklenir |
| iOS | ❌ çalışmaz | Elle tetiklenir |

macOS derlemesi için `workflow_dispatch` + `app` girdisi kullanılır:

```bash
gh workflow run macos.yml --ref native-apps -f app=kasa
```

`DEPLOY_FTP_*` secret'ları varsa çıktı `native.bogahost.com` üzerine yayınlanır ve güncelleyici
manifesti (`updates/<app>/latest.json`) tazelenir; uygulamalar güncellemeyi oradan alır.
Derleme adımlarının tamamı: [`docs/BUILD.md`](docs/BUILD.md).

Bu Linux sunucusunda **yalnızca Android derlenebilir**; iOS/macOS macOS+Xcode, Windows ise
Windows+Rust gerektirir. İmzalama ve mağaza dağıtımı için:

- [`docs/SIGNING.md`](docs/SIGNING.md) — imzalama + GitHub secret'ları
- [`docs/PUBLISH.md`](docs/PUBLISH.md) — Play / App Store / Microsoft Store / macOS
- [`docs/PWA.md`](docs/PWA.md) — mağazasız PWA kurulumu
- [`docs/PUSH.md`](docs/PUSH.md) — bildirimler (web-push vs. native FCM/APNs)
- [`docs/UPDATE.md`](docs/UPDATE.md) — masaüstü **tam otomatik güncelleme** (imzalama, yayın, Cloudflare)
- [`docs/WEB-YETENEKLERI.md`](docs/WEB-YETENEKLERI.md) — web yeteneklerinin native kabuktaki durumu (kamera/mikrofon, ekran paylaşımı, sürükle-bırak…)
- [`docs/TROUBLESHOOT.md`](docs/TROUBLESHOOT.md) — yaygın hatalar

Sürüm geçmişi: [`CHANGELOG.md`](CHANGELOG.md).

## Uygulamalar arası geçiş

Masaüstünde (Tauri) sistem tepsisi menüsünde ve macOS menü çubuğunda
**"Uygulamalar"** alt menüsü vardır: *Finans · DCIM · Chat · Görevler · Muhasebe*
(`tauri/<key>/src-tauri/src/lib.rs` içindeki `APPS` tablosu). Seçilen uygulama
**mevcut pencerede** açılır. Mobilde (Capacitor) ayrı menü yoktur; beş host da
`server.allowNavigation` içinde olduğu için geçiş aynı kabukta gerçekleşir.
Ayrıntı: [`tauri/README.md`](tauri/README.md) ve [`capacitor/README.md`](capacitor/README.md).

Kasa bu menüde yer almaz ve bir kabuk gibi davranmaz: kendi başına duran, işletim
sistemiyle konuşan ayrı bir uygulamadır.
