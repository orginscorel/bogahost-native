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

> **Karar:** Bildirim yeterliliği için en az sürtünme = uygulamayı **PWA olarak kurmak** (mevcut
> web-push aynen çalışır). Native FCM/APNs yalnızca uygulama kapalıyken/arka planda garantili teslim
> şartsa gereklidir ve **manuel kimlik bilgisi + kod entegrasyonu** ister.

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

**v1.6.0 çözümü:** `EXTRA_SCRIPT` içinde `window.fetch` sarmalanır ve panelin
**kendi** besleme yanıtı (`res.clone().json()`) dinlenir.

| Konu | Karar | Gerekçe |
|---|---|---|
| Ek istek | **Yok** | Panel zaten yokluyor; gövde klonlanıp okunur. 429 riski artmaz |
| Tekilleştirme | `localStorage` imleci + anahtar seti | Aynı bildirim iki kez çıkmaz |
| İlk açılış | Yalnız imleç kurulur | Geçmiş bildirimler toplu gösterilmez |
| Chat farklı şeması | Alan adına göre ayrılır (`items` vs `max_msg`) | URL'e değil gövdeye bakılır |
| Yedek yoklama | 45 sn hiç yanıt görülmezse başlar | Panel yoklamazsa da bildirim gelsin |
| Geri çekilme | 429/503/hata → üstel, 5 dk tavan; 401/403 → durur | Sunucu boğulmasın |
| Tıklama | Tepside **"Son bildirimi aç"** | Masaüstünde bildirime tıklama olayı **yok** (aşağıya bakın) |
| Adres güvenliği | Yalnız `*.bogahost.com` kabul edilir | Panel kabuğu yabancı adrese götüremez |

Panelin kendi `new Notification()` çağrısı da (varsa) **aynı** tekilleştirmeden geçer
(`window.__bogahostNotifyOnce`), yani çift bildirim olmaz.

### Sınırlar (net olarak)

- **UYGULAMA AÇIKKEN:** bildirimler gelir. ✅ (v1.6.0 ile düzeltildi)
- **UYGULAMA KAPALIYKEN:** bildirim **gelmez**. ❌ Bu bir hata değil, mimari sınırdır:
  gerçek APNs push için **Apple Developer hesabı + imzalı/notarize edilmiş uygulama +
  push entitlement (`aps-environment`) + APNs gönderim ucu** gerekir. macOS'ta imzasız
  bir uygulama APNs token'ı **alamaz**. Windows'ta karşılığı WNS'tir ve o da paket
  kimliği (MSIX) ister. Bu depoda **yoktur**.
- Bildirimin **kendisine tıklama** olayı `tauri-plugin-notification` tarafından
  masaüstünde sunulmaz. Bu yüzden hedef adres saklanıp tepsi menüsündeki
  **"Son bildirimi aç"** öğesine bağlandı — uydurma bir API kullanılmadı.
- Pencere gizliyken zamanlayıcı işletim sistemi tarafından yavaşlatılabilir;
  bu kabuk tarafından aşılamaz.

### Eski sınırlar (v1.5.0 shim'i — hâlâ geçerli)

- Bu **gerçek web-push DEĞİLDİR.** WebView'de `PushManager` yoktur; panel push aboneliği
  kuramaz ve **uygulama kapalıyken sunucudan bildirim gelmez.**
- Çalışan şey: **panel açıkken** üretilen her bildirimin masaüstünde görünmesi ve
  "Bildirim aç" akışının gerçekten izin alıp **test bildirimi göstermesi**.
- Panel push aboneliği kuramadığında kendi fallback'ine (yoklama/SSE) düşer — bu **kasıtlıdır**.
- Uygulama kapalıyken garantili teslim isteniyorsa **native köprü** gerekir: Windows'ta WNS,
  macOS'ta APNs. Bu, kabuk tarafında ayrı bir arka plan servisi + sunucu tarafında ikinci bir
  gönderim hedefi demektir; **bu depoda YOKTUR** ve manuel kurulum ister.

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
- Yerel bildirim: `tauri-plugin-notification`.
- Uzaktan tetikleme için uygulama açıkken WebView'deki mevcut SSE/web-push zaten çalışır; kapalıyken
  push istenirse ayrı bir native köprü/agent gerekir.

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
| **Notification köprüsü** (zaten vardı) | Tepside çalışırken WebView canlıdır; panelin SSE/fetch beslemesi kesintisiz işler ve her bildirim native masaüstü bildirimi olur |

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

- **Bugün çalışan:** Web Push, PWA + masaüstü tarayıcı (iOS'ta yalnızca kurulu PWA).
- **Native kabukta garantili arka plan push:** FCM (Android) + APNs (iOS) — manuel kimlik + entegrasyon.
- Bu depo push **kodu/anahtarı içermez**; yalnızca kabuğu üretir.
