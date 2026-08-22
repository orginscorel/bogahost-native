//! Kasa sunucusu ile iletişim ve oturum saklama.
//!
//! TASARIM: Jeton ve parolalar WebView'a ASLA verilmez. Arayüz yalnızca etiket
//! ve kullanıcı adı görür; parola, "doldur" komutu sırasında Rust tarafında
//! alınır, doğrudan klavyeye yazılır ve bellekten düşer. Böylece arayüzde bir
//! açık (XSS vb.) olsa bile kasadan parola sızdırılamaz.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Sunucu adresi — diğer uygulamalarla aynı alan adı ailesi.
pub const SUNUCU: &str = "https://dcim.bogahost.com";

/// Jetonun işletim sistemi kasasındaki adı.
const SERVIS: &str = "Bogahost Kasa";
const HESAP: &str = "cihaz-jetonu";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Kayit {
    pub id: i64,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Default)]
pub struct Durum {
    /// Bellekteki jeton. Diskte İŞLETİM SİSTEMİ KASASINDA saklanır.
    pub jeton: Mutex<Option<String>>,
    /// İki adımlı girişin birinci adımından dönen geçici bilet.
    pub bilet: Mutex<Option<String>>,
}

fn istemci() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("BogahostKasa/1.10 (Tauri)")
        .build()
        .expect("HTTP istemcisi kurulamadi")
}

// ── İşletim sistemi kasası ─────────────────────────────────────────────────

pub fn jeton_kaydet(jeton: &str) -> Result<(), String> {
    keyring::Entry::new(SERVIS, HESAP)
        .and_then(|g| g.set_password(jeton))
        .map_err(|e| format!("Oturum kaydedilemedi: {e}"))
}

pub fn jeton_oku() -> Option<String> {
    keyring::Entry::new(SERVIS, HESAP).ok()?.get_password().ok()
}

pub fn jeton_sil() {
    if let Ok(g) = keyring::Entry::new(SERVIS, HESAP) {
        let _ = g.delete_credential();
    }
}

// ── Giriş: iki adım ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct GirisSonuc {
    pub tamam: bool,
    pub iki_adim: bool,
    pub hata: Option<String>,
    pub kurulum_gerekli: bool,
    pub kullanici: Option<String>,
}

pub fn giris(durum: &Durum, kullanici: &str, sifre: &str, cihaz: &str) -> GirisSonuc {
    let y = istemci()
        .post(format!("{SUNUCU}/vault/api/giris"))
        .json(&serde_json::json!({ "kullanici": kullanici, "sifre": sifre, "cihaz": cihaz }))
        .send();

    let j: serde_json::Value = match y.and_then(|r| r.json()) {
        Ok(v) => v,
        Err(e) => {
            return GirisSonuc {
                tamam: false, iki_adim: false, kurulum_gerekli: false, kullanici: None,
                hata: Some(format!("Sunucuya ulasilamadi: {e}")),
            }
        }
    };

    if j["ok"].as_bool() != Some(true) {
        return GirisSonuc {
            tamam: false,
            iki_adim: false,
            kurulum_gerekli: j["kurulum_gerekli"].as_bool().unwrap_or(false),
            kullanici: None,
            hata: Some(j["error"].as_str().unwrap_or("Giris yapilamadi.").to_string()),
        };
    }

    if let Some(b) = j["bilet"].as_str() {
        *durum.bilet.lock().unwrap() = Some(b.to_string());
    }

    GirisSonuc {
        tamam: true,
        iki_adim: true,
        hata: None,
        kurulum_gerekli: false,
        kullanici: j["kullanici"].as_str().map(|s| s.to_string()),
    }
}

pub fn dogrula(durum: &Durum, kod: &str) -> GirisSonuc {
    let bilet = match durum.bilet.lock().unwrap().clone() {
        Some(b) => b,
        None => {
            return GirisSonuc {
                tamam: false, iki_adim: false, kurulum_gerekli: false, kullanici: None,
                hata: Some("Once giris yapin.".into()),
            }
        }
    };

    let y = istemci()
        .post(format!("{SUNUCU}/vault/api/dogrula"))
        .json(&serde_json::json!({ "bilet": bilet, "kod": kod }))
        .send();

    let j: serde_json::Value = match y.and_then(|r| r.json()) {
        Ok(v) => v,
        Err(e) => {
            return GirisSonuc {
                tamam: false, iki_adim: false, kurulum_gerekli: false, kullanici: None,
                hata: Some(format!("Sunucuya ulasilamadi: {e}")),
            }
        }
    };

    if j["ok"].as_bool() != Some(true) {
        // Bilet tek kullanimlik: sunucu dusurduyse yerelde de temizlenir
        if j["error"].as_str().map(|s| s.contains("zaman")).unwrap_or(false) {
            *durum.bilet.lock().unwrap() = None;
        }
        return GirisSonuc {
            tamam: false, iki_adim: false, kurulum_gerekli: false, kullanici: None,
            hata: Some(j["error"].as_str().unwrap_or("Kod dogrulanamadi.").to_string()),
        };
    }

    if let Some(t) = j["jeton"].as_str() {
        *durum.jeton.lock().unwrap() = Some(t.to_string());
        *durum.bilet.lock().unwrap() = None;
        let _ = jeton_kaydet(t);
    }

    GirisSonuc {
        tamam: true, iki_adim: false, hata: None, kurulum_gerekli: false,
        kullanici: j["kullanici"].as_str().map(|s| s.to_string()),
    }
}

// ── Kasa ───────────────────────────────────────────────────────────────────

fn jeton_of(durum: &Durum) -> Result<String, String> {
    durum.jeton.lock().unwrap().clone().ok_or_else(|| "Oturum yok.".to_string())
}

pub fn oturum_gecerli(durum: &Durum) -> bool {
    let Ok(t) = jeton_of(durum) else { return false };
    istemci()
        .get(format!("{SUNUCU}/vault/api/me"))
        .bearer_auth(t)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

pub fn liste(durum: &Durum) -> Result<Vec<Kayit>, String> {
    let t = jeton_of(durum)?;
    let j: serde_json::Value = istemci()
        .get(format!("{SUNUCU}/vault/api/items"))
        .bearer_auth(t)
        .send()
        .and_then(|r| r.json())
        .map_err(|e| format!("Liste alinamadi: {e}"))?;

    let mut cikti = Vec::new();
    if let Some(dizi) = j["items"].as_array() {
        for k in dizi {
            if let Ok(kayit) = serde_json::from_value::<Kayit>(k.clone()) {
                cikti.push(kayit);
            }
        }
    }
    Ok(cikti)
}

/// Adrese göre eşleşen kayıtlar — en iyi eşleşme başta.
///
/// Eşleştirme SUNUCUDA yapılır: `/vault/api/match` ucu, tarayıcı eklentisinin
/// yıllardır kullandığı sınır-güvenli host karşılaştırmasını uygular
/// ("ornek.com" kaydı "kotuornek.com" ile eşleşmez). Aynı mantığı burada
/// yeniden yazmak, iki yerde ayrışan iki kural demek olurdu.
pub fn eslesenler(durum: &Durum, url: &str) -> Result<Vec<Kayit>, String> {
    let t = jeton_of(durum)?;
    let j: serde_json::Value = istemci()
        .get(format!("{SUNUCU}/vault/api/match"))
        .query(&[("url", url)])
        .bearer_auth(t)
        .send()
        .and_then(|r| r.json())
        .map_err(|e| format!("Eslesme alinamadi: {e}"))?;

    let mut cikti = Vec::new();
    if let Some(dizi) = j["items"].as_array() {
        for k in dizi {
            if let Ok(kayit) = serde_json::from_value::<Kayit>(k.clone()) {
                cikti.push(kayit);
            }
        }
    }
    Ok(cikti)
}

/// Parolayı TEK SEFERLİK getirir. Çağıran kullanır ve bırakır; hiçbir yerde saklanmaz.
pub fn ac(durum: &Durum, id: i64) -> Result<(String, String), String> {
    let t = jeton_of(durum)?;
    let y = istemci()
        .post(format!("{SUNUCU}/vault/api/reveal/{id}"))
        .bearer_auth(t)
        .send()
        .map_err(|e| format!("Sunucuya ulasilamadi: {e}"))?;

    let durum_kodu = y.status();
    let j: serde_json::Value = y.json().map_err(|e| format!("Yanit okunamadi: {e}"))?;

    if j["ok"].as_bool() != Some(true) {
        return Err(j["error"]
            .as_str()
            .unwrap_or(&format!("HTTP {}", durum_kodu.as_u16()))
            .to_string());
    }

    Ok((
        j["username"].as_str().unwrap_or("").to_string(),
        j["password"].as_str().unwrap_or("").to_string(),
    ))
}
