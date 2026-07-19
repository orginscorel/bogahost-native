# Bogahost — Tauri 2 Masaüstü Kabukları (Windows + macOS)

Bu klasör, dört Bogahost sistemi için **Tauri v2** masaüstü uygulama kaynaklarını içerir:

| Uygulama | Klasör | Yüklenen canlı URL |
|----------|--------|--------------------|
| Bogahost Finans   | `tauri/finans/` | https://finans.bogahost.com/admin |
| Bogahost DCIM     | `tauri/dcim/`   | https://dcim.bogahost.com/admin |
| Bogahost Chat     | `tauri/chat/`   | https://chat.bogahost.com/admin |
| Bogahost Görevler | `tauri/task/`   | https://task.bogahost.com/admin |

## Mimari

Her uygulama, **canlı URL'yi yükleyen bir WebView kabuğudur**. Backend/frontend
koduna dokunulmaz — masaüstü pencere doğrudan üretimdeki admin panelini açar
(`tauri.conf.json > app.windows[0].url`). Yani "native" katman sadece pencere,
sistem tepsisi, native bildirim ve harici-link yönlendirmesi sağlar; içerik
tamamen sunucudan gelir.

- **Uzak içerik CSP'si:** Sayfa uzak origin'den geldiği için Cloudflare/sunucu
  başlıkları hükmeder. `security.csp` yalnızca yerel/asset protokolüne enjekte
  edilir; yine de `self + https://<host> + wss://<host>` (WebSocket) izinli
  makul bir politika tanımlıdır.
- **Çevrimdışı fallback:** `dist/index.html` küçük bir "Bağlanılıyor…" splash
  sayfasıdır (bundler + manuel çevrimdışı kullanım için). Normalde görünmez.
- **Otomatik güncelleme KAPALI:** `tauri-plugin-updater` dahil edilmedi.
  Dağıtım manueldir (yeni sürüm = yeni installer).

## Native davranış (`src-tauri/src/lib.rs`)

- **Sistem tepsisi (tray):** Göster / Gizle / Çıkış menüsü + tepsi ikonuna sol
  tık ile pencereyi geri getirme.
- **Kapatınca gizle:** Pencere "X" ile kapatılınca uygulama sonlanmaz, tepsiye
  gizlenir (masaüstü app hissi). Gerçek çıkış tepsi menüsünden.
- **Harici linkler:** `tauri-plugin-shell` (`shell:allow-open`) kayıtlı;
  admin panelleri kendi origin'lerinde SPA olduğu için iç gezinme uygulama
  içinde kalır.

## Bildirimler (native vs web-push)

- `tauri-plugin-notification` eklidir. Canlı admin paneli (kendi origin'imiz),
  `capabilities/remote.json` sayesinde `window.__TAURI__.notification` ile
  **native masaüstü bildirimi** tetikleyebilir (`withGlobalTauri: true`).
- **Uyarı — web-push:** Uzak PWA'nın Service Worker tabanlı Web Push aboneliği
  masaüstü WebView'de (WebView2 / WKWebView) **güvenilir çalışmaz**; arka planda
  push teslimi platforma bağlı ve sınırlıdır. Masaüstünde bildirim istiyorsanız
  panel JS'i açıkken yukarıdaki native köprüyü kullanın. Bunu "web-push
  masaüstünde çalışıyor" diye varsaymayın.

## Yetki modeli (Tauri v2 capabilities)

- `capabilities/default.json` — yerel pencere/tepsi/shell izinleri (`main`
  penceresi).
- `capabilities/remote.json` — canlı `*.bogahost.com` origin'ine **yalnızca**
  bildirim izni verir (asgari yüzey).

## Derleme

> Bu depoda derleme YAPILMAZ. Binary'ler GitHub Actions'ta üretilir:
> Windows installer'lar `windows-latest`, macOS installer'lar `macos-latest`
> runner'ında (Tauri çapraz-derleme yapmaz; her hedef kendi OS'unda derlenir).

### 0. İkonları üret (her build'den ÖNCE zorunlu)

İkon binary'leri repoya konmaz; `assets/icons/icon-source-<key>.png`
(512×512) kaynağından üretilir. Her uygulama klasöründe:

```bash
cd tauri/finans   # veya dcim / chat / task
npx tauri icon ../../assets/icons/icon-source-finans.png
```

Bu, `src-tauri/icons/` altına `icon.ico`, `icon.icns`, `32x32.png`,
`128x128.png`, `128x128@2x.png`, `icon.png` üretir. **Atlanırsa** `tauri build`
"icon not found" ile başarısız olur.

### 1. Bağımlılıklar (CI runner'da)

- Node ≥ 20 + `npm install` (yalnız `@tauri-apps/cli`).
- Rust toolchain (stable, MSRV 1.77.2).
- **Windows:** WebView2 (Win11'de hazır), MSVC build tools.
- **macOS:** Xcode Command Line Tools.
- **Linux runner'da derlenemez** (bu proje Windows/macOS hedefler).

### 2. Build komutları (her uygulama klasöründe)

```bash
npm install
npm run tauri:build         # aktif OS için tüm hedefler
npm run tauri:build:win     # Windows: .msi + NSIS .exe   (windows runner)
npm run tauri:build:mac     # macOS: .dmg + .app          (macos runner)
```

### Üretilecek çıktılar

`src-tauri/target/release/bundle/` altında:

- **Windows** (`windows-latest`): `msi/*.msi` (WiX) ve `nsis/*-setup.exe` (NSIS).
- **macOS** (`macos-latest`): `dmg/*.dmg` ve `macos/*.app`.

## MANUEL kalan adımlar — kod imzalama

Kaynaklar imzasız derlenecek şekilde hazırdır. Üretim dağıtımı için:

- **Windows (Authenticode):** Bir OV/EV kod imzalama sertifikası gerekir.
  İmzalamak için `signingIdentity`/`certificateThumbprint` + `signCommand`
  ayarlanır (ör. `signtool`). Sertifika yoksa installer imzasız çıkar ve
  SmartScreen uyarısı verir.
- **macOS (Developer ID + notarization):** Apple Developer hesabı, "Developer
  ID Application" sertifikası ve Apple ile **notarization** gerekir.
  `bundle.macOS.signingIdentity` + entitlements ayarlanır; ardından
  `xcrun notarytool` ile notarize + `xcrun stapler` ile staple yapılır.
  İmzasız `.app`/`.dmg` Gatekeeper tarafından engellenir.

Sertifikalar ve gizli anahtarlar **asla repoya konmaz** (bkz. kökteki
`.gitignore`); CI secret'ları olarak sağlanır.
