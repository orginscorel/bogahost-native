# Otomatik Güncelleme (Updater) — Masaüstü uygulamaları

Windows/macOS (Tauri) uygulamaları **yeni sürümü kendisi indirip kurar**.
Bu belge mimariyi, `latest.json` şemasını, yeni sürüm yayınlama adımlarını,
gereken GitHub secret'larını ve sorun gidermeyi anlatır.

> **Android/iOS:** Otomatik güncelleme YOKTUR. Android APK yalnızca
> `https://native.bogahost.com/downloads/` altına yüklenir; kullanıcı elle kurar.

---

## 1. Mimari — kullanıcı tarafında ne oluyor?

```
Uygulama açılır
   │
   ├─ (sessiz) tauri-plugin-updater → https://native.bogahost.com/updates/<app>/<target>/<arch>/latest.json
   │        │
   │        ├─ Güncelleme YOK  → hiçbir şey gösterilmez
   │        │
   │        └─ Güncelleme VAR  → ONAY DİYALOĞU
   │                             "Yeni sürüm X hazır (yüklü: Y). Şimdi kurulsun mu?"
   │                                 ├─ "Şimdi kur"   → indir → imza doğrula → kur → YENİDEN BAŞLAT
   │                                 └─ "Daha sonra"  → hiçbir kayıt tutulmaz,
   │                                                    BİR SONRAKİ AÇILIŞTA TEKRAR SORULUR
   │
   └─ Updater kullanılamıyorsa (eklenti yüklenmedi / pubkey PLACEHOLDER / ağ hatası)
            → YEDEK YOL: https://bogahost.com/native/latest.json okunur,
              yalnızca BİLDİRİM gösterilir (v1.1.0 davranışı korunur).
```

- Tepsi (system tray) menüsündeki **"Güncellemeleri denetle"** aynı akışı **elle**
  tetikler. Elle tetiklenen denetimde güncelleme yoksa da bildirim gösterilir
  ("En güncel sürümü kullanıyorsunuz").
- **Hata hâlinde uygulama ASLA kilitlenmez/çökmez.** Tüm hatalar yutulur ve
  `stderr`'e `[<app>][updater] ...` biçiminde yazılır.
- Updater eklentisi `Builder` zincirinde değil, `setup()` içinde `handle.plugin(...)`
  ile kayıt edilir. Böylece `pubkey` PLACEHOLDER veya bozuk olduğunda eklenti
  yüklenmez, hata yutulur ve uygulama normal çalışmaya devam eder.

Kod: `tauri/<app>/src-tauri/src/lib.rs` → `run_update_flow`, `try_auto_update`,
`prompt_and_install`, `install_update`, `check_update_legacy`.
Dosya 4 uygulamada **birebir aynıdır**; yalnızca `APP_KEY`/`APP_TITLE` farklıdır.

---

## 2. Dağıtım sunucusu ve dizin düzeni

Dağıtım sunucusu: **https://native.bogahost.com/**
(docroot `/home/bogahost/public_html/native`, dizin listeleme kapalı,
`.json` no-cache, installer'lar cache'li). Aynı içerik `https://bogahost.com/native/`
üzerinden de erişilebilir.

```
/                                          ← FTP kullanıcısının kökü
├── index.html                             indirme sayfası (CI üretir)
├── latest.json                            ESKİ manifest — v1.1.0 istemcileri okur
├── downloads/
│   ├── finans-1.2.0-windows-x86_64-setup.exe
│   ├── finans-1.2.0-windows-x86_64-setup.exe.sig
│   ├── finans-1.2.0-darwin-aarch64.app.tar.gz
│   ├── finans-1.2.0-darwin-aarch64.app.tar.gz.sig
│   ├── finans-1.2.0-macos.dmg
│   └── finans-1.2.0-android.apk
└── updates/
    ├── downloads.json                     index.html'in durum dosyası
    └── <app>/                             app = finans | dcim | chat | task
        ├── latest.json                    YEDEK endpoint (birleşik)
        ├── windows/x86_64/latest.json     BİRİNCİL endpoint
        └── darwin/aarch64/latest.json     BİRİNCİL endpoint
```

### Neden per-target/arch dosyalar birincil?

`windows.yml` ve `macos.yml` **ayrı ayrı** koşar. Ortak tek bir `latest.json`'ı
ikisi de yazsaydı, sonra koşan diğerinin platform kaydını **silerdi**.
Per-arch dosyalarını yalnızca kendi workflow'u yazdığı için çakışma olmaz.

Birleşik `updates/<app>/latest.json` yalnızca **sunucudaki kopya başarıyla
okunabilirse** güncellenir (okunup diğer platformun kaydıyla birleştirilir).
Okunamazsa dosyaya **dokunulmaz** ve uyarı basılır — otomatik güncelleme
birincil endpoint üzerinden çalışmaya devam eder.

### Uygulama tarafındaki endpoint tanımı

`tauri/<app>/src-tauri/tauri.conf.json`:

```json
"plugins": {
  "updater": {
    "pubkey": "__TAURI_UPDATER_PUBKEY__",
    "endpoints": [
      "https://native.bogahost.com/updates/finans/{{target}}/{{arch}}/latest.json",
      "https://native.bogahost.com/updates/finans/latest.json"
    ],
    "windows": { "installMode": "passive" }
  }
}
```

`{{target}}` → `windows` | `darwin`, `{{arch}}` → `x86_64` | `aarch64` (Tauri doldurur).
Tauri endpoint'leri **sırayla** dener; ilki başarısız olursa ikincisine düşer.

---

## 3. `latest.json` şeması (Tauri updater)

```json
{
  "version": "1.2.0",
  "notes": "Otomatik güncelleme eklendi.",
  "pub_date": "2026-07-21T09:12:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6...",
      "url": "https://native.bogahost.com/downloads/finans-1.2.0-darwin-aarch64.app.tar.gz"
    },
    "windows-x86_64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6...",
      "url": "https://native.bogahost.com/downloads/finans-1.2.0-windows-x86_64-setup.exe"
    }
  }
}
```

| Alan | Zorunlu | Açıklama |
| --- | --- | --- |
| `version` | evet | Yayınlanan sürüm. Uygulamanın `CARGO_PKG_VERSION`'ından büyükse güncelleme sunulur. |
| `notes` | hayır | Onay diyaloğunda gösterilir. |
| `pub_date` | hayır | ISO 8601 (RFC 3339). |
| `platforms.<target>-<arch>.signature` | evet | İlgili `.sig` dosyasının **içeriği** (dosya yolu değil). |
| `platforms.<target>-<arch>.url` | evet | Installer'ın anonim erişilebilir tam URL'i. |

Desteklenen platform anahtarları: `windows-x86_64`, `darwin-aarch64`, `darwin-x86_64`.

### Eski (yedek) manifest — `https://bogahost.com/native/latest.json`

v1.1.0 istemcileri **hâlâ bu adresi okur**, bu yüzden adres ve şema
**değiştirilmemiştir**. CI her yayında bu dosyayı da günceller:

```json
{
  "version": "1.2.0",
  "notes": "...",
  "url": "https://native.bogahost.com/"
}
```

---

## 4. Gereken GitHub secret'ları

| Secret | Zorunlu mu? | Ne işe yarar | Yoksa ne olur |
| --- | --- | --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Otomatik güncelleme için **EVET** | Installer'ları minisign ile imzalar | Updater artifact/`.sig` üretilmez; build imzasız devam eder, uygulamalar yedek bildirim yoluna düşer |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Anahtar parolalıysa evet | Private key parolası | Parolasız anahtar kullanıldıysa **boş değerle** eklenmelidir |
| `TAURI_SIGNING_PUBLIC_KEY` | Hayır (alternatifi var) | CI, build öncesi `tauri.conf.json`'daki PLACEHOLDER'ı bununla değiştirir | Public key'i doğrudan `tauri.conf.json`'a yazıp commit'lemeniz gerekir |
| `DEPLOY_FTP_HOST` | Yayın için **EVET** | FTPS sunucu adresi | Yayın adımı ATLANIR (uyarı verir, CI kırmızıya dönmez) |
| `DEPLOY_FTP_USER` | Yayın için **EVET** | FTP kullanıcısı (kökü `public_html/native` olmalı) | ↑ aynı |
| `DEPLOY_FTP_PASS` | Yayın için **EVET** | FTP parolası | ↑ aynı |
| `WINDOWS_CERTIFICATE` / `..._PASSWORD` | Hayır | Authenticode (SmartScreen uyarısını kaldırır) | İmzasız `.msi`/`.exe` |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Hayır | Developer ID imzası + notarization | İmzasız DMG (Gatekeeper uyarısı) |
| `SIGNING_KEYSTORE_BASE64`, `KEY_ALIAS`, `STORE_PASSWORD`, `KEY_PASSWORD` | Hayır | Android imzalama | İmzasız APK |

**Hiçbiri tanımlı olmasa da CI YEŞİL kalır** — imzalama ve yayın adımları
"varsa çalışır, yoksa uyarı verip atlar" mantığındadır.

---

## 5. Anahtar çifti üretimi (TEK SEFERLİK)

Private key **üretilmez ve depoya konmaz**. Üretim için hazır workflow:

1. GitHub → **Actions** → **"Updater imzalama anahtarı üret"** → **Run workflow**.
   (İsterseniz bir parola girin; boş bırakabilirsiniz.)
2. Çalışma bitince **Summary** sayfasında **PUBLIC key** görünür (gizli değildir).
3. **Artifact** olarak `updater-private-key` indirilebilir. Private key
   **yalnızca burada** dışarı çıkar, log'a asla yazılmaz.
4. Secret'ları ekleyin:
   - `TAURI_SIGNING_PRIVATE_KEY` = indirilen `bogahost-updater.key` dosyasının **tüm içeriği**
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = kullandığınız parola (parolasızsa **boş** değer)
   - `TAURI_SIGNING_PUBLIC_KEY` = özetteki public key
     *(alternatif: 4 uygulamanın `tauri.conf.json > plugins.updater.pubkey` alanındaki
     `__TAURI_UPDATER_PUBKEY__` yerine yazıp commit'leyin — public key gizli değildir)*
5. Artifact'ı **silin**; private key'i güvenli bir kasada saklayın (ör. Bilgi Kasası).

> **UYARI — anahtar kaybı geri dönüşsüzdür.** Private key kaybolursa kurulu
> uygulamalar yeni anahtarla imzalanan paketleri doğrulayamaz ve bir daha
> **otomatik güncelleme alamaz**; kullanıcıların yeni sürümü elle kurması gerekir.

---

## 6. Yeni sürüm yayınlama

### 6.1 Sürüm numarasını yükselt (hepsi aynı olmalı)

- `package.json` (kök) → `version`
- `apps.config.json` → `version`
- `tauri/<app>/package.json` → `version`
- `tauri/<app>/src-tauri/tauri.conf.json` → `version`
- `tauri/<app>/src-tauri/Cargo.toml` → `[package] version`  ← **uygulamanın "yüklü sürüm"ü buradan gelir**
- `capacitor/<app>/package.json` → `version`
- `CHANGELOG.md` → yeni bölüm

### 6.2 Tag at

```bash
git commit -am "v1.2.0"
git tag v1.2.0
git push origin native-apps --tags
```

### 6.3 Gerisi otomatik

```
tag push
  └─ release.yml → android.yml + ios.yml + windows.yml + macos.yml (secrets: inherit)
        ├─ build     : Tauri derler, TAURI_SIGNING_* varsa .sig üretir
        ├─ publish   : installer + .sig + latest.json üretir → FTPS ile yükler
        └─ GitHub Release (taslak) oluşturulur
```

Yayın adımının yaptıkları (`scripts/ci/publish-updates.mjs`):

1. Artifact'ları uygulamaya göre ayırır, güvenli adlara çevirir
   (`<app>-<sürüm>-<platform>.<uzantı>` — orijinal adlardaki **boşluklar** URL/FTP
   sorunu çıkardığı için).
2. `.sig` içeriğini okuyup `latest.json`'a **inline** yazar.
3. Per-target/arch manifestleri + (okunabiliyorsa) birleşik manifesti üretir.
4. Eski `latest.json`'ı ve `index.html`'i günceller.
5. `lftp` ile **FTPS** (`set ftp:ssl-force true`) üzerinden `mirror -R` yapar.
   **`--delete` KULLANILMAZ** — eski sürüm dosyaları ve diğer platformun
   kayıtları silinmez.

### 6.4 Kullanıcı tarafı

Kurulu uygulama bir sonraki açılışında güncellemeyi görür, onay ister,
kabul edilirse indirir + kurar + yeniden başlatır. **Elle işlem yoktur.**

---

## 7. Cloudflare — ZORUNLU ayar

`native.bogahost.com` Cloudflare **proxy (turuncu bulut)** arkasındayken
sunucu–sunucu istekleri challenge/**403** alır. Bu şunları bozar:

- **Updater'ın manifest ve installer indirmesi** (uygulama "güncelleme yok"
  sanır veya indirme başarısız olur),
- CI'ın sunucudaki mevcut `latest.json`/`downloads.json` dosyalarını okuması
  (birleşik manifest ve `index.html` güncellenmez).

**Çözüm — ikisinden biri ŞART:**

1. **DNS-only (gri bulut):** Cloudflare DNS kaydında `native` için proxy'yi
   kapatın. *(Önerilen — en basit ve en sağlam.)*
2. **Bypass kuralı:** `native.bogahost.com/*` için WAF / Bot Fight Mode ve
   "Browser Integrity Check" bypass eden bir Configuration Rule tanımlayın.

> İlgili geçmiş: `bogahost-cf-blocks-bridge` ve `bogahost-cf-403-cache` notları.
> Cloudflare 403'ü **30 güne kadar cache'leyebilir**; ayarı düzelttikten sonra
> `cf-cache-status` başlığını kontrol edin ve gerekirse Custom Purge yapın.

---

## 8. Sorun giderme

| Belirti | Olası sebep | Çözüm |
| --- | --- | --- |
| Uygulama hiç güncelleme görmüyor | `pubkey` hâlâ `__TAURI_UPDATER_PUBKEY__` | Eklenti yüklenmez, yedek bildirim yoluna düşer. Public key'i ekleyip yeniden yayınlayın. |
| CI'da "Updater kapali" uyarısı | `TAURI_SIGNING_PRIVATE_KEY` secret'ı yok | Bölüm 5'teki anahtar üretimini yapın. |
| CI'da "Yayin atlandi" uyarısı | `DEPLOY_FTP_*` eksik | FTP hesabı açıp secret'ları ekleyin. |
| `signature mismatch` / doğrulama hatası | Installer, `.sig` üretildikten **sonra** değiştirildi | Authenticode imzası installer baytlarını değiştirir. `windows.yml` bunu, Authenticode'dan sonra `tauri signer sign` ile `.sig`'i **yeniden üreterek** çözer. Elle imzalarsanız `.sig`'i de yenileyin. |
| Manifest 403 / CI "okunamadi" uyarısı | Cloudflare proxy | Bölüm 7. |
| Intel Mac güncelleme almıyor | Build yalnızca `aarch64` üretiyor | `macos-latest` runner arm64'tür. Universal binary için `tauri/<app>/package.json` → `"tauri:build:mac": "tauri build --target universal-apple-darwin --bundles dmg app"`. CI, `lipo -archs` ile mimariyi kendisi algılayıp `darwin-x86_64` kaydını da yazar. |
| Güncelleme indi, kurulmadı | Windows'ta installer izni / macOS'ta `.app` yazma izni | Uygulamayı `/Applications` (macOS) veya kullanıcı dizinine (`installMode: currentUser`) kurun. |
| Aynı anda iki onay diyaloğu | — | Olmaz: `UPDATE_PROMPT_OPEN` bayrağı tek diyalog garantiler. |

### Yayını elle doğrulama

```bash
curl -s https://native.bogahost.com/updates/finans/windows/x86_64/latest.json | jq
curl -s https://native.bogahost.com/updates/finans/darwin/aarch64/latest.json | jq
curl -sI https://native.bogahost.com/downloads/finans-1.2.0-windows-x86_64-setup.exe | head -3
curl -s https://bogahost.com/native/latest.json | jq   # eski istemciler
```

`signature` alanı boş veya eksikse `.sig` üretilmemiştir → bölüm 5'e bakın.

---

## 9. İlgili dosyalar

| Dosya | Görev |
| --- | --- |
| `tauri/<app>/src-tauri/src/lib.rs` | Güncelleme akışı (kontrol → diyalog → kur → yeniden başlat) + yedek manifest yolu |
| `tauri/<app>/src-tauri/tauri.conf.json` | `plugins.updater` (pubkey + endpoints) |
| `tauri/<app>/src-tauri/Cargo.toml` | `tauri-plugin-updater`, `tauri-plugin-dialog` |
| `tauri/<app>/src-tauri/capabilities/default.json` | `updater:default`, `dialog:default` (yalnızca yerel pencere — uzak URL'lere VERİLMEZ) |
| `scripts/ci/apply-updater-config.mjs` | Build öncesi pubkey enjeksiyonu + `createUpdaterArtifacts` |
| `scripts/ci/publish-updates.mjs` | `latest.json` + `downloads/` + `index.html` üretimi |
| `.github/workflows/generate-updater-key.yml` | Anahtar çifti üretimi (elle) |
| `.github/workflows/windows.yml` / `macos.yml` | Derleme + imzalama + FTPS yayın |
| `.github/workflows/android.yml` | APK'yı `downloads/`'a yükler (otomatik güncelleme yok) |
