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
