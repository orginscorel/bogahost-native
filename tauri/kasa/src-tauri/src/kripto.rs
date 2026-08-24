//! ZERO-KNOWLEDGE KRİPTO ÇEKİRDEĞİ — Faz 3.
//!
//! DENETİM BULGUSU K-1: parolalar `Crypt::encryptString()` ile, yani
//! SUNUCUDAKİ `APP_KEY` ile şifreleniyordu. Anahtar veritabanıyla aynı
//! makinede duruyor; sunucuya erişen herkes her parolayı okuyabiliyordu.
//! Koddaki "DB sızsa bile içerik okunamaz" yorumu yalnız ANAHTARSIZ bir
//! sızıntı için doğruydu.
//!
//! Bu modülden sonra sunucu düz metni HİÇ görmüyor. Şifreleme burada,
//! kullanıcının makinesinde yapılıyor; sunucuya yalnız ciphertext gidiyor.
//!
//! ANAHTAR HİYERARŞİSİ
//!
//!   Ana parola + kişiye özel tuz
//!     └─ Argon2id ──► Kilit Anahtarı (KA)      ← sunucuya ASLA gitmez
//!          ├─ X25519 özel anahtarı KA ile şifreli (sunucuda ciphertext)
//!          └─ açık anahtar düz (zaten açık)
//!
//!   Her kasanın bir Kasa Anahtarı (VK) var.
//!     Her üye için VK, o üyenin AÇIK anahtarına mühürlenip saklanıyor.
//!     Üye çıkarılınca zarfı silinir → gelecekteki erişimi biter.
//!
//!   Her kaydın kendi Kayıt Anahtarı (IK) var, VK ile şifreli.
//!     Tek bir kaydı döndürmek için bütün kasayı yeniden şifrelemek
//!     gerekmiyor.
//!
//! NEDEN XChaCha20-Poly1305: nonce 24 bayt, yani rastgele üretilen nonce'un
//! tekrar etme olasılığı pratikte sıfır. AES-GCM'in 12 baytlık nonce'unda
//! aynı anahtarla çok sayıda şifreleme yapılınca sayaç tutmak gerekir —
//! sayaç tutmayı unutmak, bu sınıfın en yaygın felaketi.
//!
//! KENDİ ALGORİTMAMIZI YAZMIYORUZ. Tamamı denetlenmiş RustCrypto
//! kasalarından geliyor.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

/// Zarf sürümü. Şifreleme biçimi değişirse bu artar ve eski ciphertext
/// çözülmeye devam eder — denetimde istenen geriye dönük uyumluluk.
pub const SURUM: u8 = 1;

/// Sürüm etiketi: `bh1.` ile başlayan her şey bu modülün ürettiğidir.
const ONEK: &str = "bh1.";

pub const ANAHTAR_BOYU: usize = 32;
const NONCE_BOYU: usize = 24;

// ── KDF ────────────────────────────────────────────────────────────────────

/// Argon2id parametreleri. Sunucuda SAKLANIR ama sunucu için değil:
/// kullanıcı başka bir cihazda giriş yaptığında aynı anahtarı türetebilsin
/// diye. Parametrelerin sürümlenmiş olması, ileride donanım güçlendikçe
/// maliyeti artırıp ESKİ kayıtları bozmadan yükseltmeyi mümkün kılıyor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KdfParam {
    /// KiB cinsinden bellek.
    pub bellek: u32,
    pub tur: u32,
    pub paralel: u32,
}

impl Default for KdfParam {
    /// OWASP'ın Argon2id için önerdiği alt sınırın üstünde bir başlangıç:
    /// 64 MiB bellek, 3 tur. Masaüstünde ~0.2 sn sürüyor; saldırgan için
    /// bellek maliyeti asıl caydırıcı olan.
    fn default() -> Self {
        Self { bellek: 64 * 1024, tur: 3, paralel: 4 }
    }
}

/// Ana parola + tuz → Kilit Anahtarı. Sunucuya asla gönderilmez.
pub fn kilit_anahtari(
    ana_parola: &str,
    tuz: &[u8],
    p: KdfParam,
) -> Result<[u8; ANAHTAR_BOYU], String> {
    if tuz.len() < 16 {
        return Err("Tuz en az 16 bayt olmalı.".into());
    }
    let params = Params::new(p.bellek, p.tur, p.paralel, Some(ANAHTAR_BOYU))
        .map_err(|e| format!("Argon2 parametreleri geçersiz: {e}"))?;
    let a2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut cikti = [0u8; ANAHTAR_BOYU];
    a2.hash_password_into(ana_parola.as_bytes(), tuz, &mut cikti)
        .map_err(|e| format!("Anahtar türetilemedi: {e}"))?;
    Ok(cikti)
}

pub fn rastgele(n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    OsRng.fill_bytes(&mut v);
    v
}

pub fn anahtar_uret() -> [u8; ANAHTAR_BOYU] {
    let mut k = [0u8; ANAHTAR_BOYU];
    OsRng.fill_bytes(&mut k);
    k
}

// ── Simetrik şifreleme ─────────────────────────────────────────────────────

/// Şifrele. Sonuç: `bh1.<b64 nonce>.<b64 ciphertext>`
///
/// AAD (ek doğrulanmış veri) ciphertext'i BAĞLAMINA bağlıyor. Bir kaydın
/// şifreli gövdesi başka bir kasaya ya da başka bir kayda taşınıp
/// çözülemiyor: AAD uyuşmazsa çözme başarısız oluyor. Bunu yapmasaydık
/// sunucudaki biri satırları yer değiştirerek anlamlı bir saldırı
/// kurabilirdi.
pub fn sifrele(anahtar: &[u8], duz: &[u8], aad: &str) -> Result<String, String> {
    let c = XChaCha20Poly1305::new_from_slice(anahtar)
        .map_err(|_| "Anahtar boyu hatalı.".to_string())?;
    let nonce_ham = rastgele(NONCE_BOYU);
    let nonce = XNonce::from_slice(&nonce_ham);

    let ct = c
        .encrypt(nonce, Payload { msg: duz, aad: aad.as_bytes() })
        .map_err(|_| "Şifreleme başarısız.".to_string())?;

    Ok(format!("{ONEK}{}.{}", B64.encode(&nonce_ham), B64.encode(&ct)))
}

/// Çöz. AAD birebir aynı olmalı.
pub fn coz(anahtar: &[u8], zarf: &str, aad: &str) -> Result<Vec<u8>, String> {
    let govde = zarf
        .strip_prefix(ONEK)
        .ok_or_else(|| "Tanınmayan zarf sürümü.".to_string())?;
    let (n_b64, ct_b64) = govde
        .split_once('.')
        .ok_or_else(|| "Zarf bozuk.".to_string())?;

    let nonce_ham = B64.decode(n_b64).map_err(|_| "Nonce çözülemedi.".to_string())?;
    if nonce_ham.len() != NONCE_BOYU {
        return Err("Nonce boyu hatalı.".into());
    }
    let ct = B64.decode(ct_b64).map_err(|_| "Ciphertext çözülemedi.".to_string())?;

    let c = XChaCha20Poly1305::new_from_slice(anahtar)
        .map_err(|_| "Anahtar boyu hatalı.".to_string())?;

    c.decrypt(
        XNonce::from_slice(&nonce_ham),
        Payload { msg: &ct, aad: aad.as_bytes() },
    )
    .map_err(|_| "Çözülemedi: anahtar ya da bağlam uyuşmuyor.".to_string())
}

/// Bir zarfın bu modülden çıkıp çıkmadığı — göç sırasında hangi kaydın
/// hangi sürümde olduğunu anlamak için.
pub fn bizim_mi(zarf: &str) -> bool {
    zarf.starts_with(ONEK)
}

// ── Açık anahtara mühürleme ────────────────────────────────────────────────

/// Bir anahtarı (VK) alıcının AÇIK anahtarına mühürler.
///
/// Geçici bir X25519 çifti üretilip alıcının açık anahtarıyla ortak sır
/// hesaplanıyor, HKDF ile şifreleme anahtarına dönüştürülüyor. Gönderenin
/// kalıcı bir anahtara sahip olması gerekmiyor: kasa anahtarını üyeye
/// dağıtmak için "kim gönderdi" sorusunun cevabına ihtiyacımız yok, üyelik
/// zaten sunucuda yetkilendirilmiş durumda.
///
/// Sonuç: `bh1.<b64 gecici_acik>.<b64 nonce>.<b64 ct>`
pub fn muhurle(alici_acik: &[u8], veri: &[u8], aad: &str) -> Result<String, String> {
    if alici_acik.len() != 32 {
        return Err("Açık anahtar 32 bayt olmalı.".into());
    }
    let mut ham = [0u8; 32];
    ham.copy_from_slice(alici_acik);
    let alici = PublicKey::from(ham);

    let gecici = StaticSecret::random_from_rng(OsRng);
    let gecici_acik = PublicKey::from(&gecici);
    let mut ortak = gecici.diffie_hellman(&alici).to_bytes();

    let mut anahtar = [0u8; ANAHTAR_BOYU];
    Hkdf::<Sha256>::new(Some(gecici_acik.as_bytes()), &ortak)
        .expand(b"bogahost-kasa-muhur-v1", &mut anahtar)
        .map_err(|_| "HKDF başarısız.".to_string())?;
    ortak.zeroize();

    let ic = sifrele(&anahtar, veri, aad)?;
    anahtar.zeroize();

    // İç zarfın önekini tekrarlamıyoruz; dış zarf zaten sürümlü.
    let ic_govde = ic.strip_prefix(ONEK).unwrap_or(&ic);
    Ok(format!("{ONEK}{}.{ic_govde}", B64.encode(gecici_acik.as_bytes())))
}

/// Mühürlü zarfı kendi ÖZEL anahtarınla aç.
pub fn muhur_ac(ozel: &[u8], zarf: &str, aad: &str) -> Result<Vec<u8>, String> {
    if ozel.len() != 32 {
        return Err("Özel anahtar 32 bayt olmalı.".into());
    }
    let govde = zarf
        .strip_prefix(ONEK)
        .ok_or_else(|| "Tanınmayan zarf sürümü.".to_string())?;
    let (gecici_b64, kalan) = govde
        .split_once('.')
        .ok_or_else(|| "Mühür bozuk.".to_string())?;

    let gecici_ham = B64
        .decode(gecici_b64)
        .map_err(|_| "Geçici anahtar çözülemedi.".to_string())?;
    if gecici_ham.len() != 32 {
        return Err("Geçici anahtar boyu hatalı.".into());
    }
    let mut g = [0u8; 32];
    g.copy_from_slice(&gecici_ham);
    let gecici_acik = PublicKey::from(g);

    let mut o = [0u8; 32];
    o.copy_from_slice(ozel);
    let benim = StaticSecret::from(o);
    let mut ortak = benim.diffie_hellman(&gecici_acik).to_bytes();

    let mut anahtar = [0u8; ANAHTAR_BOYU];
    Hkdf::<Sha256>::new(Some(gecici_acik.as_bytes()), &ortak)
        .expand(b"bogahost-kasa-muhur-v1", &mut anahtar)
        .map_err(|_| "HKDF başarısız.".to_string())?;
    ortak.zeroize();

    let sonuc = coz(&anahtar, &format!("{ONEK}{kalan}"), aad);
    anahtar.zeroize();
    sonuc
}

/// Yeni kullanıcı anahtar çifti. Özel anahtar ÇAĞIRANA ait; sunucuya
/// gönderilmeden önce kilit anahtarıyla şifrelenmeli.
pub fn anahtar_cifti() -> ([u8; 32], [u8; 32]) {
    let ozel = StaticSecret::random_from_rng(OsRng);
    let acik = PublicKey::from(&ozel);
    (ozel.to_bytes(), acik.to_bytes())
}

// ── Bağlam etiketleri ──────────────────────────────────────────────────────

/// AAD üretimi tek yerde. İki tarafın farklı AAD kurması, çözülemeyen
/// kayıtlar demek — bu yüzden biçim burada sabit.
pub fn aad_kayit(kullanici: i64, kasa: i64, kayit: i64, alan: &str) -> String {
    format!("v{SURUM}|u{kullanici}|k{kasa}|i{kayit}|{alan}")
}

pub fn aad_kasa_anahtari(kasa: i64, uye: i64) -> String {
    format!("v{SURUM}|kasa{kasa}|uye{uye}")
}

pub fn aad_ozel_anahtar(kullanici: i64) -> String {
    format!("v{SURUM}|ozel|u{kullanici}")
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn sifrele_coz_dolasir() {
        let k = anahtar_uret();
        let aad = aad_kayit(1, 2, 3, "parola");
        let z = sifrele(&k, b"gizli-parola", &aad).unwrap();
        assert!(bizim_mi(&z));
        assert_eq!(coz(&k, &z, &aad).unwrap(), b"gizli-parola");
    }

    #[test]
    fn yanlis_anahtar_cozemez() {
        let k = anahtar_uret();
        let baska = anahtar_uret();
        let aad = aad_kayit(1, 2, 3, "parola");
        let z = sifrele(&k, b"x", &aad).unwrap();
        assert!(coz(&baska, &z, &aad).is_err());
    }

    /// EN ÖNEMLİ TEST: bir kaydın şifreli gövdesi başka bir kayda
    /// taşınırsa çözülmemeli. AAD bunun için var.
    #[test]
    fn baska_baglamda_cozemez() {
        let k = anahtar_uret();
        let z = sifrele(&k, b"x", &aad_kayit(1, 2, 3, "parola")).unwrap();
        assert!(coz(&k, &z, &aad_kayit(1, 2, 4, "parola")).is_err());
        assert!(coz(&k, &z, &aad_kayit(9, 2, 3, "parola")).is_err());
        assert!(coz(&k, &z, &aad_kayit(1, 2, 3, "kullanici")).is_err());
    }

    #[test]
    fn ciphertext_kurcalanirsa_cozemez() {
        let k = anahtar_uret();
        let aad = aad_kayit(1, 2, 3, "parola");
        let z = sifrele(&k, b"gizli", &aad).unwrap();
        let mut bozuk: Vec<char> = z.chars().collect();
        let son = bozuk.len() - 2;
        bozuk[son] = if bozuk[son] == 'A' { 'B' } else { 'A' };
        let bozuk: String = bozuk.into_iter().collect();
        assert!(coz(&k, &bozuk, &aad).is_err());
    }

    #[test]
    fn nonce_asla_tekrar_etmez() {
        let k = anahtar_uret();
        let aad = aad_kayit(1, 2, 3, "parola");
        let mut gorulen = std::collections::HashSet::new();
        for _ in 0..2000 {
            let z = sifrele(&k, b"ayni-metin", &aad).unwrap();
            let nonce = z.strip_prefix(ONEK).unwrap().split('.').next().unwrap().to_string();
            assert!(gorulen.insert(nonce), "nonce tekrar etti");
        }
        // Aynı düz metin her seferinde FARKLI ciphertext vermeli.
        let a = sifrele(&k, b"ayni-metin", &aad).unwrap();
        let b = sifrele(&k, b"ayni-metin", &aad).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn kdf_ayni_girdiye_ayni_anahtar() {
        let tuz = rastgele(16);
        let p = KdfParam::default();
        let a = kilit_anahtari("ana-parola", &tuz, p).unwrap();
        let b = kilit_anahtari("ana-parola", &tuz, p).unwrap();
        assert_eq!(a, b);
        let c = kilit_anahtari("baska-parola", &tuz, p).unwrap();
        assert_ne!(a, c);
        // Tuz değişince anahtar da değişmeli — aynı parolayı kullanan iki
        // kişi aynı anahtara sahip olmasın.
        let d = kilit_anahtari("ana-parola", &rastgele(16), p).unwrap();
        assert_ne!(a, d);
    }

    #[test]
    fn kisa_tuz_reddedilir() {
        assert!(kilit_anahtari("x", &rastgele(8), KdfParam::default()).is_err());
    }

    #[test]
    fn muhur_alicida_acilir() {
        let (ozel, acik) = anahtar_cifti();
        let vk = anahtar_uret();
        let aad = aad_kasa_anahtari(7, 42);
        let z = muhurle(&acik, &vk, &aad).unwrap();
        assert_eq!(muhur_ac(&ozel, &z, &aad).unwrap(), vk.to_vec());
    }

    #[test]
    fn muhur_baskasinda_acilmaz() {
        let (_, acik) = anahtar_cifti();
        let (baska_ozel, _) = anahtar_cifti();
        let vk = anahtar_uret();
        let aad = aad_kasa_anahtari(7, 42);
        let z = muhurle(&acik, &vk, &aad).unwrap();
        assert!(muhur_ac(&baska_ozel, &z, &aad).is_err());
    }

    /// Üye çıkarılınca zarfı silinir; ama elindeki ESKİ zarfı başka bir
    /// kasada kullanamamalı.
    #[test]
    fn muhur_baska_kasada_acilmaz() {
        let (ozel, acik) = anahtar_cifti();
        let vk = anahtar_uret();
        let z = muhurle(&acik, &vk, &aad_kasa_anahtari(7, 42)).unwrap();
        assert!(muhur_ac(&ozel, &z, &aad_kasa_anahtari(8, 42)).is_err());
    }

    /// Uçtan uca: ana parola → kilit anahtarı → özel anahtar → kasa
    /// anahtarı → kayıt anahtarı → parola.
    #[test]
    fn tam_zincir() {
        let kullanici = 8i64;
        let kasa = 3i64;
        let kayit = 56i64;

        // 1. Kurulum
        let tuz = rastgele(16);
        let ka = kilit_anahtari("cok-guclu-ana-parola", &tuz, KdfParam::default()).unwrap();
        let (ozel, acik) = anahtar_cifti();
        let ozel_zarf = sifrele(&ka, &ozel, &aad_ozel_anahtar(kullanici)).unwrap();

        // 2. Kasa açılıyor, anahtarı üyeye mühürleniyor
        let vk = anahtar_uret();
        let vk_zarf = muhurle(&acik, &vk, &aad_kasa_anahtari(kasa, kullanici)).unwrap();

        // 3. Kayıt yazılıyor
        let ik = anahtar_uret();
        let ik_zarf = sifrele(&vk, &ik, &aad_kayit(kullanici, kasa, kayit, "ikey")).unwrap();
        let parola_zarf =
            sifrele(&ik, b"S3rver!Root#2026", &aad_kayit(kullanici, kasa, kayit, "parola")).unwrap();

        // 4. Başka bir cihazda okuma — elde yalnız ana parola var
        let ka2 = kilit_anahtari("cok-guclu-ana-parola", &tuz, KdfParam::default()).unwrap();
        let ozel2 = coz(&ka2, &ozel_zarf, &aad_ozel_anahtar(kullanici)).unwrap();
        let vk2 = muhur_ac(&ozel2, &vk_zarf, &aad_kasa_anahtari(kasa, kullanici)).unwrap();
        let ik2 = coz(&vk2, &ik_zarf, &aad_kayit(kullanici, kasa, kayit, "ikey")).unwrap();
        let parola = coz(&ik2, &parola_zarf, &aad_kayit(kullanici, kasa, kayit, "parola")).unwrap();

        assert_eq!(parola, b"S3rver!Root#2026");

        // YANLIŞ ANA PAROLA ZİNCİRİ KIRAR — sunucudan hiçbir şey sızmadan.
        let yanlis = kilit_anahtari("yanlis-parola", &tuz, KdfParam::default()).unwrap();
        assert!(coz(&yanlis, &ozel_zarf, &aad_ozel_anahtar(kullanici)).is_err());
    }
}
