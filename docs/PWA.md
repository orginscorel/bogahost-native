# PWA olarak kurulum

5 sistem (Finans / DCIM / Chat / Görevler / Muhasebe) zaten canlı web uygulamalarıdır. Native kabuk yerine
bunları doğrudan **PWA (Progressive Web App)** olarak kurmak mümkündür — **mağaza, imzalama ücreti
veya inceleme gerektirmez**. Çoğu dahili kullanım için en hızlı yol budur.

| URL | Uygulama |
|-----|----------|
| https://finans.bogahost.com/admin | Finans |
| https://dcim.bogahost.com/admin | DCIM |
| https://chat.bogahost.com/admin | Chat |
| https://task.bogahost.com/admin | Görevler |

## Kullanıcı kurulum adımları

### Android (Chrome / Edge)
1. Siteyi aç → menü (⋮) → **Uygulamayı yükle** / "Ana ekrana ekle".
2. Uygulama ayrı ikon + tam ekran pencere olarak açılır.

### iOS / iPadOS (Safari)
1. Siteyi Safari'de aç → **Paylaş** → **Ana Ekrana Ekle**.
2. Not: iOS'ta PWA kurulumu **yalnızca Safari** üzerinden yapılır (Chrome iOS desteklemez).

### Windows (Edge / Chrome)
1. Siteyi aç → adres çubuğundaki **Yükle** simgesi ya da menü → **Uygulamalar → Bu siteyi uygulama olarak yükle**.
2. Başlat menüsüne/masaüstüne kısayol eklenir.

### macOS (Safari 17+ / Chrome / Edge)
1. Safari: **Dosya → Dock'a Ekle**. Chrome/Edge: menü → **Yükle**.

## Manifest / Service Worker durumu

PWA kurulabilirliği için canlı sitede şunlar gerekir (backend ekibinde — bu depo dokunmaz):
- Geçerli `manifest.webmanifest` (name, short_name, icons 192+512, `display: standalone`,
  `theme_color`, `background_color`). Değerler `apps.config.json` ile hizalı olmalı.
- HTTPS üzerinden yayınlanan bir **Service Worker** (offline/önbellek + install prompt için).
- `theme_color` = `#5443D2`, `background_color` = `#0e1015` (config ile aynı).

> Bu depodaki kabuklar zaten aynı canlı URL'yi yükler; manifest/SW canlı tarafta mevcutsa hem PWA
> kurulumu hem native kabuk aynı deneyimi verir.

## `.well-known` — native ↔ web bağlama (MANUEL, canlı siteyi değiştirir)

Native kabuk ile PWA'yı birbirine bağlamak (Android App Links "doğrulanmış link", iOS Universal
Links, Google şifre otomatik doldurma) için canlı sitenin köküne iki dosya konur. **Bu adım canlı
web sunucusunu değiştirir → önce kullanıcı/yönetici onayı alınmalıdır. Bu depo bu dosyaları
yazmaz.**

### Android — `/.well-known/assetlinks.json`
Her hostname için (`finans.bogahost.com` vb.), o uygulamanın imza SHA-256 parmak izini içerir:

```json
[{
  "relation": ["delegate_permission/common.handle_all_urls"],
  "target": {
    "namespace": "android_app",
    "package_name": "com.bogahost.finans",
    "sha256_cert_fingerprints": ["AA:BB:CC:...  (release/Play imza parmak izi)"]
  }
}]
```
Parmak izi:
```bash
keytool -list -v -keystore bogahost-release.keystore -alias bogahost | grep SHA256
# Play App Signing kullanıyorsanız: Play Console → App integrity → App signing key sertifikası
```

### iOS — `/.well-known/apple-app-site-association` (uzantısız, `application/json`)
```json
{
  "applinks": {
    "apps": [],
    "details": [{ "appID": "TEAMID.com.bogahost.finans", "paths": ["*"] }]
  }
}
```

Her 4 hostname için ilgili `package_name` / `appID` ile ayrı dosya. Dosyalar **HTTPS**, yönlendirmesiz
ve doğru MIME ile sunulmalıdır. Yerleştirme kararı ve uygulaması **manuel** ve backend ekibinin
onayına tabidir.
