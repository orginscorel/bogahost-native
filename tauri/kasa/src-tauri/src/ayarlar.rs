//! KULLANICI AYARLARI — diskte, sade bir JSON.
//!
//! NEDEN GEREKLİ: otomatik kilitlenme süresi, pano temizleme süresi ve
//! "doldurunca Enter'a bas" tercihi koda gömülüydü. Kullanıcının
//! değiştiremediği bir değer ayar değil, varsayımdır — ve varsayım her
//! kullanıcıya uymaz.
//!
//! NEDEN AYRI DOSYA, İŞLETİM SİSTEMİ KASASI DEĞİL: burada sır yok. Jeton
//! Keychain'de duruyor; bunlar yalnız tercih. Tercihleri kasaya koymak,
//! kasayı gereksiz yere açtırmak demek olurdu.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ayar {
    /// Kasa kaç dakika sonra kendiliğinden kilitlensin. 0 = kilitlenme.
    pub otomatik_kilit_dk: u64,
    /// Panoya alınan parola kaç saniye sonra silinsin. 0 = silme.
    pub pano_temizleme_sn: u64,
    /// Doldurduktan sonra Enter'a basılsın mı.
    pub doldurunca_enter: bool,
    /// Eşleşme olduğunda panel kendiliğinden ekrana gelsin mi.
    pub panel_kendiliginden: bool,
    /// Doldurulan sitede panel kaç dakika sussun.
    pub sessizlik_dk: u64,
}

impl Default for Ayar {
    fn default() -> Self {
        Self {
            otomatik_kilit_dk: 15,
            pano_temizleme_sn: 30,
            doldurunca_enter: true,
            panel_kendiliginden: true,
            sessizlik_dk: 20,
        }
    }
}

impl Ayar {
    /// Değerleri makul sınırlara çeker.
    ///
    /// Arayüzden gelen değere GÜVENMİYORUZ: pano temizlemeyi 9999 saniyeye
    /// çekmek parolayı saatlerce panoda bırakmak demek. Sınırlar burada,
    /// çünkü ayarı okuyan her yer aynı sınırı görmeli.
    pub fn duzelt(mut self) -> Self {
        if self.otomatik_kilit_dk > 480 {
            self.otomatik_kilit_dk = 480;              // en fazla 8 saat
        }
        if self.pano_temizleme_sn > 300 {
            self.pano_temizleme_sn = 300;              // en fazla 5 dakika
        }
        if self.pano_temizleme_sn != 0 && self.pano_temizleme_sn < 5 {
            self.pano_temizleme_sn = 5;                // 1 sn'de kopyalanamaz
        }
        if self.sessizlik_dk > 240 {
            self.sessizlik_dk = 240;
        }
        self
    }
}

fn yol() -> Option<std::path::PathBuf> {
    let ev = std::env::var("HOME")
        .or_else(|_| std::env::var("APPDATA"))
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let d = std::path::Path::new(&ev).join(".bogahost-kasa");
    let _ = std::fs::create_dir_all(&d);
    Some(d.join("ayarlar.json"))
}

/// Ayarları oku. Dosya yoksa ya da bozuksa VARSAYILANA döner — bozuk bir
/// ayar dosyası yüzünden uygulamanın açılmaması kabul edilemez.
pub fn oku() -> Ayar {
    let Some(p) = yol() else { return Ayar::default() };
    let Ok(metin) = std::fs::read_to_string(p) else { return Ayar::default() };
    serde_json::from_str::<Ayar>(&metin)
        .map(|a| a.duzelt())
        .unwrap_or_default()
}

pub fn yaz(a: &Ayar) -> Result<(), String> {
    let p = yol().ok_or("Ayar klasörü bulunamadı.")?;
    let metin = serde_json::to_string_pretty(a).map_err(|e| e.to_string())?;
    std::fs::write(&p, metin).map_err(|e| format!("Ayarlar yazılamadı: {e}"))?;

    // Dosya yalnız kullanıcıya okunur olsun. Sır taşımıyor ama makinede
    // birden fazla hesap varsa tercihler de kimseyi ilgilendirmez.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn varsayilanlar_makul() {
        let a = Ayar::default();
        assert_eq!(a.otomatik_kilit_dk, 15);
        assert_eq!(a.pano_temizleme_sn, 30);
        assert!(a.doldurunca_enter);
    }

    #[test]
    fn asiri_degerler_kirpilir() {
        let a = Ayar { otomatik_kilit_dk: 99999, pano_temizleme_sn: 9999,
                       sessizlik_dk: 9999, ..Default::default() }.duzelt();
        assert_eq!(a.otomatik_kilit_dk, 480);
        assert_eq!(a.pano_temizleme_sn, 300);
        assert_eq!(a.sessizlik_dk, 240);
    }

    #[test]
    fn cok_kisa_pano_suresi_yukseltilir() {
        let a = Ayar { pano_temizleme_sn: 1, ..Default::default() }.duzelt();
        assert_eq!(a.pano_temizleme_sn, 5);
    }

    /// SIFIR "KAPALI" DEMEK, kırpılmamalı.
    #[test]
    fn sifir_korunur() {
        let a = Ayar { otomatik_kilit_dk: 0, pano_temizleme_sn: 0, ..Default::default() }.duzelt();
        assert_eq!(a.otomatik_kilit_dk, 0);
        assert_eq!(a.pano_temizleme_sn, 0);
    }

    #[test]
    fn json_dolasir() {
        let a = Ayar { otomatik_kilit_dk: 5, doldurunca_enter: false, ..Default::default() };
        let m = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<Ayar>(&m).unwrap(), a);
    }

    /// Bozuk dosya uygulamayı düşürmemeli.
    #[test]
    fn bozuk_json_varsayilana_doner() {
        assert!(serde_json::from_str::<Ayar>("{bozuk").is_err());
        // oku() bunu yakalayıp varsayılana dönüyor; burada niyeti sabitliyoruz.
        let a: Ayar = serde_json::from_str("{bozuk").unwrap_or_default();
        assert_eq!(a, Ayar::default());
    }

    /// Eksik alanlı eski dosya da okunabilmeli mi? HAYIR — serde tüm
    /// alanları zorunlu tutuyor ve okuma başarısız olunca varsayılana
    /// dönüyoruz. Bu bilinçli: yarım bir ayar dosyasını "kısmen uygula"
    /// demek, hangi değerin nereden geldiğini takip edilemez yapar.
    #[test]
    fn eksik_alanli_dosya_varsayilana_doner() {
        let a: Ayar = serde_json::from_str(r#"{"otomatik_kilit_dk":5}"#).unwrap_or_default();
        assert_eq!(a, Ayar::default());
    }
}
