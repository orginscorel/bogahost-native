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

#[cfg(target_os = "macos")]
pub fn kur() -> Result<(), String> {
    // Tek bir `do shell script ... with administrator privileges` çağrısı:
    // macOS'un kendi kimlik penceresi bir kez çıkar, kullanıcı parolasını
    // BİZE değil işletim sistemine verir.
    //
    // cfprefsd yeniden başlatılmazsa macOS yönetilen tercihleri önbellekten
    // okumaya devam eder ve politika saatlerce devreye girmez.
    let komutlar = format!(
        "mkdir -p '/Library/Managed Preferences'; \
         defaults write '{p}' PasswordManagerEnabled -bool false; \
         defaults write '{p}' AutofillAddressEnabled -bool false; \
         defaults write '{p}' AutofillCreditCardEnabled -bool false; \
         defaults write '{p}' ExtensionInstallForcelist -array '{id};{url}'; \
         defaults write '{p}' ExtensionInstallSources -array '{kaynak}'; \
         defaults write '/Library/Managed Preferences/com.microsoft.Edge' PasswordManagerEnabled -bool false; \
         chmod 644 '{p}.plist' 2>/dev/null; \
         killall cfprefsd 2>/dev/null; true",
        p = PLIST,
        id = EKLENTI_ID,
        url = GUNCELLEME_URL,
        kaynak = KAYNAK,
    );

    // AppleScript metni içinde tırnak kaçışı: komutları çift tırnaklı bir
    // AppleScript dizesine gömüyoruz, içindeki " ve \ kaçırılmalı.
    let kacisli = komutlar.replace('\\', "\\\\").replace('"', "\\\"");
    let betik = format!(
        "do shell script \"{kacisli}\" with administrator privileges"
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
    // -128 = kullanıcı yönetici penceresini iptal etti; hata değil.
    if hata.contains("-128") {
        return Err("İptal edildi.".into());
    }
    Err(format!("Kurulamadı: {}", hata.trim()))
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
