//! KAYIT ZARFLARI — saf mantık, ağ yok, Tauri yok.
//!
//! NEDEN AYRI MODÜL: bu sunucuda Tauri derlenemiyor (gtk3/webkit2gtk sistem
//! bağımlılıkları yok). Ağa ve Tauri'ye bağlı kod ancak CI'da derleniyor;
//! bir kez de derlenmeyen kod ittim ve derleme kırıldı. O yüzden kripto
//! kararlarının hepsi buraya toplandı: burası bağımsız bir kasada test
//! edilebiliyor, `kasa.rs` ve `lib.rs` yalnız taşıma ve tutkal kalıyor.
//!
//! Kayıt şu zincirle korunuyor:
//!   kasa anahtarı (VK) ──► kayıt anahtarı (IK) ──► alanlar
//! IK her kayıtta ayrı: tek bir kaydı döndürmek için bütün kasayı yeniden
//! şifrelemek gerekmiyor.

use crate::kripto;

/// Sunucuya gönderilecek zarf kümesi.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct KayitZarfi {
    pub kayit_anahtar_zarf: String,
    pub parola_zarf: Option<String>,
    pub kullanici_zarf: Option<String>,
    pub ekstra_zarf: Option<String>,
    pub notlar_zarf: Option<String>,
}

/// Şifrelenecek düz alanlar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuzKayit {
    pub parola: Option<String>,
    pub kullanici: Option<String>,
    pub ekstra: Option<String>,
    pub notlar: Option<String>,
}

/// Kaydı şifrele. Her çağrıda YENİ bir kayıt anahtarı üretiliyor.
///
/// Aynı kaydı iki kez şifrelemek iki farklı IK üretir; eski zarf hâlâ eski
/// IK ile açılabilir. Anahtar tazeleme buradan bedava geliyor.
pub fn sifrele(
    vk: &[u8],
    kullanici_id: i64,
    kasa_id: i64,
    kayit_id: i64,
    duz: &DuzKayit,
) -> Result<KayitZarfi, String> {
    let ik = kripto::anahtar_uret();

    let alan = |ad: &str| kripto::aad_kayit(kullanici_id, kasa_id, kayit_id, ad);
    let sar = |deger: &Option<String>, ad: &str| -> Result<Option<String>, String> {
        match deger {
            Some(d) if !d.is_empty() => Ok(Some(kripto::sifrele(&ik, d.as_bytes(), &alan(ad))?)),
            _ => Ok(None),
        }
    };

    Ok(KayitZarfi {
        kayit_anahtar_zarf: kripto::sifrele(vk, &ik, &alan("ikey"))?,
        parola_zarf: sar(&duz.parola, "parola")?,
        kullanici_zarf: sar(&duz.kullanici, "kullanici")?,
        ekstra_zarf: sar(&duz.ekstra, "ekstra")?,
        notlar_zarf: sar(&duz.notlar, "notlar")?,
    })
}

/// Kaydı çöz.
///
/// Bir alan yoksa `None` döner; ÇÖZÜLEMİYORSA hata döner. İkisini
/// karıştırmak, bozuk bir zarfı "boş alan" sanmak demek olurdu — kullanıcı
/// parolasının kaybolduğunu ancak ihtiyacı olduğu an fark ederdi.
pub fn coz(
    vk: &[u8],
    kullanici_id: i64,
    kasa_id: i64,
    kayit_id: i64,
    z: &KayitZarfi,
) -> Result<DuzKayit, String> {
    let alan = |ad: &str| kripto::aad_kayit(kullanici_id, kasa_id, kayit_id, ad);
    let ik = kripto::coz(vk, &z.kayit_anahtar_zarf, &alan("ikey"))
        .map_err(|e| format!("Kayıt anahtarı açılamadı: {e}"))?;

    let ac = |zarf: &Option<String>, ad: &str| -> Result<Option<String>, String> {
        match zarf {
            None => Ok(None),
            Some(s) => {
                let ham = kripto::coz(&ik, s, &alan(ad))
                    .map_err(|e| format!("{ad} açılamadı: {e}"))?;
                String::from_utf8(ham)
                    .map(Some)
                    .map_err(|_| format!("{ad} geçerli metin değil"))
            }
        }
    };

    Ok(DuzKayit {
        parola: ac(&z.parola_zarf, "parola")?,
        kullanici: ac(&z.kullanici_zarf, "kullanici")?,
        ekstra: ac(&z.ekstra_zarf, "ekstra")?,
        notlar: ac(&z.notlar_zarf, "notlar")?,
    })
}

/// Sunucudan gelen JSON'u zarf kümesine çevir.
pub fn jsondan(j: &serde_json::Value) -> Result<KayitZarfi, String> {
    let al = |ad: &str| j[ad].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string());
    Ok(KayitZarfi {
        kayit_anahtar_zarf: al("kayit_anahtar_zarf")
            .ok_or("Kayıt anahtarı zarfı yok — kayıt zero-knowledge değil.")?,
        parola_zarf: al("parola_zarf"),
        kullanici_zarf: al("kullanici_zarf"),
        ekstra_zarf: al("ekstra_zarf"),
        notlar_zarf: al("notlar_zarf"),
    })
}

/// KURULUM: ana paroladan bütün kimlik malzemesini üret.
///
/// Sunucuya giden her şey burada hazırlanıyor; ana parola ve kurtarma
/// anahtarı ÇAĞIRANDA kalıyor, dönen yapıya girmiyor.
pub struct Kurulum {
    /// Sunucuya gönderilecek gövde.
    pub govde: serde_json::Value,
    /// Kullanıcıya BİR KEZ gösterilecek kurtarma anahtarı.
    pub kurtarma_anahtari: String,
    /// Bellekte tutulacak anahtarlar.
    pub kilit: [u8; 32],
    pub ozel: [u8; 32],
}

pub fn kurulum_hazirla(kullanici_id: i64, ana_parola: &str) -> Result<Kurulum, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    if ana_parola.chars().count() < 10 {
        return Err("Ana parola en az 10 karakter olmalı.".into());
    }

    let p = kripto::KdfParam::default();
    let tuz = kripto::rastgele(16);
    let kilit = kripto::kilit_anahtari(ana_parola, &tuz, p)?;
    let (ozel, acik) = kripto::anahtar_cifti();

    let kurtarma = kripto::kurtarma_uret();
    let k_tuz = kripto::rastgele(16);
    let k_kilit = kripto::kurtarma_kilit_anahtari(&kurtarma, &k_tuz)?;

    let govde = serde_json::json!({
        "kdf_tuz": B64.encode(&tuz),
        "kdf_param": { "bellek": p.bellek, "tur": p.tur, "paralel": p.paralel },
        "acik_anahtar": B64.encode(acik),
        "ozel_anahtar_zarf": kripto::sifrele(&kilit, &ozel, &kripto::aad_ozel_anahtar(kullanici_id))?,
        "kurtarma_tuz": B64.encode(&k_tuz),
        "kurtarma_zarf": kripto::sifrele(&k_kilit, &ozel, &kripto::aad_kurtarma(kullanici_id))?,
    });

    Ok(Kurulum { govde, kurtarma_anahtari: kurtarma, kilit, ozel })
}

/// KİLİT AÇMA: sunucudan gelen kimlik + ana parola → özel anahtar.
pub fn kilidi_ac(
    kullanici_id: i64,
    ana_parola: &str,
    kimlik: &serde_json::Value,
) -> Result<([u8; 32], [u8; 32]), String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let tuz = B64
        .decode(kimlik["kdf_tuz"].as_str().unwrap_or(""))
        .map_err(|_| "Tuz okunamadı.".to_string())?;
    let p = kripto::KdfParam {
        bellek: kimlik["kdf_param"]["bellek"].as_u64().unwrap_or(65536) as u32,
        tur: kimlik["kdf_param"]["tur"].as_u64().unwrap_or(3) as u32,
        paralel: kimlik["kdf_param"]["paralel"].as_u64().unwrap_or(4) as u32,
    };
    let kilit = kripto::kilit_anahtari(ana_parola, &tuz, p)?;

    let zarf = kimlik["ozel_anahtar_zarf"].as_str().unwrap_or("");
    let ozel_ham = kripto::coz(&kilit, zarf, &kripto::aad_ozel_anahtar(kullanici_id))
        .map_err(|_| "Ana parola hatalı.".to_string())?;

    let ozel: [u8; 32] = ozel_ham
        .try_into()
        .map_err(|_| "Özel anahtar bozuk.".to_string())?;
    Ok((kilit, ozel))
}

/// KURTARMA ANAHTARIYLA AÇMA — ana parola unutulduğunda.
pub fn kurtarma_ile_ac(
    kullanici_id: i64,
    kurtarma_anahtari: &str,
    kimlik: &serde_json::Value,
) -> Result<[u8; 32], String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let tuz = B64
        .decode(kimlik["kurtarma_tuz"].as_str().unwrap_or(""))
        .map_err(|_| "Kurtarma tuzu okunamadı.".to_string())?;
    let kilit = kripto::kurtarma_kilit_anahtari(kurtarma_anahtari, &tuz)?;

    let zarf = kimlik["kurtarma_zarf"].as_str().unwrap_or("");
    let ozel_ham = kripto::coz(&kilit, zarf, &kripto::aad_kurtarma(kullanici_id))
        .map_err(|_| "Kurtarma anahtarı hatalı.".to_string())?;

    ozel_ham.try_into().map_err(|_| "Özel anahtar bozuk.".to_string())
}

/// Kasa anahtarını üyenin açık anahtarına mühürle — kasa kurulurken ve
/// üye eklenirken.
pub fn kasa_anahtari_muhurle(
    acik_anahtar_b64: &str,
    vk: &[u8],
    kasa_id: i64,
    uye_id: i64,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let acik = B64
        .decode(acik_anahtar_b64)
        .map_err(|_| "Açık anahtar okunamadı.".to_string())?;
    kripto::muhurle(&acik, vk, &kripto::aad_kasa_anahtari(kasa_id, uye_id))
}

/// Kasa anahtarını kendi özel anahtarınla aç.
pub fn kasa_anahtari_ac(
    ozel: &[u8],
    zarf: &str,
    kasa_id: i64,
    uye_id: i64,
) -> Result<[u8; 32], String> {
    let ham = kripto::muhur_ac(ozel, zarf, &kripto::aad_kasa_anahtari(kasa_id, uye_id))?;
    ham.try_into().map_err(|_| "Kasa anahtarı bozuk.".to_string())
}

#[cfg(test)]
mod testler {
    use super::*;

    fn ornek() -> DuzKayit {
        DuzKayit {
            parola: Some("S3rver!Root#2026".into()),
            kullanici: Some("root".into()),
            ekstra: None,
            notlar: Some("Notlar — Türkçe karakter: şğüöçİ".into()),
        }
    }

    #[test]
    fn kayit_dolasir() {
        let vk = kripto::anahtar_uret();
        let d = ornek();
        let z = sifrele(&vk, 8, 3, 56, &d).unwrap();
        assert_eq!(coz(&vk, 8, 3, 56, &z).unwrap(), d);
    }

    #[test]
    fn bos_alanlar_zarfsiz_kalir() {
        let vk = kripto::anahtar_uret();
        let d = DuzKayit { parola: Some("x".into()), ..Default::default() };
        let z = sifrele(&vk, 1, 1, 1, &d).unwrap();
        assert!(z.parola_zarf.is_some());
        assert!(z.kullanici_zarf.is_none());
        assert_eq!(coz(&vk, 1, 1, 1, &z).unwrap().kullanici, None);
    }

    /// AAD BAĞLAMI: zarf başka bir kayda taşınırsa çözülmemeli.
    #[test]
    fn baska_kayitta_cozulmez() {
        let vk = kripto::anahtar_uret();
        let z = sifrele(&vk, 8, 3, 56, &ornek()).unwrap();
        assert!(coz(&vk, 8, 3, 57, &z).is_err());
        assert!(coz(&vk, 8, 4, 56, &z).is_err());
        assert!(coz(&vk, 9, 3, 56, &z).is_err());
    }

    #[test]
    fn baska_kasa_anahtariyla_cozulmez() {
        let z = sifrele(&kripto::anahtar_uret(), 8, 3, 56, &ornek()).unwrap();
        assert!(coz(&kripto::anahtar_uret(), 8, 3, 56, &z).is_err());
    }

    /// Aynı kaydı iki kez şifrelemek FARKLI kayıt anahtarı üretmeli.
    #[test]
    fn her_sifrelemede_yeni_kayit_anahtari() {
        let vk = kripto::anahtar_uret();
        let a = sifrele(&vk, 1, 1, 1, &ornek()).unwrap();
        let b = sifrele(&vk, 1, 1, 1, &ornek()).unwrap();
        assert_ne!(a.kayit_anahtar_zarf, b.kayit_anahtar_zarf);
        assert_ne!(a.parola_zarf, b.parola_zarf);
        // İkisi de aynı düz metni vermeli.
        assert_eq!(coz(&vk, 1, 1, 1, &a).unwrap(), coz(&vk, 1, 1, 1, &b).unwrap());
    }

    #[test]
    fn bozuk_zarf_hata_verir_bos_donmez() {
        let vk = kripto::anahtar_uret();
        let mut z = sifrele(&vk, 1, 1, 1, &ornek()).unwrap();
        z.parola_zarf = Some("bh1.AAAA.BBBB".into());
        // "boş parola" değil, HATA dönmeli.
        assert!(coz(&vk, 1, 1, 1, &z).is_err());
    }

    #[test]
    fn jsondan_okur() {
        let vk = kripto::anahtar_uret();
        let z = sifrele(&vk, 1, 1, 1, &ornek()).unwrap();
        let j = serde_json::to_value(&z).unwrap();
        assert_eq!(jsondan(&j).unwrap(), z);
    }

    #[test]
    fn jsondan_anahtarsiz_zarf_reddedilir() {
        let j = serde_json::json!({ "parola_zarf": "bh1.x.y" });
        assert!(jsondan(&j).is_err());
    }

    #[test]
    fn kisa_ana_parola_reddedilir() {
        assert!(kurulum_hazirla(8, "kisa").is_err());
    }

    /// UÇTAN UCA: kurulum → kilit aç → kasa anahtarı → kayıt.
    #[test]
    fn tam_akis() {
        let u = 8i64;
        let kasa = 3i64;
        let kayit = 56i64;

        let k = kurulum_hazirla(u, "cok-guclu-ana-parola").unwrap();
        // Sunucuda duracak kimlik.
        let kimlik = k.govde.clone();

        // Başka bir cihazda kilidi aç.
        let (_kilit2, ozel2) = kilidi_ac(u, "cok-guclu-ana-parola", &kimlik).unwrap();
        assert_eq!(ozel2, k.ozel);

        // Yanlış parola.
        assert!(kilidi_ac(u, "yanlis-parola-uzun", &kimlik).is_err());

        // Kurtarma anahtarı aynı özel anahtarı vermeli.
        let ozel3 = kurtarma_ile_ac(u, &k.kurtarma_anahtari, &kimlik).unwrap();
        assert_eq!(ozel3, k.ozel);

        // Kasa anahtarı mühürlenip açılıyor.
        let vk = kripto::anahtar_uret();
        let acik_b64 = kimlik["acik_anahtar"].as_str().unwrap();
        let vk_zarf = kasa_anahtari_muhurle(acik_b64, &vk, kasa, u).unwrap();
        let vk2 = kasa_anahtari_ac(&ozel2, &vk_zarf, kasa, u).unwrap();
        assert_eq!(vk2, vk);

        // Kayıt yazılıp okunuyor.
        let d = ornek();
        let z = sifrele(&vk2, u, kasa, kayit, &d).unwrap();
        assert_eq!(coz(&vk2, u, kasa, kayit, &z).unwrap(), d);
    }
}
