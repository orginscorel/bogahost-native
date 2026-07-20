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

### Sınırlar (net olarak)

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

## Özet

- **Bugün çalışan:** Web Push, PWA + masaüstü tarayıcı (iOS'ta yalnızca kurulu PWA).
- **Native kabukta garantili arka plan push:** FCM (Android) + APNs (iOS) — manuel kimlik + entegrasyon.
- Bu depo push **kodu/anahtarı içermez**; yalnızca kabuğu üretir.
