# iOS overrides — Bogahost Görevler

| Kaynak | Hedef |
| --- | --- |
| `Info.plist.partial.xml` | `ios/App/App/Info.plist` içine birleştir |
| `App.entitlements` | `ios/App/App/App.entitlements` |
| `apple-app-site-association` | `https://task.bogahost.com/.well-known/apple-app-site-association` |

## MANUEL adımlar
1. `apple-app-site-association` içindeki `TEAMID` → Apple Team ID.
2. Dosyayı uzantısız, `application/json` ile `.well-known/` altında yayınla.
3. Native push: APNs `.p8` + Key ID + Team ID push sağlayıcısına tanımlanır.
4. İmzalama: dağıtım sertifikası + provisioning profile CI secret'ı.
