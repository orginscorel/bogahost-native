# Bogahost Native — Dokümantasyon

Bu klasör, Bogahost'un 4 sisteminin (Finans / DCIM / Chat / Görevler) native kabuklarının
derleme, imzalama, dağıtım ve sorun giderme belgelerini içerir.

Proje özeti ve mimari için kök [`../README.md`](../README.md).

## Belgeler

| Dosya | İçerik |
|-------|--------|
| [BUILD.md](BUILD.md) | Her platform için yerel + CI derleme adımları, gerekli OS/SDK |
| [SIGNING.md](SIGNING.md) | Android keystore, iOS sertifika/provisioning, Windows Authenticode, macOS Developer ID + secret adları |
| [IOS.md](IOS.md) | **iOS uçtan uca**: Apple Developer hesabı → App ID → sertifika/profil → GitHub secret'ları → TestFlight → App Store inceleme notları, push (APNs) ve universal link kurulumu |
| [PUBLISH.md](PUBLISH.md) | Google Play, App Store/TestFlight, Microsoft Store/MSI, macOS notarize+DMG |
| [PWA.md](PWA.md) | Mağazasız PWA kurulumu + `.well-known` (assetlinks / AASA) adımı |
| [PUSH.md](PUSH.md) | Bildirim gerçeği: web-push vs. native FCM/APNs |
| [WEB-YETENEKLERI.md](WEB-YETENEKLERI.md) | Kamera/mikrofon, ekran paylaşımı, SSE, sürükle-bırak, pano, ses, tam ekran… hangi yetenek hangi platformda çalışır, çalışmıyorsa **neden** |
| [UPDATE.md](UPDATE.md) | Masaüstü **tam otomatik güncelleme**: mimari, `latest.json` şeması, imza anahtarı üretimi, GitHub secret'ları, Cloudflare notu, sorun giderme |
| [TROUBLESHOOT.md](TROUBLESHOOT.md) | Yaygın hatalar ve çözümleri |

## CI iş akışları (`.github/workflows/`)

| Workflow | Runner | Üretir | Tetik |
|----------|--------|--------|-------|
| `android.yml` | ubuntu-latest | APK + AAB (4 uygulama) | dispatch, `v*` tag, workflow_call |
| `ios.yml` | macos-latest | .xcarchive (imzasız doğrulama) / IPA (imzalıysa) | dispatch, `v*` tag, workflow_call |
| `windows.yml` | windows-latest | MSI + EXE (NSIS) | dispatch, `v*` tag, workflow_call |
| `macos.yml` | macos-latest | DMG + App | dispatch, `v*` tag, workflow_call |
| `pwa-lighthouse.yml` | ubuntu-latest | Lighthouse raporu (canlı 4 URL) | dispatch, haftalık cron |
| `release.yml` | çok platform | 4 workflow'u çağırıp GitHub Release'e toplar | `v*` tag, dispatch |

Tüm imzalama secret'ları **opsiyoneldir**: tanımlı değilse workflow **FAIL etmez**, imzasız
artifact üretir ve bir `::warning` bırakır. Secret adları için [SIGNING.md](SIGNING.md).
