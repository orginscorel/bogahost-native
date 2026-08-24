//! BELLEKTEKİ ANAHTARLAR — Faz 3.
//!
//! Kilit anahtarı, özel anahtar ve açılmış kasa anahtarları YALNIZ BURADA ve
//! yalnız bellekte duruyor. Hiçbiri diske yazılmıyor, hiçbiri sunucuya
//! gitmiyor, hiçbiri arayüze verilmiyor.
//!
//! NEDEN AYRI BİR MODÜL: bu değerlerin nerede durduğu ve ne zaman silindiği
//! tek bir yerde görülebilmeli. Uygulamanın oraya buraya dağılmış anahtar
//! kopyaları olsaydı "kilitlendi" demenin bir anlamı kalmazdı.
//!
//! ZEROIZE: Rust'ın `Drop`'u belleği sıfırlamaz, yalnız serbest bırakır.
//! Serbest bırakılan bellek başka bir tahsise düşene kadar eski içeriğini
//! taşır; bellek dökümü alan biri anahtarı orada bulur. Bu yüzden her
//! anahtar açıkça sıfırlanıyor.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

/// Kilidin kendiliğinden kapanma süresi. Bilgisayarını açık bırakıp masadan
/// kalkan biri korunmalı; her işlemde tekrar parola sormak da kullanılamaz
/// bir ürün yapar. On beş dakika ikisinin arası.
pub const VARSAYILAN_KILIT_DK: u64 = 15;

pub struct Anahtarlik {
    kilit_anahtari: Option<[u8; 32]>,
    ozel_anahtar: Option<[u8; 32]>,
    /// Açılmış kasa anahtarları — her kasa için bir kez mühür açılıyor.
    kasa_anahtarlari: HashMap<i64, [u8; 32]>,
    son_kullanim: Option<Instant>,
    kilit_suresi: Option<Duration>,
}

impl Anahtarlik {
    pub fn yeni(kilit_dk: u64) -> Self {
        // `..Default::default()` KULLANILAMAZ: bu tip `Drop` uyguluyor ve
        // Rust, Drop'lu bir tipten alan taşınmasına izin vermiyor. Alanlar
        // tek tek yazılıyor — derleyici bunu yakaladı, çalıştırmadan önce.
        Self {
            kilit_anahtari: None,
            ozel_anahtar: None,
            kasa_anahtarlari: HashMap::new(),
            son_kullanim: None,
            kilit_suresi: if kilit_dk == 0 {
                None                       // 0 = kendiliğinden kilitlenme yok
            } else {
                Some(Duration::from_secs(kilit_dk * 60))
            },
        }
    }

    /// Kilidi aç — kilit anahtarı ve özel anahtar belleğe alınıyor.
    pub fn ac(&mut self, kilit: [u8; 32], ozel: [u8; 32]) {
        self.kilit_anahtari = Some(kilit);
        self.ozel_anahtar = Some(ozel);
        self.kasa_anahtarlari.clear();
        self.son_kullanim = Some(Instant::now());
    }

    /// KİLİTLE — her şeyi sıfırla.
    ///
    /// `clear()` yetmez: HashMap'in içindeki diziler serbest bırakılırken
    /// içerikleri bellekte kalır. Her birini tek tek sıfırlıyoruz.
    pub fn kilitle(&mut self) {
        if let Some(mut k) = self.kilit_anahtari.take() {
            k.zeroize();
        }
        if let Some(mut o) = self.ozel_anahtar.take() {
            o.zeroize();
        }
        for (_, mut v) in self.kasa_anahtarlari.drain() {
            v.zeroize();
        }
        self.son_kullanim = None;
    }

    /// Açık mı? Süresi dolduysa BURADA kilitleniyor — "sonra kontrol ederiz"
    /// diye bırakılan bir süre, süre değildir.
    pub fn acik_mi(&mut self) -> bool {
        if self.ozel_anahtar.is_none() {
            return false;
        }
        if let (Some(sure), Some(son)) = (self.kilit_suresi, self.son_kullanim) {
            if son.elapsed() >= sure {
                self.kilitle();
                return false;
            }
        }
        true
    }

    /// Kilidin kapanmasına kalan saniye — arayüzde geri sayım için.
    pub fn kalan_sn(&self) -> Option<u64> {
        let sure = self.kilit_suresi?;
        let son = self.son_kullanim?;
        Some(sure.saturating_sub(son.elapsed()).as_secs())
    }

    /// Her kullanımda süre yeniden başlıyor — çalışırken kilitlenmesin.
    fn dokun(&mut self) {
        self.son_kullanim = Some(Instant::now());
    }

    pub fn ozel(&mut self) -> Option<[u8; 32]> {
        if !self.acik_mi() {
            return None;
        }
        self.dokun();
        self.ozel_anahtar
    }

    pub fn kilit(&mut self) -> Option<[u8; 32]> {
        if !self.acik_mi() {
            return None;
        }
        self.dokun();
        self.kilit_anahtari
    }

    pub fn kasa_koy(&mut self, kasa: i64, anahtar: [u8; 32]) {
        if self.acik_mi() {
            self.kasa_anahtarlari.insert(kasa, anahtar);
            self.dokun();
        }
    }

    pub fn kasa_al(&mut self, kasa: i64) -> Option<[u8; 32]> {
        if !self.acik_mi() {
            return None;
        }
        self.dokun();
        self.kasa_anahtarlari.get(&kasa).copied()
    }

    /// Kasadan çıkarıldığımızda ya da anahtar döndüğünde o kasanın
    /// belleğe alınmış anahtarını düşür.
    pub fn kasa_dusur(&mut self, kasa: i64) {
        if let Some(mut v) = self.kasa_anahtarlari.remove(&kasa) {
            v.zeroize();
        }
    }

    pub fn kilit_suresini_ayarla(&mut self, dk: u64) {
        self.kilit_suresi = if dk == 0 { None } else { Some(Duration::from_secs(dk * 60)) };
    }
}

/// Uygulama kapanırken de silinsin — çökme dışındaki her yolda çalışır.
impl Drop for Anahtarlik {
    fn drop(&mut self) {
        self.kilitle();
    }
}

#[cfg(test)]
mod testler {
    use super::*;

    fn ornek() -> ([u8; 32], [u8; 32]) {
        ([7u8; 32], [9u8; 32])
    }

    #[test]
    fn baslangicta_kilitli() {
        let mut a = Anahtarlik::yeni(15);
        assert!(!a.acik_mi());
        assert!(a.ozel().is_none());
        assert!(a.kilit().is_none());
    }

    #[test]
    fn acilinca_anahtarlar_gelir() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(15);
        a.ac(k, o);
        assert!(a.acik_mi());
        assert_eq!(a.ozel(), Some(o));
        assert_eq!(a.kilit(), Some(k));
    }

    #[test]
    fn kilitlenince_hicbir_sey_donmez() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(15);
        a.ac(k, o);
        a.kasa_koy(3, [1u8; 32]);
        a.kilitle();
        assert!(!a.acik_mi());
        assert!(a.ozel().is_none());
        assert!(a.kilit().is_none());
        assert!(a.kasa_al(3).is_none());
    }

    #[test]
    fn kasa_anahtarlari_saklanir_ve_dusurulur() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(15);
        a.ac(k, o);
        a.kasa_koy(3, [1u8; 32]);
        a.kasa_koy(4, [2u8; 32]);
        assert_eq!(a.kasa_al(3), Some([1u8; 32]));
        a.kasa_dusur(3);
        assert!(a.kasa_al(3).is_none());
        assert_eq!(a.kasa_al(4), Some([2u8; 32]));
    }

    /// KİLİT AÇILINCA ESKİ KASA ANAHTARLARI KALMAMALI.
    /// Başka bir hesapla açılırsa öncekinin anahtarları belleğe taşınmasın.
    #[test]
    fn yeniden_acmak_kasalari_temizler() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(15);
        a.ac(k, o);
        a.kasa_koy(3, [1u8; 32]);
        a.ac([8u8; 32], [6u8; 32]);
        assert!(a.kasa_al(3).is_none());
    }

    #[test]
    fn sure_dolunca_kendiliginden_kilitlenir() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(15);
        a.ac(k, o);
        // Süreyi geçmişe çekmek yerine süreyi sıfıra yakın yapıyoruz.
        a.kilit_suresi = Some(Duration::from_millis(40));
        assert!(a.acik_mi());
        std::thread::sleep(Duration::from_millis(60));
        assert!(!a.acik_mi(), "süre dolduğu hâlde açık kaldı");
        assert!(a.ozel().is_none());
    }

    #[test]
    fn kullanim_sureyi_yeniler() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(15);
        a.ac(k, o);
        a.kilit_suresi = Some(Duration::from_millis(120));
        for _ in 0..4 {
            std::thread::sleep(Duration::from_millis(50));
            assert!(a.ozel().is_some(), "çalışırken kilitlendi");
        }
    }

    #[test]
    fn sifir_dakika_kendiliginden_kilitlemez() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(0);
        a.ac(k, o);
        assert!(a.kalan_sn().is_none());
        std::thread::sleep(Duration::from_millis(30));
        assert!(a.acik_mi());
    }

    #[test]
    fn kalan_sure_azalir() {
        let (k, o) = ornek();
        let mut a = Anahtarlik::yeni(1);
        a.ac(k, o);
        let ilk = a.kalan_sn().unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        let sonra = a.kalan_sn().unwrap();
        assert!(sonra < ilk, "kalan süre azalmadı: {ilk} -> {sonra}");
    }
}
