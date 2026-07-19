# iOS overrides — Bogahost Finans

`npx cap add ios` sonrası CI bu dosyaları uygular:

| Kaynak (bu klasör) | Hedef |
| --- | --- |
| `Info.plist.partial.xml` | `ios/App/App/Info.plist` içine anahtarları birleştir |
| `App.entitlements` | `ios/App/App/App.entitlements` (hedefe bağla) |
| `apple-app-site-association` | CANLI siteye: `https://finans.bogahost.com/.well-known/apple-app-site-association` |

## MANUEL adımlar
1. **Team ID**: `apple-app-site-association` içindeki `TEAMID` yerine Apple
   Developer Team ID'nizi yazın (ör. `A1B2C3D4E5.com.bogahost.finans`).
2. **.well-known**: `apple-app-site-association` dosyasını CANLI siteye,
   uzantısız ve `Content-Type: application/json` ile,
   `https://finans.bogahost.com/.well-known/apple-app-site-association`
   yolunda yayınlayın. Universal Link doğrulaması bunu ister.
3. **Native push (APNs)**: APNs Auth Key (`.p8`), Key ID ve Team ID'yi push
   sağlayıcınıza ELLE tanımlayın. `aps-environment` gerçek dağıtımda
   `production` olmalı. Ayrıntı: `../../README.md`.
4. **İmzalama**: dağıtım sertifikası + provisioning profile CI secret'ı olarak
   sağlanır; `xcodebuild archive` imzasız üretir, imzalama ayrı adımdır.
