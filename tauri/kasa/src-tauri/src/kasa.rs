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
    /// Sunucu gönderiyordu ama yapıda yoktu, arayüze hiç ulaşmıyordu.
    #[serde(default)]
    pub ip: Option<String>,
    /// Kayıt bu kullanıcıya mı ait? Düzenleme/silme buna bağlı.
    #[serde(default)]
    pub benim: bool,
    /// Son doldurma/açma zamanı (ISO 8601). Liste bağlamı için.
    #[serde(default)]
    pub son_kullanim: Option<String>,
    /// Kaydın oluşturulma zamanı — "parola kaç aylık" sorusunun cevabı.
    #[serde(default)]
    pub olusturma: Option<String>,
    #[serde(default)]
    pub guncelleme: Option<String>,
    /// Parola gücü 0–3. SUNUCUDA hesaplanıyor; parolanın kendisi hiç gelmiyor,
    /// bu yüzden gizli kayıtlarda bile güvenle gösterilebiliyor.
    #[serde(default)]
    pub guc: Option<u8>,
    /// Gizli kayıt: paylaşılan kişi DOLDURABİLİR ama göremez/kopyalayamaz.
    #[serde(default)]
    pub gizli: bool,
    /// Bu kullanıcı için görüntüleme/kopyalama açık mı (sahibi her zaman açık).
    #[serde(default = "varsayilan_dogru")]
    pub gosterilebilir: bool,
}

fn varsayilan_dogru() -> bool {
    true
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

/// Yedek jeton dosyası.
///
/// NEDEN GEREKLİ: macOS Anahtar Zinciri kaydı uygulamanın KOD İMZASINA bağlı.
/// İmzasız/ad-hoc derlemede her yeni sürüm farklı bir uygulama sayılır ve
/// önceki kaydı OKUYAMAZ — "beni hatırla" her güncellemede bozuluyordu.
/// (Erişilebilirlik izniyle aynı kök neden.)
///
/// DÜRÜST TAKAS: dosya, Anahtar Zinciri kadar güvenli değildir; kullanıcı
/// hesabıyla çalışan başka bir program okuyabilir. Buna karşılık oturum
/// güncellemeler boyunca korunur. Uygulama Developer ID ile imzalandığında
/// Anahtar Zinciri yolu zaten tutar ve dosyaya hiç düşülmez.
fn yedek_yol() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    let taban = std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join("Library/Application Support"));
    #[cfg(windows)]
    let taban = std::env::var("APPDATA").ok().map(std::path::PathBuf::from);
    #[cfg(not(any(target_os = "macos", windows)))]
    let taban: Option<std::path::PathBuf> = std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".config"));

    let d = taban?.join("Bogahost Kasa");
    std::fs::create_dir_all(&d).ok()?;
    Some(d.join("oturum"))
}

fn yedek_yaz(jeton: &str) {
    let Some(p) = yedek_yol() else { return };
    if std::fs::write(&p, jeton).is_err() {
        return;
    }
    // Yalnız sahibi okuyabilsin.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
}

fn yedek_oku() -> Option<String> {
    let p = yedek_yol()?;
    let s = std::fs::read_to_string(p).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn yedek_sil() {
    if let Some(p) = yedek_yol() {
        let _ = std::fs::remove_file(p);
    }
}

pub fn jeton_kaydet(jeton: &str) -> Result<(), String> {
    // Önce işletim sisteminin kasası — imzalı kurulumda en doğru yer.
    let kasa_sonucu = keyring::Entry::new(SERVIS, HESAP).and_then(|g| g.set_password(jeton));
    // Yedek her hâlükârda yazılır: kasa yazsa bile bir sonraki SÜRÜM onu
    // okuyamayabilir (imza değişir), oturum yine de sürsün.
    yedek_yaz(jeton);

    kasa_sonucu.map_err(|e| format!("Oturum kaydedilemedi: {e}"))?;
    Ok(())
}

pub fn jeton_oku() -> Option<String> {
    keyring::Entry::new(SERVIS, HESAP)
        .ok()
        .and_then(|g| g.get_password().ok())
        .or_else(yedek_oku)
}

pub fn jeton_sil() {
    if let Ok(g) = keyring::Entry::new(SERVIS, HESAP) {
        let _ = g.delete_credential();
    }
    yedek_sil();
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

/// DOLDURMAK İÇİN getirir — görüntülemeden AYRI uç.
///
/// NEDEN AYRI: gizli kayıtlarda `/reveal` kapalı (sunucu 403 döner), `/fill`
/// açık. Böylece "göster/kopyala" ile "hedefe yaz" birbirinden ayrılıyor:
/// personele bilginin kendisi verilmeden o bilgiyle iş yaptırılabiliyor.
/// Her çağrı sunucuda ayrı bir eylem olarak denetime yazılır.
pub fn doldurmak_icin_ac(durum: &Durum, id: i64) -> Result<(String, String), String> {
    let t = jeton_of(durum)?;
    let y = istemci()
        .post(format!("{SUNUCU}/vault/api/fill/{id}"))
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

/// GÖSTERME/KOPYALAMA amaçlı açar. Çağıran kullanır ve bırakır; saklanmaz.
///
/// `amac=goster` sunucuya niyeti bildirir. Adresi ya da IP'si olan kayıtlar
/// DOLDURULABİLİR olduğu için sunucu kopyalamayı reddeder: parolayı hedefe
/// doğrudan yazan bir yol dururken panoya almak, yapıştırıldığı her yerde düz
/// metin göstermek demektir. Karar sunucuda verilir — istemcinin düğmeyi
/// gizlemesine güvenilmez.
pub fn ac(durum: &Durum, id: i64) -> Result<(String, String), String> {
    let t = jeton_of(durum)?;
    let y = istemci()
        .post(format!("{SUNUCU}/vault/api/reveal/{id}"))
        .query(&[("amac", "goster")])
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

// ── Kayıt yönetimi ─────────────────────────────────────────────────────────
//
// Sahiplik GÖVDEDEN OKUNMAZ: sunucu kaydı her zaman jetonun sahibine yazar,
// yani başkası adına kayıt açılamaz. Yöneticinin eklediği kayıtta düzenleme ve
// silme sunucuda 403 ile reddedilir — istemci gizlemesine güvenilmez.

/// Sunucudan gelen hata metnini olduğu gibi taşır. "Bir şeyler ters gitti"
/// demek yerine sebebi göstermek kullanıcıyı çözüme götürür.
fn yaniti_coz(y: reqwest::blocking::Response) -> Result<serde_json::Value, String> {
    let kod = y.status();
    let j: serde_json::Value = y.json().map_err(|e| format!("Yanit okunamadi: {e}"))?;
    if j["ok"].as_bool() != Some(true) {
        return Err(j["error"]
            .as_str()
            .unwrap_or(&format!("HTTP {}", kod.as_u16()))
            .to_string());
    }
    Ok(j)
}

pub fn kayit_ekle(durum: &Durum, veri: serde_json::Value) -> Result<i64, String> {
    let t = jeton_of(durum)?;
    let y = istemci()
        .post(format!("{SUNUCU}/vault/api/items"))
        .bearer_auth(t)
        .json(&veri)
        .send()
        .map_err(|e| format!("Sunucuya ulasilamadi: {e}"))?;
    let j = yaniti_coz(y)?;
    Ok(j["id"].as_i64().unwrap_or(0))
}

pub fn kayit_guncelle(durum: &Durum, id: i64, veri: serde_json::Value) -> Result<(), String> {
    let t = jeton_of(durum)?;
    let y = istemci()
        .put(format!("{SUNUCU}/vault/api/items/{id}"))
        .bearer_auth(t)
        .json(&veri)
        .send()
        .map_err(|e| format!("Sunucuya ulasilamadi: {e}"))?;
    yaniti_coz(y)?;
    Ok(())
}

pub fn kayit_sil(durum: &Durum, id: i64) -> Result<(), String> {
    let t = jeton_of(durum)?;
    let y = istemci()
        .delete(format!("{SUNUCU}/vault/api/items/{id}"))
        .bearer_auth(t)
        .send()
        .map_err(|e| format!("Sunucuya ulasilamadi: {e}"))?;
    yaniti_coz(y)?;
    Ok(())
}

// ── Parola üreteci ─────────────────────────────────────────────────────────
//
// NEDEN İSTEMCİDE: üretilen parola sunucuya "üretim isteği" olarak gitmiyor,
// yalnızca kaydedilirken gidiyor. Ağda bir tur daha dolaşmasının anlamı yok.
//
// Rastgelelik işletim sisteminden alınıyor (getrandom). Kendi karıştırıcımızı
// yazmak, parola üretiminde yapılabilecek en kötü şeydir.

const BUYUK: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ"; // I ve O yok
const KUCUK: &[u8] = b"abcdefghijkmnopqrstuvwxyz"; // l yok
const RAKAM: &[u8] = b"23456789"; // 0 ve 1 yok
const SIMGE: &[u8] = b"!@#$%^&*()-_=+[]{}<>?";

/// İşletim sisteminden rastgele bayt.
fn rastgele(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    getrandom::getrandom(&mut b).expect("isletim sistemi rastgeleligi alinamadi");
    b
}

/// Yanlılıksız aralık seçimi.
///
/// `bayt % uzunluk` yaygın ama YANLI: 256 uzunluğa tam bölünmediğinde baştaki
/// karakterler daha sık çıkar. Aralık dışını atıp yeniden çekiyoruz.
fn sec(havuz: &[u8]) -> u8 {
    let n = havuz.len();
    let sinir = 256 - (256 % n);
    loop {
        let b = rastgele(1)[0] as usize;
        if b < sinir {
            return havuz[b % n];
        }
    }
}

pub fn parola_uret(uzunluk: usize, rakam: bool, simge: bool) -> String {
    let uzunluk = uzunluk.clamp(8, 128);
    let mut havuz: Vec<u8> = Vec::new();
    havuz.extend_from_slice(BUYUK);
    havuz.extend_from_slice(KUCUK);
    if rakam {
        havuz.extend_from_slice(RAKAM);
    }
    if simge {
        havuz.extend_from_slice(SIMGE);
    }

    // Her seçilen sınıftan EN AZ BİR karakter garanti edilir; yoksa "rakam
    // içersin" dendiği hâlde rakamsız parola çıkabiliyor.
    let mut c: Vec<u8> = vec![sec(BUYUK), sec(KUCUK)];
    if rakam {
        c.push(sec(RAKAM));
    }
    if simge {
        c.push(sec(SIMGE));
    }
    while c.len() < uzunluk {
        c.push(sec(&havuz));
    }

    // Fisher-Yates: garanti karakterler hep başta durmasın.
    for i in (1..c.len()).rev() {
        let j = (sec(b"0123456789abcdefghijklmnopqrstuvwxyz") as usize) % (i + 1);
        c.swap(i, j);
    }

    String::from_utf8_lossy(&c).to_string()
}

/// Parola gücü: 0 zayıf, 1 orta, 2 iyi, 3 güçlü.
///
/// Kaba ama dürüst bir ölçü: uzunluk ve karakter çeşitliliği. "Entropi" diye
/// kesin bir sayı vermiyoruz — parola bir sözlük kelimesiyse hesaplanan entropi
/// yalan söyler.
pub fn parola_gucu(p: &str) -> u8 {
    let u = p.chars().count();
    let cesit = [
        p.chars().any(|c| c.is_ascii_lowercase()),
        p.chars().any(|c| c.is_ascii_uppercase()),
        p.chars().any(|c| c.is_ascii_digit()),
        p.chars().any(|c| !c.is_ascii_alphanumeric()),
    ]
    .iter()
    .filter(|x| **x)
    .count();

    if u < 8 || cesit < 2 {
        0
    } else if u < 12 || cesit < 3 {
        1
    } else if u < 16 {
        2
    } else {
        3
    }
}

// ── Faz 1: çöp kutusu, kayıt geçmişi, cihazlar ─────────────────────────────
//
// Üçü de aynı deseni izliyor: sunucu karar veriyor, uygulama yalnız gösteriyor.
// Yetki kontrolü burada TEKRARLANMIYOR — istemcide yapılan denetim, denetim
// değil süstür; sunucu zaten reddediyor.

/// Ortak GET — JSON döner, `ok:false` gelirse hata mesajına çevirir.
fn getir(durum: &Durum, yol: &str) -> Result<serde_json::Value, String> {
    let t = jeton_of(durum)?;
    let y = istemci()
        .get(format!("{SUNUCU}{yol}"))
        .bearer_auth(t)
        .send()
        .map_err(|e| format!("Sunucuya ulasilamadi: {e}"))?;
    yaniti_coz(y)
}

/// Ortak POST/DELETE — gövdesiz eylemler için.
fn eylem(durum: &Durum, metot: &str, yol: &str) -> Result<serde_json::Value, String> {
    let t = jeton_of(durum)?;
    let c = istemci();
    let istek = match metot {
        "DELETE" => c.delete(format!("{SUNUCU}{yol}")),
        _ => c.post(format!("{SUNUCU}{yol}")),
    };
    let y = istek
        .bearer_auth(t)
        .send()
        .map_err(|e| format!("Sunucuya ulasilamadi: {e}"))?;
    yaniti_coz(y)
}

/// Çöp kutusundaki kayıtlar. Silinen kayıt yok olmuyor; burada bekliyor.
pub fn cop(durum: &Durum) -> Result<serde_json::Value, String> {
    getir(durum, "/vault/api/cop")
}

pub fn cop_geri(durum: &Durum, id: i64) -> Result<(), String> {
    eylem(durum, "POST", &format!("/vault/api/cop/{id}/geri")).map(|_| ())
}

pub fn cop_kalici_sil(durum: &Durum, id: i64) -> Result<(), String> {
    eylem(durum, "DELETE", &format!("/vault/api/cop/{id}")).map(|_| ())
}

/// Kayıt geçmişi — PAROLA İÇERMEZ, yalnız ne zaman ne değişti.
pub fn gecmis(durum: &Durum, id: i64) -> Result<serde_json::Value, String> {
    getir(durum, &format!("/vault/api/items/{id}/gecmis"))
}

pub fn surume_don(durum: &Durum, id: i64, revizyon: i64) -> Result<(), String> {
    eylem(durum, "POST", &format!("/vault/api/items/{id}/surum/{revizyon}")).map(|_| ())
}

/// Bu hesabın açık cihazları. Kaybolan bir dizüstü buradan kesiliyor.
pub fn cihazlar(durum: &Durum) -> Result<serde_json::Value, String> {
    getir(durum, "/vault/api/cihazlar")
}

pub fn cihaz_iptal(durum: &Durum, id: i64) -> Result<(), String> {
    eylem(durum, "DELETE", &format!("/vault/api/cihazlar/{id}")).map(|_| ())
}

/// Bu cihaz HARİÇ hepsini kapat.
pub fn diger_cihazlari_kapat(durum: &Durum) -> Result<i64, String> {
    let j = eylem(durum, "POST", "/vault/api/cihazlar/digerlerini-kapat")?;
    Ok(j["kapatilan"].as_i64().unwrap_or(0))
}
