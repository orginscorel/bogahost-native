# PWA İyileştirmeleri — Canlıya Uygulama Kılavuzu

> ⚠️ **Bu değişiklikler 4 CANLI uygulamanın frontend'ine dokunur.** Kullanıcı onayı olmadan
> uygulama. Her dosyayı değiştirmeden önce `.bak` yedeği al, sonra ilgili modülü test et.
> Backend / API / DB / WebSocket / yetki KATMANLARINA DOKUNULMAZ — yalnız statik PWA varlıkları
> ve layout'a küçük eklemeler.

Mevcut durum (denetim sonucu): manifest'ler ve ikonlar **zaten tam ve profesyonel**
(id, name, maskable icon, shortcuts, display_override hepsi var; icon-192/512/maskable +
apple-touch + favicon mevcut). Yalnızca aşağıdaki 2 gerçek eksik tamamlanır.

## Uygulama başına yollar
| Uygulama | Kök (public) | Layout | Service worker |
|---|---|---|---|
| Finans | `/home/finansboga/finans/public` | `resources/views/admin/layout.blade.php` | `public/push-sw.js` |
| DCIM | `/home/dcimboga/dcim/public` | `resources/views/admin/layout.blade.php` | `public/push-sw.js` |
| Chat | `/home/bogahost/public_html/livechat/public` | `resources/views/admin/layout.blade.php` | `public/sw.js` |
| Görevler | `/home/taskboga/laravel/public` | `resources/views/admin/layout.blade.php` | `public/push-sw.js` |

## 1) Çevrimdışı (offline) sayfası — 5 uygulama
1. `offline.html` → her uygulamanın `public/offline.html` konumuna kopyala.
   (İstersen tema rengini uygulamaya göre değiştir; şablon Finans moru #5443D2.)
2. `sw-offline-snippet.js` içeriğini ilgili SW dosyasının **en altına** ekle.
   - ÖNCE kontrol et: mevcut SW `fetch` içinde navigasyon (`mode === 'navigate'`) için zaten
     `respondWith` yapıyor mu? Yapıyorsa append ETME → snippet'in navigation-fallback mantığını
     mevcut fetch handler'ına entegre et (tek respondWith kazanır). Çoğu push-sw.js yalnız
     `push`/`notificationclick` dinler, `fetch` yok → append güvenli.
   - `chown <kullanıcı>:<kullanıcı>` yap (finansboga/dcimboga/bogahost/taskboga).
3. SW sürümünü/`?v=` cache-buster'ını bump et (footer/register satırında) ki cihazlar yeni SW'yi alsın.

## 2) "Uygulamayı Yükle" istemi — Task (eksik) + tutarlılık
- `install-prompt.blade.php` → her uygulamanın `resources/views/partials/install-prompt.blade.php`
  konumuna koy; layout'ta `</body>` ÖNCESİNE `@include('partials.install-prompt')` ekle.
- **Finans**: layout'ta zaten `beforeinstallprompt` mantığı VAR → çakışmasın diye ya mevcut olanı
  kaldırıp bu ortak partial'ı kullan, ya da eklemeyi atla.
- **DCIM**: güncel layout'ta prompt yok (yalnız `.bak`'ta) → partial'ı ekle.
- **Task**: prompt yok → partial'ı ekle.
- iOS Safari için partial otomatik "Paylaş → Ana Ekrana Ekle" yönergesi gösterir.

## Uygulama sonrası test (her uygulama için)
- İlgili modül sayfaları 200/302 (500 yok) — `User::find(<admin>)` + HTTP kernel render.
- `/offline.html` doğrudan açılıyor (200).
- Manifest hâlâ geçerli, SW hatasız register oluyor (DevTools → Application).
- Chat/Task/DCIM/Finans ekranları + push + WebSocket bozulmadı.
- Değişen her dosya `.bak` yedekli, doğru kullanıcı sahipliğinde.

## Native kabuk (assetlinks / apple-app-site-association)
Android App Links + iOS Universal Links doğrulaması için canlı siteye şu dosyalar konur
(bu da CANLI dokunuş → ayrı onay): `public/.well-known/assetlinks.json` ve
`public/.well-known/apple-app-site-association`. Şablonlar `capacitor/<key>/` altında;
SHA256/TeamID imzalama sonrası doldurulur (bkz. `docs/SIGNING.md`).
