//! Tarayıcı koruması — denetle ve kur.
//!
//! İKİ ŞEYİ AYNI ANDA SAĞLAR:
//!   1) Tarayıcının kendi parola kasası kapanır ("Şifreyi kaydedeyim mi?"
//!      balonu çıkmaz, kullanıcı ayarlardan geri açamaz).
//!   2) Bogahost Kasa eklentisi zorunlu kurulur; kullanıcı silse bile Chrome
//!      update.xml'i okuyup yeniden kurar.
//!
//! NEDEN UYGULAMA İÇİNDE: bunlar yönetilen politikadır ve yönetici hakkı ister.
//! Kullanıcıya "şu betiği Terminal'de çalıştır" demek, herkesin bileceği ve
//! yapacağı bir şey değil — pratikte hiç kurulmuyor. Uygulama açılışta durumu
//! okur, eksikse tek düğmeyle işletim sisteminin kendi yönetici penceresini
//! açar.
//!
//! DURUMU OKUMAK yönetici hakkı İSTEMEZ; yalnızca KURMAK ister.

use serde::Serialize;

/// Eklenti kimliği — imzalama anahtarından türer, DEĞİŞMEZ.
/// Değişirse Chrome bunu bambaşka bir eklenti sayar ve zorunlu kurulum kopar.
pub const EKLENTI_ID: &str = "nfoohkianbefpbobbbgfikiiicnghjeh";
const GUNCELLEME_URL: &str = "https://native.bogahost.com/eklenti/update.xml";
const KAYNAK: &str = "https://native.bogahost.com/*";
/// Profil kimliği — kaldırma bu kimlikle yapılıyor, profil metniyle AYNI olmalı.
const PROFIL_KIMLIK: &str = "com.bogahost.kasa.tarayici";

#[derive(Serialize, Clone, Default)]
pub struct Durum {
    /// Tarayıcının kendi parola kasası kapalı mı?
    pub kasa_kapali: bool,
    /// Eklenti zorunlu kurulum listesinde mi?
    pub eklenti_zorunlu: bool,
    /// Bu platformda kurulum desteği var mı?
    pub kurulabilir: bool,
}

impl Durum {
    pub fn tamam(&self) -> bool {
        self.kasa_kapali && self.eklenti_zorunlu
    }
}

// ── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
const PLIST: &str = "/Library/Managed Preferences/com.google.Chrome";

#[cfg(target_os = "macos")]
fn oku(anahtar: &str) -> Option<String> {
    let c = std::process::Command::new("defaults")
        .args(["read", PLIST, anahtar])
        .output()
        .ok()?;
    if !c.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&c.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
pub fn durum() -> Durum {
    Durum {
        // `defaults` mantıksal değeri 0/1 olarak yazdırır.
        kasa_kapali: oku("PasswordManagerEnabled").map(|v| v == "0").unwrap_or(false),
        // ExtensionSettings asil mekanizma; forcelist yalnizca eski
        // kurulumlar icin geriye donuk kontrol ediliyor.
        eklenti_zorunlu: oku("ExtensionSettings")
            .or_else(|| oku("ExtensionInstallForcelist"))
            .map(|v| v.contains(EKLENTI_ID))
            .unwrap_or(false),
        kurulabilir: true,
    }
}

/// Chrome politikasını taşıyan yapılandırma profili.
///
/// NEDEN PROFİL, `defaults write` DEĞİL:
/// `/Library/Managed Preferences` altına elle plist yazmak eski macOS'ta
/// çalışıyordu; Ventura'dan itibaren bu dizin yalnızca YÜKLÜ PROFİLLERDEN
/// üretiliyor ve elle yazılan dosyalar yok sayılıyor (ya da bir sonraki
/// cfprefsd turunda siliniyor). Belirtisi tam olarak şuydu: "politika yazıldı
/// ama Chrome okumadı", tarayıcı kapatılıp açılsa bile.
///
/// Profil yüklendiğinde macOS o dizini KENDİSİ oluşturur; böylece durum()
/// içindeki okuma da anlamlı hâle gelir.
#[cfg(target_os = "macos")]
fn profil_metni() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>PayloadDisplayName</key><string>Bogahost Kasa — Tarayıcı Koruması</string>
  <key>PayloadDescription</key><string>Chrome'un kendi parola kasasını kapatır ve Bogahost Kasa eklentisini kurar.</string>
  <key>PayloadOrganization</key><string>Bogahost</string>
  <key>PayloadIdentifier</key><string>com.bogahost.kasa.tarayici</string>
  <key>PayloadType</key><string>Configuration</string>
  <key>PayloadUUID</key><string>7B2F1C64-9A3D-4E51-9C77-BOGAHOSTKASA01</string>
  <key>PayloadVersion</key><integer>1</integer>
  <key>PayloadScope</key><string>System</string>
  <key>PayloadRemovalDisallowed</key><false/>
  <key>PayloadContent</key>
  <array>
    <dict>
      <key>PayloadType</key><string>com.google.Chrome</string>
      <key>PayloadIdentifier</key><string>com.bogahost.kasa.tarayici.chrome</string>
      <key>PayloadUUID</key><string>7B2F1C64-9A3D-4E51-9C77-BOGAHOSTKASA02</string>
      <key>PayloadVersion</key><integer>1</integer>
      <key>PayloadDisplayName</key><string>Google Chrome</string>
      <key>PasswordManagerEnabled</key><false/>
      <key>AutofillAddressEnabled</key><false/>
      <key>AutofillCreditCardEnabled</key><false/>
      <!-- MAĞAZA DIŞI EKLENTİ İÇİN DOĞRU MEKANİZMA BU.
           ExtensionInstallForcelist tek başına yetmiyor: chrome://policy
           sayfasında girdi "[BLOCKED]" olarak görünüyor ve durum "Hata, Uyarı"
           oluyordu. Chrome, Web Mağazası dışından zorunlu kurulumu ancak
           ExtensionSettings içinde override_update_url ile açıkça izin
           verildiğinde yapıyor. -->
      <key>ExtensionSettings</key>
      <dict>
        <key>{id}</key>
        <dict>
          <key>installation_mode</key><string>force_installed</string>
          <key>update_url</key><string>{url}</string>
          <key>override_update_url</key><true/>
        </dict>
      </dict>
      <key>ExtensionInstallSources</key>
      <array><string>{kaynak}</string></array>
    </dict>
    <dict>
      <key>PayloadType</key><string>com.microsoft.Edge</string>
      <key>PayloadIdentifier</key><string>com.bogahost.kasa.tarayici.edge</string>
      <key>PayloadUUID</key><string>7B2F1C64-9A3D-4E51-9C77-BOGAHOSTKASA03</string>
      <key>PayloadVersion</key><integer>1</integer>
      <key>PayloadDisplayName</key><string>Microsoft Edge</string>
      <key>PasswordManagerEnabled</key><false/>
    </dict>
  </array>
</dict>
</plist>
"#,
        id = EKLENTI_ID,
        url = GUNCELLEME_URL,
        kaynak = KAYNAK,
    )
}

/// Yüklü tarayıcı koruması profilini kaldırır.
///
/// NEDEN AYRI BİR EYLEM: aynı kimlikte bir profil yüklüyken yenisini açmak
/// macOS'ta ya sessizce reddediliyor ya da kullanıcıdan elle silmesini
/// istiyor. Sistem Ayarları'nda profil kaldırma yeri de kolay bulunmuyor.
/// `profiles remove` yönetici hakkı ister; işletim sisteminin kendi kimlik
/// penceresi çıkar, parola bize değil sisteme verilir.
#[cfg(target_os = "macos")]
pub fn profil_kaldir() -> Result<(), String> {
    let betik = format!(
        "do shell script \"/usr/bin/profiles remove -identifier {kimlik}\" with administrator privileges",
        kimlik = PROFIL_KIMLIK
    );
    let c = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&betik)
        .output()
        .map_err(|e| format!("osascript çalıştırılamadı: {e}"))?;

    if c.status.success() {
        return Ok(());
    }
    let hata = String::from_utf8_lossy(&c.stderr);
    if hata.contains("-128") {
        return Err("İptal edildi.".into());
    }
    // Profil zaten yoksa `profiles remove` hata döner; bu bir sorun değil.
    Err(format!("Kaldırılamadı (profil yüklü olmayabilir): {}", hata.trim()))
}

#[cfg(not(target_os = "macos"))]
pub fn profil_kaldir() -> Result<(), String> {
    Err("Bu platformda yapılandırma profili kullanılmıyor.".into())
}

#[cfg(target_os = "macos")]
pub fn kur() -> Result<(), String> {
    use std::io::Write;

    // 1) ESKİ macOS YOLU — yönetilen tercihi doğrudan yaz.
    //    Sonuç DOĞRULANIR: eskiden komut zincirinin sonunda `true` vardı ve
    //    her `defaults write` başarısız olsa bile osascript başarı dönüyordu;
    //    uygulama "yazıldı" diyip aslında hiçbir şey yazmamış oluyordu.
    let komutlar = format!(
        "mkdir -p '/Library/Managed Preferences' && \
         defaults write '{p}' PasswordManagerEnabled -bool false && \
         defaults write '{p}' AutofillAddressEnabled -bool false && \
         defaults write '{p}' AutofillCreditCardEnabled -bool false && \
         defaults write '{p}' ExtensionInstallForcelist -array '{id};{url}' && \
         defaults write '{p}' ExtensionInstallSources -array '{kaynak}' && \
         chmod 644 '{p}.plist' && \
         killall cfprefsd 2>/dev/null; \
         defaults read '{p}' PasswordManagerEnabled",
        p = PLIST,
        id = EKLENTI_ID,
        url = GUNCELLEME_URL,
        kaynak = KAYNAK,
    );
    let kacisli = komutlar.replace('\\', "\\\\").replace('"', "\\\"");
    let betik = format!("do shell script \"{kacisli}\" with administrator privileges");

    let c = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&betik)
        .output()
        .map_err(|e| format!("osascript çalıştırılamadı: {e}"))?;

    if !c.status.success() {
        let hata = String::from_utf8_lossy(&c.stderr);
        // Kullanıcı yönetici penceresini kapattıysa devam etmenin anlamı yok.
        if hata.contains("-128") {
            return Err("İptal edildi.".into());
        }
        // BAŞARISIZLIK BURADA ÖLÜMCÜL DEĞİL — ve eskiden öyle sayılıyordu.
        //
        // Modern macOS /Library/Managed Preferences altına dosya OLUŞTURMUYOR;
        // `defaults write` sessizce yazmıyor, ardından `chmod` "No such file or
        // directory" veriyor ve zincir kopuyordu. Kod bunu hata sayıp geri
        // dönüyordu, yani ASIL ÇÖZÜM olan profil yoluna hiç ulaşılmıyordu.
        // Artık bu yol yalnızca eski sürümler için "denenir"; tutmazsa aşağı
        // düşülür.
    }

    // Yazma tuttuysa iş bitti (eski macOS).
    if durum().tamam() {
        return Ok(());
    }

    // 2) MODERN macOS YOLU — yapılandırma profili.
    //    Ventura ve sonrasında /Library/Managed Preferences yalnızca yüklü
    //    profillerden üretilir; elle yazılan dosya yok sayılır. Profil
    //    kullanıcı onayı ister ve `profiles install` artık MDM dışında
    //    kullanılamıyor, bu yüzden dosya açılıp Sistem Ayarları'na düşürülür.
    // İndirilenler'e yazılıyor, geçici dizine değil: otomatik açılma bir
    // sebeple çalışmazsa kullanıcının dosyayı BULABİLMESİ gerekiyor. Geçici
    // dizin macOS'ta süreç başına ve okunaksız bir yol.
    let yol = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join("Downloads"))
        .ok()
        .filter(|d| d.is_dir())
        .unwrap_or_else(std::env::temp_dir)
        .join("Bogahost-Kasa-Tarayici-Korumasi.mobileconfig");
    let mut f = std::fs::File::create(&yol).map_err(|e| format!("Profil yazılamadı: {e}"))?;
    f.write_all(profil_metni().as_bytes())
        .map_err(|e| format!("Profil yazılamadı: {e}"))?;
    drop(f);

    // ESKİ PROFİLİ ÖNCE KALDIR.
    //
    // Aynı kimlikte bir profil yüklüyken yenisini açmak macOS'ta ya sessizce
    // reddediliyor ya da kullanıcıdan elle silmesini istiyor — Sistem
    // Ayarları'ndan profil kaldırmak da kolay bulunan bir yer değil.
    // Kullanıcının bildirdiği durum tam olarak buydu: "güncelleme geldi ama
    // eski profili kaldıramıyorum".
    //
    // Kaldırma BAŞARISIZ OLABİLİR (profil hiç yoktur, kullanıcı yönetici
    // penceresini kapatır) ve bu ölümcül değil: yine de yeni profil açılır,
    // en kötü ihtimalle kullanıcı elle siler.
    let _ = profil_kaldir();

    let _ = std::process::Command::new("open").arg(&yol).spawn();
    // Ayarların Profiller bölümünü de aç ki kullanıcı aramasın.
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.Profiles-Settings.extension")
        .spawn();

    Err("PROFIL_ONAYI".into())
}

// ── Windows ────────────────────────────────────────────────────────────────

#[cfg(windows)]
const ANAHTAR: &str = r"HKLM\SOFTWARE\Policies\Google\Chrome";

#[cfg(windows)]
fn reg_oku(yol: &str, ad: &str) -> Option<String> {
    let c = std::process::Command::new("reg")
        .args(["query", yol, "/v", ad])
        .output()
        .ok()?;
    if !c.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&c.stdout).to_string())
}

#[cfg(windows)]
pub fn durum() -> Durum {
    Durum {
        kasa_kapali: reg_oku(ANAHTAR, "PasswordManagerEnabled")
            .map(|v| v.contains("0x0"))
            .unwrap_or(false),
        // ExtensionSettings asil mekanizma (bkz. kur()); forcelist geriye donuk.
        eklenti_zorunlu: reg_oku(ANAHTAR, "ExtensionSettings")
            .or_else(|| reg_oku(&format!("{ANAHTAR}\\ExtensionInstallForcelist"), "1"))
            .map(|v| v.contains(EKLENTI_ID))
            .unwrap_or(false),
        kurulabilir: true,
    }
}

#[cfg(windows)]
pub fn kur() -> Result<(), String> {
    // PowerShell `-Verb RunAs` UAC penceresini açar; yönetici hakkı olmadan
    // HKLM\SOFTWARE\Policies yazılamaz.
    // ExtensionSettings TEK SATIRLIK JSON olarak yazılır. Mağaza dışı zorunlu
    // kurulumun çalıştığı tek yol bu: ExtensionInstallForcelist girdisi
    // chrome://policy sayfasında "[BLOCKED]" görünüyor ve kurulum yapılmıyor.
    let ayarlar = format!(
        r#"{{\"{id}\":{{\"installation_mode\":\"force_installed\",\"update_url\":\"{url}\",\"override_update_url\":true}}}}"#,
        id = EKLENTI_ID,
        url = GUNCELLEME_URL,
    );
    let komut = format!(
        "reg add \"{k}\" /v PasswordManagerEnabled /t REG_DWORD /d 0 /f; \
         reg add \"{k}\" /v AutofillAddressEnabled /t REG_DWORD /d 0 /f; \
         reg add \"{k}\" /v AutofillCreditCardEnabled /t REG_DWORD /d 0 /f; \
         reg add \"{k}\" /v ExtensionSettings /t REG_SZ /d \"{ayarlar}\" /f; \
         reg add \"{k}\\ExtensionInstallSources\" /v 1 /t REG_SZ /d \"{kaynak}\" /f",
        k = ANAHTAR,
        ayarlar = ayarlar,
        kaynak = KAYNAK,
    );

    let c = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden \
                 -ArgumentList '-NoProfile','-Command','{}'",
                komut.replace('\'', "''")
            ),
        ])
        .output()
        .map_err(|e| format!("powershell çalıştırılamadı: {e}"))?;

    if c.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Kurulamadı (yönetici izni verilmemiş olabilir): {}",
            String::from_utf8_lossy(&c.stderr).trim()
        ))
    }
}

// ── Diğer ──────────────────────────────────────────────────────────────────

#[cfg(not(any(target_os = "macos", windows)))]
pub fn durum() -> Durum {
    Durum::default()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn kur() -> Result<(), String> {
    Err("Bu platformda desteklenmiyor.".into())
}

// ── Eklentinin İKİNCİ kurulum yolu (macOS) ─────────────────────────────────
//
// NEDEN İKİNCİ BİR YOL: zorunlu kurulum politikası Chrome'un iznine bağlı ve
// mağaza dışı eklentilerde Chrome zorluk çıkarıyor ("[BLOCKED]"). macOS'ta
// Chrome'un bir de "harici eklenti" mekanizması var: belirli bir klasöre
// eklentinin kimliğiyle adlandırılmış küçük bir JSON konursa Chrome onu bir
// sonraki açılışta kuruyor.
//
// İkisi ÇAKIŞMAZ, birbirini tamamlar: politika yolu kullanıcı silince geri
// getiriyor, harici yol ilk kurulumu daha güvenilir yapıyor.

/// CRX'in makinede duracağı yer. Chrome dosyayı buradan okuyacağı için
/// kullanıcının indirilenler klasörü uygun değil — orası silinebilir.
#[cfg(target_os = "macos")]
const CRX_YOL: &str = "/Library/Application Support/Bogahost/bogahost-kasa.crx";

#[cfg(target_os = "macos")]
const HARICI_DIZIN: &str = "/Library/Application Support/Google/Chrome/External Extensions";

/// Eklentiyi Chrome'un harici eklenti klasörü üzerinden kurar.
///
/// CRX sunucudan indirilip makineye konuyor, sonra kimlik adında bir JSON
/// yazılıyor. Yönetici hakkı ister (sistem klasörüne yazılıyor).
#[cfg(target_os = "macos")]
pub fn eklenti_kur() -> Result<(), String> {
    let crx_url = "https://native.bogahost.com/eklenti/bogahost-kasa.crx";
    // Sürüm update.xml ile aynı olmalı; Chrome ikisini karşılaştırıyor.
    let surum = eklenti_surumu().unwrap_or_else(|| "1.2.1".to_string());

    let komutlar = format!(
        "mkdir -p '/Library/Application Support/Bogahost' && \
         mkdir -p '{dizin}' && \
         /usr/bin/curl -fsSL '{crx_url}' -o '{crx}' && \
         chmod 644 '{crx}' && \
         printf '%s' '{{\\\"external_crx\\\": \\\"{crx}\\\", \\\"external_version\\\": \\\"{surum}\\\"}}' > '{dizin}/{id}.json' && \
         chmod 644 '{dizin}/{id}.json' && \
         echo tamam",
        dizin = HARICI_DIZIN,
        crx = CRX_YOL,
        crx_url = crx_url,
        surum = surum,
        id = EKLENTI_ID,
    );

    let kacisli = komutlar.replace('\\', "\\\\").replace('"', "\\\"");
    let betik = format!("do shell script \"{kacisli}\" with administrator privileges");

    let c = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&betik)
        .output()
        .map_err(|e| format!("osascript çalıştırılamadı: {e}"))?;

    if c.status.success() {
        return Ok(());
    }
    let hata = String::from_utf8_lossy(&c.stderr);
    if hata.contains("-128") {
        return Err("İptal edildi.".into());
    }
    Err(format!("Eklenti kurulamadı: {}", hata.trim()))
}

/// update.xml'deki sürümü okur — harici kurulum JSON'u aynı sürümü yazmalı,
/// yoksa Chrome dosyayı yok sayıyor.
#[cfg(target_os = "macos")]
fn eklenti_surumu() -> Option<String> {
    let c = std::process::Command::new("/usr/bin/curl")
        .args(["-fsSL", "--max-time", "15", GUNCELLEME_URL])
        .output()
        .ok()?;
    if !c.status.success() {
        return None;
    }
    let metin = String::from_utf8_lossy(&c.stdout);
    let bas = metin.find("version='")? + 9;
    let kalan = &metin[bas..];
    let son = kalan.find('\'')?;
    Some(kalan[..son].to_string())
}

/// Harici kurulumu geri alır — eklentiyi tamamen temizlemek isteyen için.
#[cfg(target_os = "macos")]
pub fn eklenti_kaldir() -> Result<(), String> {
    let komut = format!(
        "rm -f '{dizin}/{id}.json' '{crx}'; echo tamam",
        dizin = HARICI_DIZIN,
        id = EKLENTI_ID,
        crx = CRX_YOL,
    );
    let kacisli = komut.replace('\\', "\\\\").replace('"', "\\\"");
    let betik = format!("do shell script \"{kacisli}\" with administrator privileges");
    let c = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&betik)
        .output()
        .map_err(|e| format!("osascript çalıştırılamadı: {e}"))?;
    if c.status.success() {
        Ok(())
    } else {
        Err(format!("Kaldırılamadı: {}", String::from_utf8_lossy(&c.stderr).trim()))
    }
}

/// Harici kurulum dosyası yerinde mi?
#[cfg(target_os = "macos")]
pub fn eklenti_dosyasi_var() -> bool {
    std::path::Path::new(&format!("{HARICI_DIZIN}/{EKLENTI_ID}.json")).exists()
}

/* WINDOWS'TA YÖNETİCİ KURULUMU YOK — AMA DÜĞME DE ÇALIŞMIYORDU.
   Ayarlar'daki "Kur" düğmesi Windows'ta her zaman "bu platformda harici
   eklenti kurulumu yok" hatası veriyordu. Çalışmayan bir düğme göstermek,
   kullanıcıyı kendi hatası sanmaya iter.
   Artık arayüz `sistem()` ile platformu öğrenip o düğmeyi hiç göstermiyor;
   buradaki gövde de hata yerine İNDİRME yoluna düşüyor — biri yine de
   çağırırsa iş görsün. */
#[cfg(not(target_os = "macos"))]
pub fn eklenti_kur() -> Result<(), String> {
    eklenti_indir().map(|_| ())
}
#[cfg(not(target_os = "macos"))]
pub fn eklenti_kaldir() -> Result<(), String> {
    Err("Bu platformda yönetici kurulumu yok; eklentiyi Chrome'dan kaldırın.".into())
}
#[cfg(not(target_os = "macos"))]
pub fn eklenti_dosyasi_var() -> bool {
    false
}

// ── Eklentiyi elle kurmak isteyene ─────────────────────────────────────────

/// Eklenti paketinin adresi. `eklenti_kur` bunun CRX'ini SİSTEM klasörüne
/// yönetici hakkıyla koyuyor; buradaki yol ise yönetici hakkı istemez.
const EKLENTI_ZIP: &str = "https://native.bogahost.com/eklenti/bogahost-kasa-eklenti.zip";

/// Paketin açıldığı klasör. Hep AYNI yer olmalı: kullanıcı Chrome'a bu
/// klasörü tanıtıyor, biz de güncellemede içindekileri değiştiriyoruz.
/// Başka bir klasöre indirseydik her güncellemede yeniden tanıtmak gerekirdi.
pub fn eklenti_klasoru() -> Option<std::path::PathBuf> {
    let ev = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
    Some(std::path::Path::new(&ev).join("Downloads").join("bogahost-kasa-eklenti"))
}

/// Klasördeki manifest.json'un sürümü — yani Chrome'un ŞU AN okuduğu sürüm.
pub fn eklenti_yerel_surum() -> Option<String> {
    let m = eklenti_klasoru()?.join("manifest.json");
    let metin = std::fs::read_to_string(m).ok()?;
    let j: serde_json::Value = serde_json::from_str(&metin).ok()?;
    j["version"].as_str().map(|v| v.to_string())
}

/// Sunucudaki sürümün ÖNBELLEKLİ hâli.
///
/// Eklenti köprüye dakikada bir soruyor; her seferinde update.xml'i çekmek
/// hem gereksiz hem de kaba olurdu. Sürüm yarım saatte bir tazeleniyor.
static YAYIN_SURUM: std::sync::Mutex<Option<(String, std::time::Instant)>> =
    std::sync::Mutex::new(None);
const YAYIN_TAZELIK: std::time::Duration = std::time::Duration::from_secs(1800);

pub fn eklenti_yayin_surum() -> Option<String> {
    // Kilit ağ çağrısı boyunca TUTULMUYOR: tutulsaydı yavaş bir istek bütün
    // köprüyü bekletirdi.
    {
        let g = YAYIN_SURUM.lock().unwrap();
        if let Some((v, t)) = g.as_ref() {
            if t.elapsed() < YAYIN_TAZELIK {
                return Some(v.clone());
            }
        }
    }
    let v = eklenti_uzak_surum()?;
    *YAYIN_SURUM.lock().unwrap() = Some((v.clone(), std::time::Instant::now()));
    Some(v)
}

/// Sunucudaki sürüm. `eklenti_surumu()` yalnız macOS'ta ve curl ile vardı;
/// bu her iki platformda da çalışıyor.
pub fn eklenti_uzak_surum() -> Option<String> {
    let metin = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?
        .get(GUNCELLEME_URL)
        .send()
        .ok()?
        .text()
        .ok()?;
    let bas = metin.find("version='")? + 9;
    let kalan = &metin[bas..];
    let son = kalan.find('\'')?;
    Some(kalan[..son].to_string())
}

fn surum_kucuk_mu(a: &str, b: &str) -> bool {
    let say = |s: &str| -> Vec<u32> { s.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (x, y) = (say(a), say(b));
    for i in 0..x.len().max(y.len()) {
        let (m, n) = (*x.get(i).unwrap_or(&0), *y.get(i).unwrap_or(&0));
        if m != n {
            return m < n;
        }
    }
    false
}

/// PAKETLENMEMİŞ EKLENTİ KENDİ KENDİNE GÜNCELLENMEZ.
///
/// Bildirilen durum: uygulama güncellendi ama Chrome'daki eklenti eski
/// göründü. Sebebi şu — "Paketlenmemiş öğe yükle" ile kurulan bir eklenti
/// `update.xml`e hiç bakmaz; Chrome açılışta klasördeki DOSYALARI okur.
/// Dosyalar değişmediği sürece sonsuza kadar eski sürüm çalışır.
///
/// Bu yüzden tazelemeyi uygulama üstleniyor: klasör varsa ve içindeki sürüm
/// sunucudakinden eskiyse dosyalar sessizce değiştiriliyor. Kullanıcıya
/// düşen tek şey Chrome'u kapatıp açmak.
///
/// Klasör YOKSA hiçbir şey yapılmıyor — eklentiyi bu yolla kurmamış birinin
/// İndirilenler'ine kendiliğinden dosya bırakmak doğru olmaz.
pub fn eklenti_tazele() -> Result<Option<String>, String> {
    let Some(yerel) = eklenti_yerel_surum() else { return Ok(None) };
    let Some(uzak) = eklenti_uzak_surum() else { return Ok(None) };
    if !surum_kucuk_mu(&yerel, &uzak) {
        return Ok(None);
    }
    eklenti_indir_sessiz()?;
    Ok(Some(uzak))
}

/// Paketin adresi — arayüz bunu kopyalanabilir bir bağlantı olarak gösteriyor.
///
/// Kullanıcının isteği: "eklentilere kasa uygulamasından link bazlı ulaşım
/// verilebilsin". Bazı makinelerde indirme klasörünü açmak işe yaramıyor
/// (uzak masaüstü, kısıtlı profil); elde bir bağlantı olması her zaman
/// çalışan yol.
pub fn eklenti_zip_adresi() -> &'static str {
    EKLENTI_ZIP
}

/// Eklentiyi kullanıcının İndirilenler klasörüne indirir ve AÇAR.
///
/// NEDEN AYRI BİR YOL VAR: zorunlu kurulum politikası ve harici kurulum
/// dosyası ikisi de yönetici hakkı istiyor ve Chrome mağaza dışı eklentilerde
/// zorluk çıkarabiliyor. Personelin elinde hiçbir şey kalmasın istemiyoruz:
/// bu düğme paketi indirip klasörü açıyor, "Paketlenmemiş öğe yükle" ile iki
/// tıkta kuruluyor. Hiçbir hak istemez, her makinede çalışır.
///
/// Klasör her seferinde yenileniyor ki eski sürüm yüklenmesin.
pub fn eklenti_indir() -> Result<String, String> {
    let klasor = eklenti_indir_sessiz()?;
    klasoru_goster(&klasor);
    Ok(klasor.display().to_string())
}

/// İndirip açar ama klasörü AÇMAZ — arka plandaki tazeleme bunu kullanıyor.
/// Kullanıcı bir şey istemediği hâlde ekranına pencere açmak rahatsızlıktır.
fn eklenti_indir_sessiz() -> Result<std::path::PathBuf, String> {
    let klasor = eklenti_klasoru().ok_or("Kullanıcı klasörü bulunamadı.")?;
    let indirilenler = klasor.parent().ok_or("İndirilenler klasörü bulunamadı.")?.to_path_buf();
    std::fs::create_dir_all(&indirilenler)
        .map_err(|e| format!("İndirilenler klasörü açılamadı: {e}"))?;

    let zip = indirilenler.join("bogahost-kasa-eklenti.zip");

    let bayt = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?
        .get(EKLENTI_ZIP)
        .send()
        .and_then(|y| y.error_for_status())
        .and_then(|y| y.bytes())
        .map_err(|e| format!("İndirilemedi: {e}"))?;
    std::fs::write(&zip, &bayt).map_err(|e| format!("Yazılamadı: {e}"))?;

    // ESKİ KLASÖRÜ YALNIZCA KENDİMİZİNKİYSE SİL.
    // İsim çakışması olabilir; içinde manifest.json yoksa dokunmuyoruz ve
    // paketi yanına bırakıyoruz — kullanıcının dosyasını silmek yok.
    if klasor.join("manifest.json").exists() {
        let _ = std::fs::remove_dir_all(&klasor);
    }
    std::fs::create_dir_all(&klasor).map_err(|e| format!("Klasör açılamadı: {e}"))?;

    ac_arsiv(&zip, &klasor)?;
    Ok(klasor)
}

#[cfg(target_os = "macos")]
fn ac_arsiv(zip: &std::path::Path, klasor: &std::path::Path) -> Result<(), String> {
    let c = std::process::Command::new("/usr/bin/unzip")
        .args(["-o", "-q"])
        .arg(zip)
        .arg("-d")
        .arg(klasor)
        .output()
        .map_err(|e| format!("unzip çalıştırılamadı: {e}"))?;
    if c.status.success() {
        Ok(())
    } else {
        Err(format!("Arşiv açılamadı: {}", String::from_utf8_lossy(&c.stderr).trim()))
    }
}

#[cfg(windows)]
fn ac_arsiv(zip: &std::path::Path, klasor: &std::path::Path) -> Result<(), String> {
    let c = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!(
            "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
            zip.display(),
            klasor.display()
        ))
        .output()
        .map_err(|e| format!("powershell çalıştırılamadı: {e}"))?;
    if c.status.success() {
        Ok(())
    } else {
        Err(format!("Arşiv açılamadı: {}", String::from_utf8_lossy(&c.stderr).trim()))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn ac_arsiv(_zip: &std::path::Path, _klasor: &std::path::Path) -> Result<(), String> {
    Err("Bu platformda arşiv açma yok.".into())
}

fn klasoru_goster(klasor: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open").arg(klasor).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(klasor).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = klasor;
}
