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
        eklenti_zorunlu: oku("ExtensionInstallForcelist")
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
      <key>ExtensionInstallForcelist</key>
      <array><string>{id};{url}</string></array>
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
        if hata.contains("-128") {
            return Err("İptal edildi.".into());
        }
        return Err(format!("Kurulamadı: {}", hata.trim()));
    }

    // Yazma tuttuysa iş bitti.
    if durum().tamam() {
        return Ok(());
    }

    // 2) MODERN macOS YOLU — yapılandırma profili.
    //    Ventura ve sonrasında /Library/Managed Preferences yalnızca yüklü
    //    profillerden üretilir; elle yazılan dosya yok sayılır. Profil
    //    kullanıcı onayı ister ve `profiles install` artık MDM dışında
    //    kullanılamıyor, bu yüzden dosya açılıp Sistem Ayarları'na düşürülür.
    let yol = std::env::temp_dir().join("Bogahost-Kasa-Tarayici-Korumasi.mobileconfig");
    let mut f = std::fs::File::create(&yol).map_err(|e| format!("Profil yazılamadı: {e}"))?;
    f.write_all(profil_metni().as_bytes())
        .map_err(|e| format!("Profil yazılamadı: {e}"))?;
    drop(f);

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
        eklenti_zorunlu: reg_oku(&format!("{ANAHTAR}\\ExtensionInstallForcelist"), "1")
            .map(|v| v.contains(EKLENTI_ID))
            .unwrap_or(false),
        kurulabilir: true,
    }
}

#[cfg(windows)]
pub fn kur() -> Result<(), String> {
    // PowerShell `-Verb RunAs` UAC penceresini açar; yönetici hakkı olmadan
    // HKLM\SOFTWARE\Policies yazılamaz.
    let komut = format!(
        "reg add \"{k}\" /v PasswordManagerEnabled /t REG_DWORD /d 0 /f; \
         reg add \"{k}\" /v AutofillAddressEnabled /t REG_DWORD /d 0 /f; \
         reg add \"{k}\" /v AutofillCreditCardEnabled /t REG_DWORD /d 0 /f; \
         reg add \"{k}\\ExtensionInstallForcelist\" /v 1 /t REG_SZ /d \"{id};{url}\" /f; \
         reg add \"{k}\\ExtensionInstallSources\" /v 1 /t REG_SZ /d \"{kaynak}\" /f",
        k = ANAHTAR,
        id = EKLENTI_ID,
        url = GUNCELLEME_URL,
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
