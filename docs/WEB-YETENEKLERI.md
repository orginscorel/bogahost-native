# Web yeteneklerinin native kabuktaki durumu

Panellerde çalışan her web yeteneğinin **Tauri masaüstü kabuğunda** ve **Capacitor mobil
kabuğunda** çalışıp çalışmadığının uçtan uca denetimi (v1.6.0).

> Bu tablodaki "çalışır" ifadeleri **kaynak kod ve satıcı belgelerinden** doğrulanmıştır,
> çalışan bir binary üzerinde test edilmemiştir (bu depoda derleme yapılmaz).
> "Mümkün değil" satırları üst akış (Tauri/wry/WebKit) sınırlarıdır ve kabuk tarafından
> aşılamaz — gerekçeleri ve kaynakları aşağıdadır.

## Özet tablo

| # | Yetenek | macOS (WKWebView) | Windows (WebView2) | Android/iOS | Ne yapıldı |
|---|---|---|---|---|---|
| 1 | `getUserMedia` (kamera/mik) | ✅ **düzeltildi** | ✅ çalışır | ✅ çalışır | Info.plist + entitlements |
| 2 | `getDisplayMedia` (ekran) | ⚠️ kısmen | ✅ çalışır | ❌ platformda yok | Hata mesajı + ayar düğmesi |
| 3 | WebRTC / STUN / TURN | ✅ çalışır | ✅ çalışır | ✅ çalışır | CSP engel değil (aşağıya bakın) |
| 4 | SSE / `EventSource` | ✅ çalışır | ✅ çalışır | ✅ çalışır | Değişiklik gerekmedi |
| 5 | WebSocket | ✅ çalışır | ✅ çalışır | ✅ çalışır | Değişiklik gerekmedi |
| 6 | `<input type=file>` | ✅ çalışır | ✅ çalışır | ✅ çalışır | Değişiklik gerekmedi |
| 7 | Sürükle-bırak yükleme | ✅ **düzeltildi** | ✅ **düzeltildi** | — | `disable_drag_drop_handler()` |
| 8 | Panoya kopyala | ✅ çalışır | ⚠️ yedekli | ✅ çalışır | `execCommand` yedeği |
| 8b | Panodan **oku** | ✅ çalışır | ❌ reddedilir | ✅ çalışır | Açık hata mesajı |
| 9 | Ses çalma (zil/bip) | ✅ **düzeltildi** | ✅ **düzeltildi** | ✅ çalışır | Ses kilidi + autoplay bayrağı |
| 10 | Tam ekran API | ⚠️ **yedekli** | ✅ çalışır | ✅ çalışır | Pencere tam ekran yedeği |
| 11 | localStorage / çerez kalıcılığı | ✅ kalıcı | ✅ kalıcı | ✅ kalıcı | — (paylaşım notu ↓) |
| 11b | 4 uygulama arası oturum paylaşımı | ❌ **mümkün değil** | ✅ çalışır | — | Belgelendi |
| 12 | Geolocation | ❌ mümkün değil | ❌ mümkün değil | ✅ çalışır | Belgelendi |
| 13 | Yazdırma | ✅ çalışır | ✅ çalışır | — | v1.5.0, doğrulandı |
| 14 | Bildirimler (uygulama açıkken) | ✅ **düzeltildi** | ✅ **düzeltildi** | ✅ çalışır | Besleme köprüsü |
| 14b | Bildirimler (uygulama kapalıyken) | ❌ mümkün değil | ❌ mümkün değil | ✅ (FCM/APNs kurulursa) | `docs/PUSH.md` |
| 15 | Dosya indirme | ✅ çalışır | ✅ çalışır | ✅ çalışır | v1.5.0, doğrulandı |

---

## Ayrıntılar

### 1. Kamera ve mikrofon — ASIL SORUN, ÇÖZÜLDÜ

**macOS kök nedeni iki katmanlıydı:**

1. **TCC (Transparency, Consent & Control).** `Info.plist`'te `NSCameraUsageDescription`
   veya `NSMicrophoneUsageDescription` yoksa macOS, kameraya erişmeye çalışan uygulamayı
   **izin sorusu göstermeden anında öldürür**. Kullanıcının gördüğü "hiç tepki vermiyor"
   davranışı buydu.
2. **Hardened Runtime.** Tauri'de `bundle.macOS.hardenedRuntime` **varsayılan olarak
   `true`**'dur. Entitlement verilmeden kamera/mikrofon çekirdek tarafından reddedilir.

**Yapılanlar:**
- `src-tauri/Info.plist` — bundler bu dosyayı **otomatik bulur ve birleştirir**;
  `tauri.conf.json`'da ayrıca alan tanımlamaya gerek yoktur.
- `src-tauri/Bogahost.entitlements` — `com.apple.security.device.camera`,
  `com.apple.security.device.audio-input` (+ ağ, dosya, JIT). `bundle.macOS.entitlements`
  ile bağlandı.

**WKWebView izin işleyicisi gerekli mi? HAYIR.** wry, `WKUIDelegate`'in
`webView:requestMediaCapturePermissionForOrigin:...` yöntemini **zaten uyguluyor** ve
koşulsuz `WKPermissionDecision::Grant` döndürüyor (wry 0.22.0'dan beri,
`src/wkwebview/class/wry_web_view_ui_delegate.rs`). Yani WebKit katmanı sorun değildi;
**tek eksik işletim sistemi katmanıydı.**

> Tauri 2'de `WebviewWindowBuilder` üzerinde medya izni için **hiçbir genel API yoktur.**
> wry'nin `dev` dalında `with_permission_handler` eklendi ancak yayımlanmış 0.55.1'de
> **yok** ve Tauri onu hiç çağırmıyor. Bu bilgi doğrulanmıştır, tahmin değildir.

**Windows'ta durum:** WebView2 kendi izin penceresini gösterir; kod gerekmez.
**Ancak** kullanıcı bir kez "Engelle" derse seçim **profile yazılır** ve pencere bir daha
çıkmaz (`tauri-apps/tauri#5042`, 2022'den beri açık). Tek kurtarma yolu uygulama kapalıyken
`%LOCALAPPDATA%\<paket>\EBWebView\Default\Preferences` dosyasını silmektir.

**Bu tuzağı kapatan kod BİLEREK EKLENMEDİ.** Gerekçe: `ICoreWebView2.add_PermissionRequested`
kancası `webview2-com` + `windows` crate'lerini bağımlılık olarak eklemeyi gerektirir ve
bu depoda **derleme yapılamadığı** için Windows CI'ını kırma riski taşır. Kamera Windows'ta
zaten çalıştığından, kazanç yalnızca bu kenar durumdur. Eklenmek istenirse doğrulanmış
imzalar:

```toml
[target.'cfg(windows)'.dependencies]
webview2-com = "0.38"          # Tauri 2.11 ve wry 0.55 ile AYNI sürüm olmalı
windows = { version = "0.61", features = ["Win32_Foundation"] }
```

```rust
#[cfg(windows)]
fn auto_allow_media(w: &tauri::WebviewWindow) -> tauri::Result<()> {
  use webview2_com::PermissionRequestedEventHandler;
  use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CAMERA,
    COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, COREWEBVIEW2_PERMISSION_STATE_ALLOW,
  };
  w.with_webview(|pw| unsafe {
    let Ok(core) = pw.controller().CoreWebView2() else { return };
    let mut token = 0i64;
    let _ = core.add_PermissionRequested(
      &PermissionRequestedEventHandler::create(Box::new(|_wv, args| {
        let Some(args) = args else { return Ok(()) };
        let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
        args.PermissionKind(&mut kind)?;
        if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA
          || kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE {
          args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
        }
        Ok(())
      })),
      &mut token,
    );
  })
}
```

**Linux (WebKitGTK):** getUserMedia **çalışmaz.** `WebKitSettings:enable-media-stream`
varsayılan `FALSE` ve wry hiç kurmuyor; ayrıca işlenmemiş `UserMediaPermissionRequest`
varsayılan olarak reddedilir. Bu depo Linux masaüstü hedefi üretmediği için etkisizdir.

### 2. Ekran paylaşımı (`getDisplayMedia`)

WKWebView `getDisplayMedia`'yı **destekler** (yaygın "Apple desteklemiyor" iddiası
yanlıştır). Ancak wry'nin getUserMedia temsilcisini uygulaması, WebKit'in ekran yakalamayı
da aynı yola sokmasına ve **reddetmesine** sebep oluyordu. WebKit bunu 2024-05-31'de
düzeltti; **macOS 14.0–14.5 aralığı riskli kabul edilmelidir** (`wry#1195` hâlâ açık).

Kabuk tarafında yapılan: başarısızlıkta sessiz kalınmıyor, sebebi anlatan kutu ve
**Ekran Kaydı** gizlilik ayarını açan düğme gösteriliyor.

### 3. WebRTC ve CSP — önemli düzeltme

**`tauri.conf.json` içindeki CSP uzak panellere UYGULANMAZ.** Tauri, CSP'yi yalnızca
*kendi* varlık protokolüyle sunduğu içeriğe HTTP başlığı olarak ekler
(`crates/tauri/src/protocol/tauri.rs`). `WebviewUrl::External(...)` ile yüklenen sayfa
doğrudan WebView'in ağ yığınından geçer — Tauri araya giremez.

Sonuç: panellerin `wss://` ve TURN bağlantılarını **sunucudan gelen CSP** yönetir, kabuk
değil. Bu yüzden kabuk CSP'si WebRTC için **genişletilmedi** — genişletmek hiçbir şeyi
çözmez, yalnızca yerel splash sayfasının korumasını gevşetirdi. (Ayrıca ICE/STUN/TURN
trafiği `connect-src` kapsamında değildir.)

### 7. Sürükle-bırak — sessiz bozukluk, düzeltildi

Tauri'nin kendi sürükle-bırak işleyicisi **varsayılan olarak açık** ve işletim sistemi
olayını yutuyor. wry'nin kendi belgesi sonucu açıkça söylüyor: *"if you do block this
behavior, it won't be possible to drop files on `<input type="file">` forms."*

Tauri'nin belgesi bunu yalnızca Windows'a özgüymüş gibi anlatır; **doğru değildir** —
wry üç arka uçta da (webview2 `IDropTarget`, wkwebview `performDragOperation`,
webkitgtk) araya giriyor. Kabuk sürükle-bırakla hiçbir şey yapmadığı için işleyici
kapatıldı; hiçbir özellik kaybedilmedi, panelin yükleme alanları çalışır hale geldi.

### 9. Ses — zil ve bip

Windows'ta `--autoplay-policy=no-user-gesture-required` eklendi. ⚠️
`additional_browser_args` Tauri'nin varsayılanlarının **yerine geçer** (`unwrap_or_else`),
bu yüzden varsayılan `--disable-features=...` listesi aynen tekrar yazıldı.

⚠️ Aynı veri klasörünü paylaşan tüm webview'ler **aynı argümanları** kullanmalıdır
(`tauri#11144`); bu yüzden ana pencere, splash ve önizleme pencereleri ortak
`WEBVIEW2_BROWSER_ARGS` sabitini kullanır.

macOS'ta karşılığı **yoktur**: wry'de `with_autoplay` vardır ama Tauri 2 dışarı açmaz.
Bunun yerine ilk kullanıcı hareketinde `AudioContext` uyandırılıp sessiz bir ses çalınır.

### 10. Tam ekran

macOS'ta HTML `element.requestFullscreen()` **çalışmaz**: wry ilgili WKPreferences
anahtarını (`fullScreenEnabled`, **özel/private KVC anahtarı**) yalnızca `fullscreen`
özelliği açıkken kurar; Tauri'de bunun tek yolu `macos-private-api`'dir. Özel API
kullanmak App Store riski taşıdığı için **açılmadı**. Bunun yerine istek başarısız
olursa **pencere** tam ekran yapılır (`bogahost_set_fullscreen`).

### 11b. macOS'ta oturum paylaşımı — MÜMKÜN DEĞİL

`WebviewWindowBuilder::data_directory()` yalnızca **Windows ve Linux** arka uçlarında
etkilidir. WKWebView'de karşılığı yoktur ve wry değeri **sessizce yok sayar**; macOS'ta
her uygulama `WKWebsiteDataStore::defaultDataStore` kullanır.

**Sonuç:** macOS'ta Finans / DCIM / Chat / Görevler kabukları çerez paylaşmaz ve her
birinde ayrı giriş yapılır. Windows'ta paylaşım çalışır. Alternatif
(`with_data_store_identifier`, macOS 14+) Tauri tarafından dışarı açılmıyor.

### 12. Geolocation

Masaüstünde çalışmaz. wry, WKWebView için CoreLocation temsilcisini bağlamıyor
(`wry#81`, 2020'den beri açık); WebView2'de `PermissionRequested` tetiklenir ama Tauri
ele almadığı için reddedilir. Paneller konum kullanmıyor; kullanmaya başlanırsa mobil
kabuklarda çalışacak, masaüstünde çalışmayacaktır.

### 14. Bildirimler

Ayrıntı ve kök neden analizi için **`docs/PUSH.md`** — v1.6.0'ın en önemli düzeltmesidir.

---

## Mobil (Capacitor) tarafı

Kamera/mikrofon için gerekenler **zaten yerindeydi** ve doğrulandı:

- `android-overrides/AndroidManifest.xml`: `CAMERA`, `RECORD_AUDIO`,
  `MODIFY_AUDIO_SETTINGS` + `uses-feature ... required="false"`.
- `ios-overrides/Info.plist.partial.xml`: `NSCameraUsageDescription`,
  `NSMicrophoneUsageDescription`, `NSPhotoLibrary*`, `UIBackgroundModes` (audio/voip).

Android WebView'de `getUserMedia`, `WebChromeClient.onPermissionRequest`'in **grant**
etmesini gerektirir. Capacitor 7 köprüsü bunu runtime izinleri verildiğinde ele alır;
alamazsa `BridgeActivity` alt sınıfı gerekir — bkz. `android-overrides/README.md`.
