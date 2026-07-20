# Bogahost Native

Bogahost'un 4 iç yönetim sisteminin **native kabukları**:

| Anahtar | Uygulama | Canlı URL | App ID |
|---------|----------|-----------|--------|
| `finans` | Bogahost Finans | https://finans.bogahost.com/admin | `com.bogahost.finans` |
| `dcim` | Bogahost DCIM | https://dcim.bogahost.com/admin | `com.bogahost.dcim` |
| `chat` | Bogahost Chat | https://chat.bogahost.com/admin | `com.bogahost.chat` |
| `task` | Bogahost Görevler | https://task.bogahost.com/admin | `com.bogahost.task` |

## Mimari

Her uygulama, **canlı PWA URL'sini yükleyen bir WebView kabuğudur**. Uygulama içinde HTML/CSS/JS
bulunmaz; kabuk yalnızca yukarıdaki canlı adresi açar.

- **Android + iOS** → [Capacitor 7](https://capacitorjs.com/) (`capacitor/<key>/`)
- **Windows + macOS** → [Tauri 2](https://tauri.app/) (`tauri/<key>/`)

```
bogahost-native/
├── apps.config.json         # 4 uygulamanın tek doğruluk kaynağı (URL, appId, renk, ikon)
├── package.json             # kök npm komutları (aşağıdaki tablo)
├── scripts/*.mjs            # dev / build / icons / doctor / clean orkestratörleri
├── assets/icons/            # her uygulama için kaynak ikon (icon-source-<key>.png)
├── capacitor/<key>/         # Android + iOS projesi (CI'da regenerate edilir)
├── tauri/<key>/             # Windows + macOS projesi
├── .github/workflows/       # CI: android / ios / windows / macos / pwa-lighthouse / release
└── docs/                    # bu klasör
```

> **Önemli:** Bu depo **yalnızca kabukları** üretir. Backend/frontend uygulama koduna
> (`/home/finansboga`, `/home/dcimboga`, `/home/bogahost`, `/home/taskboga`) **dokunmaz**.
> Uygulama davranışı canlı siteden gelir.

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

GitHub Actions tüm binary'leri üretir — bkz. [`docs/BUILD.md`](docs/BUILD.md).
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
**"Uygulamalar"** alt menüsü vardır: *Finans · DCIM · Chat · Görevler*. Seçilen
uygulama **mevcut pencerede** açılır. Mobilde (Capacitor) ayrı menü yoktur; dört
host da `server.allowNavigation` içinde olduğu için geçiş aynı kabukta gerçekleşir.
Ayrıntı: [`tauri/README.md`](tauri/README.md) ve [`capacitor/README.md`](capacitor/README.md).
