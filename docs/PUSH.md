# Bildirimler (PUSH)

## Gerçek durum

Bogahost'un mevcut push altyapısı **Web Push (VAPID / aes128gcm)** üzerine kuruludur ve 4 sistemde
(DCIM / Chat / Görevler / Finans) çalışır — bkz. auto-memory `bogahost-push-birlesik` ve
`bogahost-push-reliability-2026-07-19`. Bu bildirimler:

- **PWA olarak kurulmuş** uygulamalarda ve masaüstü tarayıcılarda çalışır (Service Worker + Web Push).
- **iOS'ta yalnızca** Safari'den **Ana Ekrana Eklenmiş PWA** içinde çalışır (iOS 16.4+).

## Native kabuklarda durum

Bu depodaki native kabuklar canlı URL'yi bir WebView'de yükler. WebView içindeki Web Push,
platform WebView'inin desteğine bağlıdır ve **arka planda / uygulama kapalıyken güvenilir teslim
sağlamaz**. Gerçek native bildirim için platform servisleri gerekir:

| Platform | Native servis | Gerekli |
|----------|---------------|---------|
| Android (Capacitor) | **FCM** (Firebase Cloud Messaging) | `google-services.json` + FCM sender key |
| iOS (Capacitor) | **APNs** | APNs Auth Key (`.p8`) + Apple Push yetkisi |
| Windows/macOS (Tauri) | OS bildirimleri (`tauri-plugin-notification`) — uzaktan push için ayrı köprü | — |

> **Karar (güncellendi):** Masaüstü kabuğunda bildirim için **PWA kurmak GEREKMEZ ve
> beklenmez.** Kullanıcı uygulamayı açar; bildirimi **uygulamanın kendisi** getirir ve
> **işletim sistemi bildirimi** olarak gösterir. Tarayıcı, Service Worker, `PushManager`,
> VAPID ve Apple/APNs zincirinin tamamı masaüstü kabuğunda **devre dışıdır** —
> bkz. [Masaüstü bildirim mimarisi](#masaüstü-tauri--bildirim-mimarisi-saat-rustta).
> Native FCM/APNs yalnızca **Capacitor (Android/iOS)** tarafı için ve yalnızca uygulama
> tamamen kapalıyken garantili teslim şartsa gereklidir.

## Masaüstü (Tauri) — Notification köprüsü (v1.5.0)

**Sorun:** Tauri WebView'i `window.Notification` **sunmuyor**. Panellerin Bildirim/Cihazlar
ekranı bu nesneyi arayıp bulamayınca *"Bu tarayıcı bildirimi desteklemiyor"* diyordu.

**Çözüm:** Sayfaya enjekte edilen betik (`INIT_SCRIPT`) standart Notification API'sinin bir
**shim**'ini kurar ve native masaüstü bildirimine bağlar:

| Sayfa tarafı | Native komut | Davranış |
|---|---|---|
| `new Notification(t, {body})` | `bogahost_notify` | Masaüstü bildirimi gösterir |
| `Notification.permission` | `bogahost_notify_state` | `"granted"` veya `"default"` |
| `Notification.requestPermission()` | `bogahost_notify_request` | Sistem izin penceresini **arka planda** açar, sonucu yoklar, izin verilirse **test bildirimi** gösterir |

Shim yalnızca `window.Notification` **yoksa** kurulur — çalışan bir WebView bozulmaz.
Ayrıca paneller native kabuğu ayırt edebilsin diye `window.__BOGAHOST_NATIVE_NOTIFY__ = true`
ve doğrudan çağrılabilen `window.__bogahostNotify(title, body)` sunulur.

## Masaüstü (Tauri) — besleme köprüsü (v1.6.0) ⭐ ASIL DÜZELTME

v1.5.0 köprüsü **yetersizdi**, çünkü yanlış varsayıma dayanıyordu: panellerin
`new Notification()` çağırdığı varsayılmıştı. **Çağırmıyorlar.**

**Gerçek akış (kaynak koddan doğrulandı):**

1. Panel her ~30 sn bildirim beslemesini yoklar
   (`/admin/notifications/feed` — DCIM / Finans / Görevler;
   `/admin/notifications` — Chat, imleç tabanlı).
2. Yeni kayıt varsa **sayfa içi baloncuk (LCToast) + bip** gösterir.
3. Masaüstü bildirimi için ayrıca **service worker + web push** kullanır
   (`pushInit()` → `navigator.serviceWorker` + `PushManager`).

Native WebView'de 3. adım **hiç çalışmaz** — `serviceWorker` ve `PushManager` yoktur,
`pushInit()` ilk satırda `return` eder. `new Notification()` de hiç çağrılmadığı için
v1.5.0 shim'i **asla tetiklenmedi**. Kullanıcının "hiçbir bildirim gelmiyor"
şikâyetinin tam sebebi budur.

## Masaüstü (Tauri) — bildirim mimarisi (saat Rust'ta)

**İlke:** Bildirimi **tarayıcı değil, uygulama** getirir ve **uygulama** gösterir.
Kabuk; Service Worker, `PushManager`, VAPID, Apple/APNs zincirinin **hiçbirine bağlı değildir.**

```
Rust  start_notify_clock          (işletim sistemi iş parçacığı, 45 sn — kısılmaz)
  └─ webview.eval("__bogahostFeedTick()")
       └─ sayfa: fetch(panelin bildirim ucu)      (oturum çerezi + CSRF + yetki SAYFADA çözülü)
            └─ invoke("bogahost_notify_feed", {items, unread})
                 └─ Rust: kalıcı tekilleştirme → NATIVE bildirim + Dock/tepsi rozeti
```

### Neden bu bölüşüm?

| Soru | Seçilen | Gerekçe |
|---|---|---|
| **Saati kim tutar?** | **Rust** (OS iş parçacığı) | Sayfadaki `setInterval` pencere **gizlendiğinde/örtüldüğünde** işletim sistemi tarafından kısılır — tam da uygulama tepsideyken, yani bildirimin en çok beklendiği anda. `eval` ile **açıkça çalıştırılan** betik zamanlayıcı değildir; pencere gizliyken de anında koşar. |
| **İsteği kim atar?** | **Sayfa** | Oturum çerezi `HttpOnly`'dir ve panelin `auth`/`perm` ara katmanları sayfa bağlamında zaten geçerlidir. Çerezi Rust'a taşımak (`cookies_for_url`) mümkün ama fazladan bir kırılma noktası olurdu. |
| **Kalıcılık/gösterim?** | **Rust** | `localStorage` panel tarafından temizlenebilir; `app_config_dir` uygulamanın kendi verisidir. |

### Kullanılan panel ucu (YENİ UÇ YAZILMADI)

Zilin **zaten** yokladığı uç aynen kullanılır — backend'e tek satır dokunulmadı.

| Uygulama | Uç | Şema |
|---|---|---|
| Finans / DCIM / Görevler | `GET /admin/notifications/feed` | `{unread, items:[{id,title,body,url,read,age_s}]}` |
| Chat | `GET /admin/notifications?after_msg&after_conv&after_internal` | `{ok, messages[], new_conversations[], internal_messages[], max_*, waiting}` |

Şema **gövdedeki alana** göre ayırt edilir (`items` vs `max_msg`), URL'e göre değil.
Chat kayıtları (`msg:` / `conv:` / `int:`) kabuk tarafında başlık/gövde/adrese normalize edilir.

### Kararlar

| Konu | Karar |
|---|---|
| Ek sunucu yükü | **Pencere açıkken sıfır**: panel zaten yokluyor, `window.fetch` sarmalanıp yanıt gövdesi klonlanarak okunur. Kendi isteğimiz yalnızca **60 sn'dir panel yanıtı görülmediyse** atılır (yani pratikte pencere gizliyken). |
| Tekilleştirme | **Kalıcı**, `app_config_dir/notify-state.json` içinde son **300** bildirim anahtarı (halka tampon). Uygulama yeniden başlayınca eski bildirimler **tekrar patlamaz**. |
| 4 uygulama ayrımı | Paket kimlikleri farklı → `app_config_dir` farklı → **her uygulama kendi listesini** tutar. |
| İlk çalıştırma | Kalıcı liste **dosyası henüz yoksa** yalnızca **15 dk'dan genç** kayıtlar duyurulur; gerisi sessizce "görüldü" işaretlenir. (v1.9.7'ye kadar 120 sn idi ve uç son 12 kaydı döndürdüğü için ilk tur pratikte **her şeyi yutuyordu**. Dosya **okunamadığında** da "ilk çalıştırma" sayılıyordu — yani bozuk/erişilemez dosya bildirimleri **kalıcı olarak** susturabiliyordu; artık okuma hatasında **susturulmaz**.) |
| Patlama koruması | Tek turda en fazla **4** bildirim; fazlası tek "**N yeni bildirim var**" özetine düşer. |
| Okunmuşlar | Panelde `read` işaretli kayıt masaüstünde **duyurulmaz**. |
| Rozet | `unread` (Chat'te `waiting`) → `set_badge_count` (macOS/Linux Dock). **Windows'ta desteklenmez**, hata sessizce yutulur. |
| Sessizlik | Ağ hatası / 5xx → **üstel geri çekilme** (90 sn → 15 dk tavan). 401/403 → yoklama **5 dk** durur (v1.9.7'ye kadar **kalıcı** dururdu: tek bir 401 bildirimleri sayfa yenilenene kadar tamamen öldürüyordu). Hiçbirinde **kullanıcıya bildirim çıkmaz** — ama artık hepsi **"Bildirim durumu…"** ekranına ve `stderr`e yazılır. |
| Giriş ekranı | `/admin` dışındaysa **ya da** adres `/admin/login`, `/admin/logout`, `/admin/2fa/…`, `/admin/erisim-engeli` ise yoklama **yapılmaz** (401 döngüsü olmasın). (v1.9.7'ye kadar "sayfada `input[type=password]` var mı" bakılıyordu; panelin profil/şifre veya kasa ekranı **o sayfada** yoklamayı tamamen susturuyordu.) |
| Adres güvenliği | Bildirimin `url` alanı yalnızca `*.bogahost.com` ise kabul edilir (`resolve_internal_url`). |

### Teşhis ve elle test (tepsi menüsü)

Bu zincirin **her halkası** eskiden hatasını sessizce yutuyordu (`let _ = eval(...)`,
`let _ = show()`, sayfa tarafında boş `catch`). "Bildirim gelmiyor" denildiğinde
nerede koptuğunu gösteren tek bir işaret yoktu. Tepsiye iki öğe eklendi:

| Öğe | Ne yapar | Ne ayırt eder |
|---|---|---|
| **Test bildirimi gönder** | Anında native bildirim gösterir + gerçek beslemeyi bir kez zorla yoklatır | Bildirim **görünüyorsa** işletim sistemi/izin tarafı sağlamdır → sorun yoklamadadır. **Görünmüyorsa** izin kapalıdır → sayfada çıkan kutudaki "Sistem Ayarlarını Aç" düğmesi doğrudan oraya götürür. |
| **Bildirim durumu…** | Zincirin 5 halkasını tek ekranda gösterir: yoklama turu sayısı + son tur, sayfa tarafının son durumu, köprü (invoke) çağrı sayısı + son çağrı, gösterilen bildirim sayısı + son başlık, tekilleştirme dosyası + kayıtlı anahtar sayısı | Hangi halkanın çalıştığını/durduğunu **ölçerek** gösterir |

Ayrıca her adım `stderr`e `[<app>][notify] …` önekiyle yazılır.

### İzin durumu — dürüst durum (ÖNEMLİ)

`tauri-plugin-notification` v2'nin **masaüstü** uygulamasında
(`plugins-workspace/plugins/notification/src/desktop.rs`) `permission_state()` ve
`request_permission()` **sabit olarak `Granted` döner** — yalnızca Windows'ta değil,
**macOS ve Linux'ta da**. Yani uygulama izin durumunu **okuyamaz**.

Sonuç: v1.9.7'ye kadar tepside gösterilen **"Bildirimler: açık"** etiketi bir
**iddiaydı**, ölçüm değil; izin gerçekte kapalıyken de "açık" yazıyordu. Artık durum
iddia edilmez — öğe doğrudan **sistem ayarına** götürür, gerçek durum ise yukarıdaki
**"Test bildirimi gönder"** ile ölçülür.

> Ek olarak: `x-apple.systempreferences:` / `ms-settings:` adresleri
> `tauri-plugin-shell`in varsayılan `open` süzgecinden (`^((mailto:\w+)|(tel:\w+)|(https?://\w+)).+`)
> **geçemez**. Yani "ayarları aç" düğmesi de v1.9.7'ye kadar **hiçbir şey yapmıyordu**;
> artık işletim sisteminin kendi açıcısı doğrudan çağrılır (bkz. `open_native`).

### Bildirime tıklama — dürüst durum

`tauri-plugin-notification` (v2) masaüstünde bildirime **tıklama/aksiyon geri çağrısı
SUNMAZ** — `NotificationBuilder`'da `on_click`/`on_action` **yoktur** (docs.rs ile
doğrulandı). Bu yüzden hedef adres saklanır ve tepsi menüsündeki
**"Son bildirimi aç"** öğesine bağlanır; tıklanınca sayfa ana pencerede açılır ve
pencere öne getirilir. **Olmayan bir API uydurulmadı.**

### Ne zaman ne çalışır?

| Durum | Bildirim düşer mi? | Nasıl |
|---|---|---|
| Pencere **açık ve önde** | ✅ | Panelin kendi yoklaması sarmalayıcıdan okunur (ek istek yok) |
| Pencere **arkada / örtülü** | ✅ | Rust saati 45 sn'de bir `eval` eder; panel yoklaması dursa da kabuk kendi ister |
| Pencere **kapalı, uygulama tepside** | ✅ | Aynı — WebView canlıdır, saat Rust'tadır |
| Oturum açılışında **`--hidden` başlatıldı** | ✅ | v1.7.0 autostart; pencere hiç açılmaz, kabuk tepside yoklamaya devam eder |
| Panel oturumu düştü (401/403) | ❌ (sessiz) | Yoklama **5 dk** durur, sonra kendiliğinden yeniden dener; kullanıcı panele girince sürer |
| Uygulama **tamamen kapalı (süreç yok)** | ❌ | **Fiziksel sınır** — aşağıya bakın |

> **Kısılma notu (dürüst):** Rust saati kısılmaz, ancak `eval` ile tetiklenen `fetch`
> WebView içinde koşar. WebKit/WebView2'nin örtülü pencerede ağ isteğini geciktirmesi
> teorik olarak mümkündür. Bu **ölçülmedi** (bu makinede Rust yok, derlenemedi).
> Ölçülene kadar iddia edilen tek şey: **zamanlayıcı kısılması bu tasarımda ortadan
> kalkmıştır**, çünkü artık zamanlayıcı kullanılmıyor.

### Kritik sınır — uygulama TAMAMEN kapalıyken

Süreç yoksa bu yöntem **çalışmaz** ve bu bir hata değil, **fiziksel sınırdır**:

- Gerçek APNs push için **Apple Developer hesabı + imzalı/notarize edilmiş uygulama +
  push entitlement (`aps-environment`) + APNs gönderim ucu** gerekir. macOS'ta **imzasız**
  bir uygulama APNs token'ı **alamaz**. Windows'taki karşılığı WNS'tir ve o da **MSIX paket
  kimliği** ister. Bu depo **imzasız kabuk üretir**; ikisi de **YOKTUR**.
- **Pratikte sorun oluşturmaz:** v1.7.0 ile oturum açılışında **sessizce tepside başlatma**
  (`--hidden` + autostart) eklendi, close-to-tray zaten vardı. Uygulama fiilen **sürekli
  çalışır**, dolayısıyla bildirim her zaman düşer. "Süreç yok" durumu yalnızca kullanıcı
  **bilerek Cmd+Q / Çıkış** dediğinde oluşur — ve o an bir kez uyarı gösterilir.

> **Not:** WebView'de `PushManager` yoktur ve panel push aboneliği kuramaz. Bu **artık bir
> eksiklik değildir** — masaüstü kabuğu bildirim için web-push zincirini hiç kullanmıyor.
> Kullanıcının PWA/Safari ile uğraşmasına **gerek yoktur**.

## Native push eklemek (özet adımlar — manuel)

Bu adımlar canlı backend'e ve kabuk projelerine kod/anahtar ekler; **bu depo bunları otomatik
yapmaz**, ayrı bir iş kalemidir.

### Android — FCM
1. Firebase projesi oluştur, her `com.bogahost.<key>` için Android app kaydet.
2. `google-services.json`'ı ilgili `capacitor/<key>/android/app/` altına koy (secret olarak yönet).
3. `@capacitor/push-notifications` eklentisini ekle; token'ı canlı backend'e kaydet.
4. Backend, mevcut web-push yanında FCM HTTP v1 API ile de gönderim yapmalı (token tipine göre yönlendirme).

### iOS — APNs
1. Apple Developer → Keys → **APNs Auth Key** (`.p8`) üret; Key ID + Team ID not al.
2. App ID'lerde **Push Notifications** yetkisini aç (her `com.bogahost.<key>`).
3. `@capacitor/push-notifications` ile token al, backend'e kaydet.
4. Backend, APNs'e (`.p8` JWT) gönderim yapacak şekilde genişletilir.

### Masaüstü (Tauri)
- **Gerekmiyor.** Kabuk paneli kendisi yokluyor ve bildirimi kendisi gösteriyor
  (bkz. [bildirim mimarisi](#masaüstü-tauri--bildirim-mimarisi-saat-rustta)).
- APNs/WNS yalnızca uygulama **tamamen kapalıyken** teslim şartsa gerekir; o da imzalı
  uygulama + paket kimliği ister.

## Masaüstü (Tauri) — "kapalıyken de bildirim" (v1.6.0 sonrası)

Kullanıcı beklentisi: *"uygulamalar arka planda kapalı bile olsa çalışır halde olmalı,
bildirimler düşmeli."*

**Fiziksel sınır (dürüst hâli):** Süreç TAMAMEN sonlandıysa masaüstü kabuğa bildirim
**gelemez** — bunun için işletim sistemi düzeyinde bir push servisi (APNs/WNS) ve
**imzalı/notarize edilmiş** bir uygulama gerekir. Bu depo imzasız kabuk üretir.

**Pratikte istenen sonucu veren çözüm** (üç parça birlikte):

| Parça | Ne yapar |
|---|---|
| **Otomatik başlatma (autostart)** | `tauri-plugin-autostart` — bilgisayar açılınca uygulama `--hidden` argümanıyla sessizce başlar, pencere açılmaz, yalnızca tepside durur |
| **Close-to-tray** (zaten vardı) | Pencere kapatılınca uygulama çıkmaz, tepside çalışmaya devam eder |
| **Bildirim yoklayıcı** (saat Rust'ta) | Tepside çalışırken WebView canlıdır; Rust saati 45 sn'de bir yoklamayı tetikler ve her yeni kayıt native masaüstü bildirimi olur |

Sonuç: uygulama **fiilen sürekli çalışır**, kullanıcı açısından "kapalıyken de bildirim
geliyor" beklentisi karşılanır.

### Tepsi öğesi

**Bilgisayar açılınca başlat** — açma/kapama, işaretli durum işletim sistemindeki gerçek
kayıttan okunur (`autolaunch().is_enabled()`), varsayılmaz.

- **İlk kurulumda varsayılan: AÇIK** (`mark_once("autostart-default")` ile yalnızca bir kez
  uygulanır — kullanıcı kapatırsa bir daha zorlanmaz).
- macOS: LaunchAgent · Windows: `Run` kayıt anahtarı.

### Cmd+Q / Çıkış

Tam çıkış **engellenmez**. Yalnızca **ilk seferde** bilgilendirme diyaloğu çıkar
("kapalıyken bildirim alınmaz, tepside bırakabilirsiniz"), sonra bir daha gösterilmez
(`mark_once("quit-notice")`). Diyalog yanıtlanmasa bile 20 sn sonra uygulama kapanır —
"kapanmayan uygulama" durumu oluşmaz.

### İlk açılışta bildirim izni

- İzin yoksa **bir kez** anlaşılır bir soru sorulur ("Bildirimleri açmak ister misiniz?
  Yeni görev, mesaj ve uyarılar anında iletilir"), ardından sistem izin penceresi açılır.
- Kullanıcı reddederse **üstelenmez**. Kalıcı ama rahatsız etmeyen yol: tepsideki
  **"Bildirimler: kapalı (ayarları aç)"** öğesi — tıklayınca doğrudan sistem ayarına gider
  (macOS `x-apple.systempreferences:…notifications`, Windows `ms-settings:notifications`).
- İzin verilirse tek seferlik doğrulama bildirimi gösterilir.

> Windows'ta `tauri-plugin-notification` izin durumunu güvenilir raporlamaz (her zaman
> `Granted` döner); bu yüzden Windows'ta durum **iddia edilmez**, ayara yönlendirilir.

## Özet

- **Masaüstü kabuğu (Tauri):** bildirimi **uygulama** getirir ve **uygulama** gösterir.
  Tarayıcı / Service Worker / `PushManager` / VAPID / Apple **kullanılmaz**. PWA kurmaya
  **gerek yoktur**. Uygulama açık veya tepsideyken bildirim düşer; tamamen kapalıyken düşmez.
- **Web (tarayıcı / kurulu PWA):** mevcut Web Push aynen çalışmaya devam eder — bu depo ona
  dokunmaz.
- **Capacitor (Android/iOS) tarafında garantili arka plan push:** FCM + APNs — manuel kimlik
  + entegrasyon ister.
- Bu depo push **kodu/anahtarı içermez** ve **panel dosyalarına dokunmaz**; yalnızca kabuğu üretir.
