# Güncelleme (Update) — Masaüstü uygulamaları

Bu belge, Windows/macOS (Tauri) uygulamalarının **yeni sürümü nasıl fark ettiğini**,
yeni sürümün **nasıl yayınlandığını** ve **hangi adımların hâlâ elle yapıldığını** anlatır.

---

## 1. Neden "tam otomatik updater" değil?

Tauri v2'nin resmî `tauri-plugin-updater` eklentisi şunları **zorunlu** kılar:

1. **Genel (anonim) erişilebilir indirme adresi.** Updater, installer/`.tar.gz` dosyasını
   kimlik doğrulamasız indirir. Bu depo (`orginscorel/bogahost-native`) **PRIVATE**;
   GitHub Release asset'leri anonim indirilemez (401/404 döner). Updater sessizce
   başarısız olur.
2. **İmzalama anahtar çifti.** `tauri.conf.json > plugins.updater.pubkey` alanı
   zorunludur ve her artifact `TAURI_SIGNING_PRIVATE_KEY` ile imzalanmalıdır.
   Anahtar yoksa updater çalışmaz. (Bu depoya private key **konmaz**.)

Yani bugünkü kurulumda `tauri-plugin-updater` **çalışmayan** bir özellik olurdu.
Bunun yerine, private depo ile **bugün çalışan** ve imzalama anahtarı gerektirmeyen
çözüm seçildi:

> **Seçilen çözüm (b): sürüm denetimi + bildirim + indirme sayfası.**

---

## 2. Nasıl çalışır?

Kod: her uygulamanın `src-tauri/src/lib.rs` dosyası (`check_update`, `is_newer`).

1. Uygulama açılışında **sessiz** bir denetim yapılır (arka planda, Rust tarafında —
   WebView'in CSP'sinden bağımsızdır).
2. Tepsi (system tray) menüsündeki **"Güncellemeleri denetle"** ile elle de tetiklenebilir.
   Elle tetiklenen denetim sonucu **her hâlükârda** bildirim gösterir
   (güncel ise "En güncel sürümü kullanıyorsunuz").
3. Denetim şu adresi indirir:

   ```
   https://bogahost.com/native/latest.json
   ```

4. JSON'daki `version`, uygulamanın kendi sürümüyle (`CARGO_PKG_VERSION`) karşılaştırılır
   (noktalı sayısal karşılaştırma; `v` öneki tolere edilir).
5. Yeni sürüm varsa **native bildirim** gösterilir ve tepsi menüsündeki
   **"İndirme sayfasını aç"** öğesi, manifestteki `url` adresini sistem tarayıcısında açar.

**İndirme/kurulum otomatik DEĞİLDİR** — kullanıcı installer'ı indirip çalıştırır.
Bu bilinçli bir tercihtir; abartılı bir "otomatik güncelleme" vaadi verilmiyor.

### `latest.json` biçimi

```json
{
  "version": "1.2.0",
  "notes": "Uygulamalar arası geçiş menüsü eklendi.",
  "url": "https://github.com/orginscorel/bogahost-native/releases/latest"
}
```

| Alan      | Zorunlu | Açıklama |
| --------- | ------- | -------- |
| `version` | evet    | En son yayınlanan sürüm (ör. `1.2.0`) |
| `notes`   | hayır   | Bildirimde gösterilecek kısa not |
| `url`     | hayır   | "İndirme sayfasını aç" hedefi. Verilmezse depo Releases sayfası açılır. |

Ağ hatası, 404 veya bozuk JSON durumunda uygulama **sessizce** devam eder
(açılışta hiçbir uyarı çıkmaz); yalnızca elle denetimde hata bildirimi gösterilir.

---

## 3. Yeni sürüm nasıl yayınlanır?

1. **Sürüm numaralarını yükselt** (hepsi aynı olmalı):
   - `package.json` (kök)
   - `apps.config.json` → `version`
   - `tauri/<key>/package.json`
   - `tauri/<key>/src-tauri/tauri.conf.json` → `version`
   - `tauri/<key>/src-tauri/Cargo.toml` → `[package] version`  ← bildirimdeki "yüklü sürüm" buradan gelir
   - `capacitor/<key>/package.json`
2. `CHANGELOG.md`'ye kısa bir madde ekle.
3. Commit + `git tag v1.2.0` + `git push --tags`.
4. `.github/workflows/release.yml` 4 platformu derler ve **taslak (draft)** bir
   GitHub Release oluşturur. Release'i gözden geçirip yayınla.
5. **MANUEL:** `latest.json` dosyasını canlı siteye koy/güncelle:
   `https://bogahost.com/native/latest.json`
   (Bu depo canlı dosyalara dokunmaz — yükleme insan tarafından yapılır.)

> Adım 5 yapılmazsa güncelleme bildirimi **hiç çıkmaz**; uygulama yine sorunsuz çalışır.

---

## 4. İleride tam otomatik updater istenirse (yol haritası)

Aşağıdakiler tamamlanırsa `tauri-plugin-updater`'a geçilebilir:

1. **Asset'ler anonim indirilebilir olmalı** — ya depo/Release'ler **public** yapılır,
   ya da installer'lar `bogahost.com` üzerinde bir dizine yüklenir.
2. **İmzalama anahtar çifti üretilir** (bu depoda **üretilmedi**, private key repoya konmaz):
   ```bash
   npx @tauri-apps/cli signer generate -w ~/.tauri/bogahost.key
   ```
   - Public key → `tauri.conf.json > plugins.updater.pubkey`
   - Private key → GitHub **secret** `TAURI_SIGNING_PRIVATE_KEY`
   - Parola → GitHub **secret** `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   - `release.yml` içindeki build adımlarına bu iki secret `env` olarak eklenir.
3. `Cargo.toml`'a `tauri-plugin-updater = "2"`, `lib.rs`'e `.plugin(tauri_plugin_updater::Builder::new().build())`,
   `capabilities/default.json`'a `updater:default` eklenir ve `tauri.conf.json`'a:
   ```json
   "plugins": {
     "updater": {
       "endpoints": ["https://bogahost.com/native/{{target}}-{{arch}}.json"],
       "pubkey": "<PUBLIC KEY>"
     }
   }
   ```
4. Her release'de Tauri'nin ürettiği `latest.json` (imza içeren) endpoint'e yüklenir.

Bu adımlar **yapılmadı** — bilinçli olarak, çünkü private depo + anahtarsız durumda
updater çalışmaz ve "çalışıyor" görünen ama sessizce başarısız olan bir özellik
bırakmak istemedik.

---

## 5. Mobil (Android/iOS)

Mobil kabuklar canlı URL'yi yükler; **içerik güncellemesi mağaza güncellemesi gerektirmez**.
Yalnızca kabuk/izin/ikon değişince yeni APK/AAB/IPA gerekir. Bu yüzden mobilde
sürüm denetimi eklenmemiştir — mağaza (Play/App Store) kendi güncelleme mekanizmasını kullanır.
