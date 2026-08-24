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
const EKLENTI_ID: &str = "nfoohkianbefpbobbbgfikiiicnghjeh";
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

#[cfg(not(target_os = "macos"))]
pub fn eklenti_kur() -> Result<(), String> {
    Err("Bu platformda harici eklenti kurulumu yok; politika kullanılıyor.".into())
}
#[cfg(not(target_os = "macos"))]
pub fn eklenti_kaldir() -> Result<(), String> {
    Err("Bu platformda harici eklenti kurulumu yok.".into())
}
#[cfg(not(target_os = "macos"))]
pub fn eklenti_dosyasi_var() -> bool {
    false
}
