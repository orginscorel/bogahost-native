# iOS — Derleme, İmzalama ve Dağıtım Kılavuzu

Bu belge, dört Bogahost uygulamasının (**Finans / DCIM / Chat / Görevler**) iOS
sürümlerini sıfırdan App Store'a taşımak için gereken **her adımı** anlatır.

## Bugünkü durum

| | Durum |
| --- | --- |
| Derleme altyapısı | ✅ Hazır — `.github/workflows/ios.yml` |
| Xcode projesi üretimi | ✅ Otomatik (`scripts/prepare.mjs`, taze checkout'ta `cap add ios`) |
| Info.plist izin metinleri | ✅ Otomatik (Türkçe) |
| Entitlements (push + universal link) | ✅ Otomatik, **pbxproj'a bağlı** |
| Sürüm / build numarası | ✅ Otomatik (`package.json` + CI run numarası) |
| IPA export + TestFlight | ✅ Kodlandı, secret bekliyor |
| **İmzalama sertifikaları** | ⛔ **EKSİK — Apple Developer hesabı gerekiyor** |

> **Tek eksik secret'lardır.** Aşağıdaki tablodaki secret'lar eklendiğinde
> **hiçbir kod değişikliği yapmadan** imzalı IPA üretilir. Secret yokken CI
> imzasız bir `.xcarchive` üretip **yeşil** kalır (derleme doğrulaması).

---

## 1. Apple Developer hesabı açma

1. <https://developer.apple.com/programs/enroll/> adresine gidin.
2. **Kuruluş (Organization)** olarak kaydolun — şirket adıyla yayınlamak ve
   birden çok geliştiriciyi yönetmek için gereklidir. Bireysel hesapta
   uygulama, geliştiricinin kendi adıyla yayımlanır.
3. Kuruluş kaydı için gerekenler:
   - **D‑U‑N‑S numarası** (ücretsiz, Dun & Bradstreet'ten alınır; 5–14 iş günü sürebilir — **en erken bunu başlatın**).
   - Şirketin yasal adı, adresi, web sitesi (bogahost.com).
   - Kayıt yapan kişinin şirketi bağlama yetkisi olduğunun teyidi.
4. Ücret: **yıllık 99 USD**.
5. Onay geldikten sonra <https://developer.apple.com/account> → **Membership
   details** ekranındaki **Team ID**'yi (10 karakter, örn. `A1B2C3D4E5`) not edin.
   Bu değer birçok yerde lazım olacak.

---

## 2. App ID / Bundle ID oluşturma (5 uygulama)

<https://developer.apple.com/account/resources/identifiers/list> → **+**

Her uygulama için **ayrı** bir App ID oluşturun (wildcard **kullanmayın**;
gerekçesi aşağıda):

| Uygulama | Bundle ID | Açıklama |
| --- | --- | --- |
| Finans | `com.bogahost.finans` | Bogahost Finans |
| DCIM | `com.bogahost.dcim` | Bogahost DCIM |
| Chat | `com.bogahost.chat` | Bogahost Chat |
| Görevler | `com.bogahost.task` | Bogahost Görevler |

Her App ID'de şu **Capabilities** kutularını işaretleyin:

- ☑ **Push Notifications** — `aps-environment` entitlement'ı için.
- ☑ **Associated Domains** — universal link (`applinks:*.bogahost.com`) için.

> ⚠️ **Wildcard App ID (`com.bogahost.*`) KULLANMAYIN.** Push Notifications ve
> Associated Domains wildcard App ID'lerde **desteklenmez**. Kullanırsanız
> imzalama geçer ama bildirimler ve universal link'ler sessizce çalışmaz.
> CI bu durumu tespit edip uyarı basar (`ios.yml` → "Wildcard profil").

Ardından App Store Connect'te (<https://appstoreconnect.apple.com>) her bundle
ID için bir **uygulama kaydı** açın (My Apps → + → New App).

---

## 3. Dağıtım sertifikası (Distribution Certificate)

macOS'lu bir makinede yapılması en kolayıdır.

**a) CSR üretin** (Anahtar Zinciri Erişimi / Keychain Access):
Menü → *Sertifika Yardımcısı* → *Bir Sertifika Yetkilisinden Sertifika İste* →
e‑posta + ortak ad girin, **"Diske kaydedildi"** seçin → `CertificateSigningRequest.certSigningRequest`.

**b) Sertifikayı oluşturun:**
<https://developer.apple.com/account/resources/certificates/list> → **+** →
**Apple Distribution** → CSR dosyasını yükleyin → `.cer` dosyasını indirin.

**c) `.p12`'ye dönüştürün:** `.cer`'e çift tıklayıp Keychain'e ekleyin →
Anahtar Zinciri'nde **"Apple Distribution: …"** girdisini bulun → sağ tık →
**Dışa Aktar** → biçim `.p12` → **bir parola belirleyin** (bu parola
`APPLE_CERT_PASSWORD` secret'ı olacak).

> Aynı `.p12` **beş uygulamanın hepsi** için kullanılır. Sertifika bundle ID'ye
> bağlı değildir; profil bağlıdır.

---

## 4. Provisioning Profile (uygulama başına 4 adet)

<https://developer.apple.com/account/resources/profiles/list> → **+**

Her bundle ID için birer profil üretin:

- **Dağıtım türü:** *App Store Connect* (TestFlight + App Store için)
  veya *Ad Hoc* (kayıtlı cihazlara dahili dağıtım için).
- **App ID:** ilgili bundle ID.
- **Sertifika:** 3. adımda ürettiğiniz Apple Distribution sertifikası.
- İndirin → `.mobileprovision`.

Sonuçta elinizde 4 dosya olur:
`Bogahost_Finans.mobileprovision`, `…_DCIM…`, `…_Chat…`, `…_Task…`.

---

## 5. Dosyaları base64'e çevirip GitHub secret'a ekleme

macOS/Linux'ta:

```bash
# Sertifika (tek dosya, hepsi için ortak)
base64 -i Certificates.p12 | pbcopy          # macOS
base64 -w0 Certificates.p12                  # Linux

# Profiller (her biri ayrı secret)
base64 -w0 Bogahost_Finans.mobileprovision
base64 -w0 Bogahost_DCIM.mobileprovision
base64 -w0 Bogahost_Chat.mobileprovision
base64 -w0 Bogahost_Task.mobileprovision
```

> `-w0` **şart** — satır kaydırma olursa `base64 -d` çözemez. macOS `base64`
> zaten tek satır üretir.

Çıktıyı GitHub'da: **Settings → Secrets and variables → Actions → New repository secret**.

### Secret tablosu (tek liste)

| Secret adı | Zorunlu mu | İçerik | Nereden |
| --- | --- | --- | --- |
| `APPLE_CERT` | **Evet** | Dağıtım sertifikası `.p12` → base64 | Adım 3 |
| `APPLE_CERT_PASSWORD` | **Evet** | `.p12` dışa aktarım parolası | Adım 3‑c |
| `TEAM_ID` | **Evet** | 10 karakterlik Team ID (ör. `A1B2C3D4E5`) | Adım 1 |
| `PROVISIONING_PROFILE_FINANS` | **Evet** | Finans `.mobileprovision` → base64 | Adım 4 |
| `PROVISIONING_PROFILE_DCIM` | **Evet** | DCIM `.mobileprovision` → base64 | Adım 4 |
| `PROVISIONING_PROFILE_CHAT` | **Evet** | Chat `.mobileprovision` → base64 | Adım 4 |
| `PROVISIONING_PROFILE_TASK` | **Evet** | Görevler `.mobileprovision` → base64 | Adım 4 |
| `PROVISIONING_PROFILE` | Hayır | Ortak/yedek profil. Uygulamaya özel secret yoksa kullanılır. | — |
| `APPSTORE_KEY_ID` | Hayır* | App Store Connect API anahtar kimliği | Adım 6 |
| `APPSTORE_ISSUER_ID` | Hayır* | App Store Connect Issuer ID (UUID) | Adım 6 |
| `APPSTORE_PRIVATE_KEY` | Hayır* | `AuthKey_XXXX.p8` içeriği (ham metin **veya** base64) | Adım 6 |

\* TestFlight'a **otomatik yükleme** ve **otomatik imzalama** istiyorsanız gerekir.
Yoksa CI manuel imzalamaya düşer ve IPA'yı artifact olarak bırakır (elle yüklersiniz).

Repo **variable** (secret değil, isteğe bağlı):

| Variable | Ne işe yarar |
| --- | --- |
| `BUILD_NUMBER_OFFSET` | Build numarasını sabit bir sayı kadar kaydırır. Mağazaya daha yüksek numaralı bir build yüklendiyse (veya CI geçmişi sıfırlandıysa) kullanın. |

---

## 6. App Store Connect API anahtarı (isteğe bağlı ama önerilir)

TestFlight'a **CI'dan otomatik yükleme** ve **otomatik imzalama** sağlar.

1. <https://appstoreconnect.apple.com/access/integrations/api> → **Keys** sekmesi.
2. **+** → isim verin → Erişim: **App Manager**.
3. `.p8` dosyasını indirin — **yalnızca bir kez indirilebilir**, kaybederseniz
   yenisini üretmeniz gerekir.
4. Aynı ekrandaki **Key ID** ve **Issuer ID** değerlerini not edin.
5. Üçünü secret olarak ekleyin (yukarıdaki tablo).

**İmzalama stili bu secret'lara göre otomatik seçilir:**

| ASC API anahtarı | İmzalama | Ne olur |
| --- | --- | --- |
| Var | `automatic` | Xcode profilleri kendi çözer/yeniler; TestFlight yüklemesi mümkün. |
| Yok | `manual` | Yüklenen `.mobileprovision` + sertifika birebir kullanılır. IPA artifact olarak iner. |

İkisi de kodlanmıştır; seçim **yalnız secret'lara bakar**.

---

## 7. Derlemeyi çalıştırma

**Actions → iOS (Archive + IPA) → Run workflow**

| Girdi | Seçenekler | Anlamı |
| --- | --- | --- |
| `export_method` | `app-store-connect` | TestFlight / App Store (varsayılan) |
| | `release-testing` / `ad-hoc` | Kayıtlı cihazlara dahili dağıtım |
| | `development` | Geliştirici cihazları |
| | `enterprise` | Yalnız Apple Developer Enterprise Program |
| `upload_testflight` | ✓ / ✗ | IPA'yı doğrudan TestFlight'a yükle (ASC API anahtarı şart) |

`v*` biçiminde bir sürüm etiketi push etmek de workflow'u tetikler
(varsayılan: `app-store-connect`, TestFlight yüklemesi **kapalı**).

> macOS runner'ları private repoda dakika kotasını **10 kat** hızlı tüketir;
> bu yüzden iOS derlemesi her branch push'unda değil, yalnız etiket ve elle çalışır.

### Sürüm numaraları nereden gelir

| Alan | Kaynak |
| --- | --- |
| `MARKETING_VERSION` (kullanıcıya görünen) | kök `package.json` → `version` |
| `CURRENT_PROJECT_VERSION` (build numarası) | GitHub Actions `run_number` (+ `BUILD_NUMBER_OFFSET`) |

`scripts/prepare.mjs` bu değerleri `project.pbxproj`'a yazar; `Info.plist`
içindeki `$(MARKETING_VERSION)` / `$(CURRENT_PROJECT_VERSION)` yer tutucuları
derleme sırasında bunlarla dolar. App Store **her yüklemede daha büyük** bir
build numarası ister — `run_number` her çalıştırmada arttığı için bu sağlanır.

---

## 8. TestFlight'a yükleme

**Otomatik:** ASC API secret'ları varken workflow'u `upload_testflight: ✓` ile
çalıştırın. Yükleme sonrası Apple'ın işlemesi 5–30 dakika sürer.

**Elle:** Actions çalıştırmasının `ios-<uygulama>` artifact'ından `.ipa`'yı
indirin ve **Transporter** uygulamasıyla (Mac App Store'da ücretsiz) yükleyin.

Sonrasında App Store Connect → **TestFlight**:

1. Build işlendiğinde **"Manage"** ile ihracat uyumluluğu sorusu çıkabilir —
   `ITSAppUsesNonExemptEncryption=false` anahtarını Info.plist'e zaten
   eklediğimiz için **normalde çıkmaz**.
2. **Internal Testing** grubu oluşturun, ekip üyelerini ekleyin — inceleme
   beklemeden anında dağıtılır (en fazla 100 kişi).
3. **External Testing** için Apple'ın **Beta App Review**'undan geçmesi gerekir
   (genelde 1 gün).

---

## 9. App Store inceleme notları

Uygulama kaydında (App Store Connect → App Review Information) mutlaka doldurun:

- **Demo hesabı:** kullanıcı adı + parola. **Zorunludur** — uygulama giriş
  ekranıyla açıldığı için inceleme uzmanı içeri giremezse **reddedilir**.
  2FA aktifse ya inceleme hesabında kapatın ya da "Notes" alanına nasıl
  geçileceğini yazın (Guideline 2.1 ihlali sayılmaması için).
- **Notes:** uygulamanın kurum içi bir operasyon paneli olduğunu, beş sistemin
  ne yaptığını ve demo hesapla nelerin görülebileceğini açıklayın.

### ⚠️ Reddedilme riskleri (gerçekçi olalım)

**1) Guideline 4.2 — "Minimum Functionality"**
Bu uygulamalar canlı siteyi yükleyen WebView kabuklarıdır. Apple, *"sadece web
sitenizi gösteren"* uygulamaları **düzenli olarak reddeder**. Bu **en olası
ret sebebidir.** Azaltma önerileri:

- **Push bildirimleri** — native APNs bildirimi en güçlü savunmadır; zaten
  altyapısı var (`docs/PUSH.md`). İnceleme notunda **açıkça belirtin**.
- **Kamera / mikrofon** — Chat'teki görüntülü/sesli arama native izinler
  kullanır; bu native bir yetenektir.
- **Dosya indirme ve "Fotoğraflara kaydet"** — native paylaşım sayfası.
- **Offline ekranı** — bağlantı yokken anlamlı bir ekran (bkz.
  `pwa-enhancements/offline.html`); "bağlantı yok" beyaz sayfası ret sebebidir.
- **Uygulamalar arası geçiş** — beş sistem arasında kabuk içi geçiş.
- **Universal link** — `https://finans.bogahost.com/...` bağlantıları
  uygulamada açılır.
- İnceleme notuna şunu yazın: *"Bu, Bogahost personelinin kullandığı kurum içi
  bir operasyon aracıdır; genel tüketiciye yönelik bir web sitesi kopyası
  değildir."*

**Yedek plan:** 4.2'den reddedilirse en temiz çözüm **Apple Developer
Enterprise Program** (yıllık 299 USD, App Store dışı kurum içi dağıtım) veya
**Ad Hoc / TestFlight ile dahili dağıtım**'dır. `ios.yml` bu yöntemlerin
üçünü de destekler (`export_method`).

**2) Guideline 2.1 — çalışmayan demo hesabı.** Yukarıya bakın.

**3) `voip` arka plan modu.** Bilinçli olarak **kaldırıldı**. Apple, `voip`
bildiren uygulamalardan PushKit + CallKit entegrasyonu şart koşar; bu bir
WebView kabuğu olduğu için ret sebebi olurdu. Arama sesi için `audio` modu yeterli.

**4) İzin metinleri.** Dört uygulamada da kamera/mikrofon/fotoğraf açıklamaları
Türkçe ve somut. Boş veya genel ("uygulama kamerayı kullanır") metinler
reddedilir. Uygulama bir izni **hiç** kullanmıyorsa açıklama bulunması sorun
değildir; kullanıp açıklama bulunmaması **çökme** sebebidir.

---

## 10. Push (APNs) kurulumu

Native iOS push için **ayrı** bir anahtar gerekir (imzalama sertifikasından farklı):

1. <https://developer.apple.com/account/resources/authkeys/list> → **+**
2. **Apple Push Notifications service (APNs)** kutusunu işaretleyin → Continue → Register.
3. `AuthKey_XXXXXXXXXX.p8` dosyasını indirin — **yalnız bir kez indirilebilir.**
4. Not edin: **Key ID** (dosya adındaki 10 karakter) ve **Team ID**.
5. Bu üçünü (`.p8` + Key ID + Team ID) push sunucusuna tanımlayın:
   - Doğrudan APNs kullanıyorsanız: JWT üretimi için üçü de gerekir.
   - Firebase üzerinden gidiyorsanız: Firebase Console → Project Settings →
     Cloud Messaging → **APNs Authentication Key** → `.p8`'i yükleyin.

`App.entitlements` içindeki `aps-environment` değeri **`production`**'dır.
TestFlight ve App Store dağıtımları production APNs sunucusunu kullanır — bu
doğrudur. (Yalnızca Xcode'dan cihaza doğrudan kurarken `development` gerekir.)

Mimarinin tamamı: [`PUSH.md`](PUSH.md)

---

## 11. Universal Link (.well-known/apple-app-site-association)

Bir bağlantıya tıklandığında Safari yerine uygulamanın açılmasını sağlar.

**a) Team ID'yi doldurun.** Her `capacitor/<app>/ios-overrides/apple-app-site-association`
dosyasındaki `TEAMID` yer tutucusunu gerçek Team ID ile değiştirin:

```json
{
  "applinks": {
    "apps": [],
    "details": [
      { "appID": "A1B2C3D4E5.com.bogahost.finans", "paths": ["*"] }
    ]
  }
}
```

**b) Canlı sunucuya koyun.** Dört dosya, dört ayrı alan adına:

| Dosya | Yayınlanacağı adres |
| --- | --- |
| `capacitor/finans/ios-overrides/apple-app-site-association` | `https://finans.bogahost.com/.well-known/apple-app-site-association` |
| `capacitor/dcim/…` | `https://dcim.bogahost.com/.well-known/apple-app-site-association` |
| `capacitor/chat/…` | `https://chat.bogahost.com/.well-known/apple-app-site-association` |
| `capacitor/task/…` | `https://task.bogahost.com/.well-known/apple-app-site-association` |

Kurallar (hepsi zorunlu):

- Dosya adı **uzantısız** (`.json` **ekleme**).
- `Content-Type: application/json`.
- **HTTPS**, geçerli sertifika, **yönlendirme yok** (301/302 kabul edilmez).
- Kimlik doğrulaması / giriş duvarı arkasında olmamalı.
- Cloudflare arkasında olduğu için: `.well-known/` yolunun challenge'a
  takılmadığından emin olun (bkz. hafıza notu *bogahost cf blocks bridge*).

**c) Doğrulayın:**

```bash
curl -sI https://finans.bogahost.com/.well-known/apple-app-site-association
# 200 OK + Content-Type: application/json bekleniyor, 301/302 DEĞİL
```

Apple'ın CDN'i dosyayı önbelleğe alır; değişiklik sonrası uygulamayı silip
yeniden kurmak gerekebilir.

Android tarafındaki karşılığı `assetlinks.json`'dır
(`capacitor/<app>/android-overrides/assetlinks.json`, aynı `.well-known` yolu).

---

## 12. Sorun giderme

| Belirti | Sebep / çözüm |
| --- | --- |
| CI "Unsigned iOS build" uyarısı veriyor | `APPLE_CERT` / profil / `TEAM_ID` secret'ları eksik. Beklenen davranış. |
| "Sertifika okunamadı" | `APPLE_CERT_PASSWORD` yanlış ya da `.p12` kod imzalama kimliği içermiyor. |
| "Profil uyuşmuyor" | `PROVISIONING_PROFILE_<APP>` başka bir bundle ID'nin profili. |
| "Wildcard profil" uyarısı | Wildcard App ID kullanılmış → push ve universal link çalışmaz. Uygulamaya özel App ID üretin. |
| Bildirim gelmiyor | `aps-environment` production ↔ APNs sunucusu uyumsuz olabilir; ya da entitlement imzaya gömülmemiş (bu artık otomatik). |
| Universal link uygulamayı açmıyor | AASA dosyası 301 dönüyor, `Content-Type` yanlış veya Team ID hâlâ `TEAMID`. |
| App Store "build numarası kullanımda" | `BUILD_NUMBER_OFFSET` variable'ını artırın. |

---

## İlgili belgeler

- [`BUILD.md`](BUILD.md) — genel derleme
- [`SIGNING.md`](SIGNING.md) — tüm platformların imzalama özeti
- [`PUSH.md`](PUSH.md) — bildirim mimarisi
- [`PWA.md`](PWA.md) — PWA katmanı (şu an canlı olan)
- [`../capacitor/README.md`](../capacitor/README.md) — Capacitor kabuk mimarisi
