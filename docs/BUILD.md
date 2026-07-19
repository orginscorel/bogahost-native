# Derleme (BUILD)

Her uygulama canlı URL'yi yükleyen bir kabuktur. Derleme = platform binary'sini (APK/AAB, IPA,
MSI/EXE, DMG/App) üretmek. Aşağıda hem **yerel** hem **CI** adımları var.

> **Bu sunucu hakkında:** Bu depo bir Linux (el8) sunucusunda duruyor. Burada **yalnızca Android**
> derlenebilir (JDK + Android SDK kurulursa). **iOS ve macOS macOS + Xcode**, **Windows ise
> Windows + Rust/MSVC** gerektirir; bu sunucuda üretilemezler. Çapraz-platform binary'ler için
> **GitHub Actions** kullanın (`.github/workflows/`). CI runner'ları doğru işletim sistemini sağlar.

## Ortak ön koşullar

| Araç | Sürüm | Not |
|------|-------|-----|
| Node.js | ≥ 20 | `package.json > engines` |
| npm | 10+ | `npm ci` |

```bash
npm ci
npm run doctor   # eksik SDK/araçları raporlar
npm run icons    # apps.config.json + assets/icons → platform ikonları
```

## Android (Capacitor 7) — APK + AAB

**Gerekli:** JDK 17, Android SDK (platform-tools + build-tools + platform 34+).

### Yerel
```bash
npm run icons
cd capacitor/finans           # veya dcim / chat / task
npx cap sync android
cd android
./gradlew assembleRelease     # APK  → app/build/outputs/apk/release/
./gradlew bundleRelease       # AAB  → app/build/outputs/bundle/release/
```

### CI — `android.yml`
- Runner: `ubuntu-latest`
- Adımlar: `setup-java (17)` → `setup-android` → `setup-node (20)` → `npm ci` → `npm run icons`
  → `cap sync android` → `gradle assembleRelease bundleRelease` → APK + AAB artifact upload.
- Matris: `finans, dcim, chat, task` (4 paralel job).
- İmzalama: `SIGNING_KEYSTORE_BASE64` secret'ı varsa `keystore.properties` yazılır ve imzalanır;
  yoksa imzasız üretilir + uyarı. Bkz. [SIGNING.md](SIGNING.md).

## iOS (Capacitor 7) — Archive + IPA

**Gerekli:** macOS + Xcode 15+, CocoaPods, Apple Developer hesabı (dağıtım için).

### Yerel
```bash
npm run icons
cd capacitor/finans
npx cap sync ios
cd ios/App && pod install && cd -
# imzasız doğrulama:
xcodebuild -workspace capacitor/finans/ios/App/App.xcworkspace \
  -scheme App -configuration Release -destination 'generic/platform=iOS' \
  -archivePath build/finans.xcarchive clean archive \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

### CI — `ios.yml`
- Runner: `macos-latest`
- Secret **yoksa**: `CODE_SIGNING_ALLOWED=NO` ile imzasız `.xcarchive` üretilir (derleme doğrulaması,
  App Store'a yüklenemez).
- Secret **varsa** (`APPLE_CERT`, `APPLE_CERT_PASSWORD`, `PROVISIONING_PROFILE`, `TEAM_ID`): sertifika
  geçici keychain'e alınır, provisioning profile yüklenir, imzalı archive + `xcodebuild -exportArchive`
  ile IPA export edilir.

## Windows (Tauri 2) — MSI + EXE

**Gerekli:** Windows 10/11, Rust (stable, MSVC), WebView2 Runtime (Win11'de yerleşik).

### Yerel
```powershell
npm run icons
cd tauri/finans
npx @tauri-apps/cli@^2 build
# çıktı: src-tauri/target/release/bundle/msi/*.msi ve bundle/nsis/*.exe
```

### CI — `windows.yml`
- Runner: `windows-latest`
- Adımlar: `dtolnay/rust-toolchain@stable` → `setup-node (20)` → `npm ci` → `npm run icons`
  → `tauri build` → MSI + EXE artifact.
- İmzalama: `WINDOWS_CERTIFICATE` (base64 PFX) varsa `signtool` ile Authenticode imzalanır; yoksa unsigned.

## macOS (Tauri 2) — DMG + App

**Gerekli:** macOS + Xcode Command Line Tools, Rust (stable, apple targets).

### Yerel
```bash
npm run icons
cd tauri/finans
npx @tauri-apps/cli@^2 build
# çıktı: src-tauri/target/release/bundle/dmg/*.dmg ve bundle/macos/*.app
```

### CI — `macos.yml`
- Runner: `macos-latest`
- Tauri, `APPLE_CERTIFICATE` + `APPLE_SIGNING_IDENTITY` (+ notarization için `APPLE_ID` /
  `APPLE_PASSWORD` / `APPLE_TEAM_ID`) env'leri set edilirse **otomatik imzalar ve notarize eder**;
  secret yoksa imzasız DMG üretir.

## Release toplama — `release.yml`

`v*` tag push'unda `android/ios/windows/macos` workflow'ları `workflow_call` ile çağrılır,
tüm artifact'lar indirilir ve **taslak (draft) GitHub Release**'e eklenir. Notları gözden geçirip
yayınlayın. `secrets: inherit` sayesinde imzalama secret'ları tanımlıysa imzalı, değilse imzasız
artifact toplanır.

## Reusability notu

Dört platform workflow'u `workflow_call` tetiğini destekler; bu yüzden hem tek tek (dispatch/tag)
hem de `release.yml` tarafından yeniden kullanılabilir. Tüm imzalama secret'ları `required: false`
olduğundan eksik secret job'u **bozmaz**.
