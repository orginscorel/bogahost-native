# iOS overrides — Bogahost Muhasebe Arşivi

`npx cap add ios` sonrası CI bu dosyaları uygular:

| Kaynak (bu klasör) | Hedef | Kim uygular |
| --- | --- | --- |
| `Info.plist.partial.xml` | `ios/App/App/Info.plist` içine anahtarları birleştir | `scripts/prepare.mjs` (PlistBuddy) |
| `App.entitlements` | `ios/App/App/App.entitlements` **+ pbxproj `CODE_SIGN_ENTITLEMENTS`** | `scripts/prepare.mjs` (otomatik) |
| `apple-app-site-association` | CANLI siteye: `https://muh.bogahost.com/.well-known/apple-app-site-association` | **ELLE** (sunucu dosyası) |

Sürüm alanları (`MARKETING_VERSION` / `CURRENT_PROJECT_VERSION`) de `prepare.mjs`
tarafından kök `package.json` sürümünden ve CI run numarasından yazılır.

## Hâlâ ELLE yapılacaklar

1. **Team ID** — `apple-app-site-association` içindeki `TEAMID` yer tutucusunu
   gerçek Apple Developer Team ID'nizle değiştirin:

   ```json
   "appID": "A1B2C3D4E5.com.bogahost.muh"
   ```

   Team ID'yi <https://developer.apple.com/account> → Membership details
   sayfasında bulursunuz (10 karakter, harf+rakam). App Store Connect'te
   göründüğü hâliyle birebir yazın.

2. **.well-known yayını** — dosyayı CANLI siteye **uzantısız** ve
   `Content-Type: application/json` başlığıyla,
   `https://muh.bogahost.com/.well-known/apple-app-site-association`
   yolunda yayınlayın. Yönlendirme OLMAMALI (301/302 kabul edilmez) ve
   kimlik doğrulaması istememeli. Universal Link doğrulaması bunu okur.

3. **Native push (APNs)** — APNs Auth Key (`.p8`), Key ID ve Team ID'yi push
   sağlayıcısına ELLE tanımlayın. `App.entitlements` içinde `aps-environment`
   `production`'dır; TestFlight ve App Store dağıtımları production APNs
   kullanır, bu doğrudur. (Yalnız Xcode'dan cihaza doğrudan kurarken
   `development` gerekir.)

4. **İmzalama** — dağıtım sertifikası + provisioning profile CI secret'ı olarak
   verilir; `.github/workflows/ios.yml` gerisini yapar.

Tam adım adım kılavuz: [`../../../docs/IOS.md`](../../../docs/IOS.md)
