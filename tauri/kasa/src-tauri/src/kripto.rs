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

// ── Kurtarma anahtarı ──────────────────────────────────────────────────────
//
// ANA PAROLAYI UNUTMAK TOPLAM KAYIP DEMEK OLMAMALI.
//
// Zero-knowledge mimaride "şifremi unuttum → e-postadan yeni şifre" diye bir
// şey yoktur; sunucu kullanıcının anahtarını bilmiyor ki sıfırlasın. Bunun
// yerine kurulum sırasında BİR KEZ gösterilen yüksek entropili bir kurtarma
// anahtarı üretiliyor ve özel anahtar ONUNLA DA sarılıyor.
//
//   ana parola ──► kilit anahtarı ──┐
//                                    ├──► aynı özel anahtar
//   kurtarma anahtarı ──► kurtarma ──┘
//                         kilit anahtarı
//
// Kurtarma anahtarı sunucuya ASLA gitmez; kullanıcı onu yazdırıp saklar.
// Kaybedilirse ve ana parola da unutulursa kasa gerçekten açılamaz — bu bir
// kusur değil, zero-knowledge'ın tanımı.
//
// NEDEN ARGON2 DEĞİL HKDF: kurtarma anahtarı 256 bit rastgele. Kaba kuvvetle
// denenecek bir "insan parolası" değil, o yüzden yavaşlatmanın anlamı yok.
// Argon2 insanların seçtiği düşük entropili parolaları korumak içindir.

/// Kurtarma anahtarı uzunluğu — 32 bayt = 256 bit.
const KURTARMA_BAYT: usize = 32;

/// İnsanın yazabileceği alfabe — Crockford Base32 düzeni.
///
/// `0 O` · `1 I L` · `U` dışarıda: elle yazılan ya da telefonla okunan bir
/// dizede en sık karışan çiftler bunlar. `U` ayrıca istenmeyen kelimeler
/// oluşmasını azaltmak için yok.
///
/// `B` ve `8` ikisi de var — Crockford'un tercihi bu ve yazı tipi ayrımı
/// genelde yeterli. Karıştırılırsa kurtarma başarısız olur ama sessiz bir
/// yanlış sonuç ÜRETMEZ: çözme ya doğru anahtarı verir ya hata döner.
const ALFABE: &[u8] = b"ABCDEFGHJKMNPQRSTVWXYZ23456789";

/// Yeni kurtarma anahtarı: `XXXXX-XXXXX-...` biçiminde, 10 grup.
pub fn kurtarma_uret() -> String {
    let ham = rastgele(KURTARMA_BAYT);
    let mut harfler = String::new();
    // Her bayttan bir harf; 30 harflik alfabede modulo sapması ihmal
    // edilebilir çünkü asıl entropi aşağıdaki türetmede ham bayttan değil
    // ÜRETİLEN DİZEDEN geliyor: 50 harf × log2(30) ≈ 245 bit.
    for _ in 0..50 {
        let b = rastgele(1)[0] as usize;
        harfler.push(ALFABE[b % ALFABE.len()] as char);
    }
    let _ = ham;
    harfler
        .as_bytes()
        .chunks(5)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("-")
}

/// Kullanıcının yazdığı kurtarma anahtarını normalleştir.
///
/// Tire, boşluk ve büyük/küçük harf farkı yok sayılıyor. Kâğıttan okuyup
/// yazan birinin araya tire koyup koymaması kurtarmayı engellememeli.
pub fn kurtarma_duzelt(girdi: &str) -> String {
    girdi
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Kurtarma anahtarından kurtarma kilit anahtarı.
pub fn kurtarma_kilit_anahtari(kurtarma: &str, tuz: &[u8]) -> Result<[u8; ANAHTAR_BOYU], String> {
    let temiz = kurtarma_duzelt(kurtarma);
    if temiz.len() < 40 {
        return Err("Kurtarma anahtarı eksik görünüyor.".into());
    }
    if tuz.len() < 16 {
        return Err("Tuz en az 16 bayt olmalı.".into());
    }
    let mut anahtar = [0u8; ANAHTAR_BOYU];
    Hkdf::<Sha256>::new(Some(tuz), temiz.as_bytes())
        .expand(b"bogahost-kasa-kurtarma-v1", &mut anahtar)
        .map_err(|_| "Kurtarma anahtarı türetilemedi.".to_string())?;
    Ok(anahtar)
}

pub fn aad_kurtarma(kullanici: i64) -> String {
    format!("v{SURUM}|kurtarma|u{kullanici}")
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
    fn kurtarma_bicimi_okunabilir() {
        let k = kurtarma_uret();
        assert_eq!(k.len(), 59);                       // 50 harf + 9 tire
        assert_eq!(k.matches('-').count(), 9);
        // Karıştırılabilir karakter olmamalı.
        for c in k.chars().filter(|c| *c != '-') {
            assert!(!"01ILOU".contains(c), "karistirilabilir karakter: {c}");
            assert!(c.is_ascii_uppercase() || c.is_ascii_digit());
        }
        // İki üretim aynı olmamalı.
        assert_ne!(kurtarma_uret(), kurtarma_uret());
    }

    #[test]
    fn kurtarma_tire_ve_kucuk_harf_onemsemez() {
        let tuz = rastgele(16);
        let k = kurtarma_uret();
        let a = kurtarma_kilit_anahtari(&k, &tuz).unwrap();
        let b = kurtarma_kilit_anahtari(&k.replace('-', ""), &tuz).unwrap();
        let c = kurtarma_kilit_anahtari(&k.to_lowercase(), &tuz).unwrap();
        let d = kurtarma_kilit_anahtari(&format!("  {}  ", k.replace('-', " ")), &tuz).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, d);
    }

    #[test]
    fn eksik_kurtarma_reddedilir() {
        assert!(kurtarma_kilit_anahtari("ABCDE-FGHJK", &rastgele(16)).is_err());
    }

    /// ANA PAROLA UNUTULDU: kurtarma anahtarı aynı özel anahtarı açmalı.
    #[test]
    fn kurtarma_ayni_ozel_anahtari_acar() {
        let u = 8i64;
        let (ozel, _acik) = anahtar_cifti();

        // Kurulum: özel anahtar İKİ AYRI yolla sarılıyor.
        let tuz = rastgele(16);
        let ka = kilit_anahtari("unutulacak-parola", &tuz, KdfParam::default()).unwrap();
        let normal_zarf = sifrele(&ka, &ozel, &aad_ozel_anahtar(u)).unwrap();

        let kurtarma = kurtarma_uret();
        let k_tuz = rastgele(16);
        let kka = kurtarma_kilit_anahtari(&kurtarma, &k_tuz).unwrap();
        let kurtarma_zarf = sifrele(&kka, &ozel, &aad_kurtarma(u)).unwrap();

        // Ana parola unutuldu; elde yalnız kâğıttaki kurtarma anahtarı var.
        let kka2 = kurtarma_kilit_anahtari(&kurtarma.to_lowercase(), &k_tuz).unwrap();
        let geri = coz(&kka2, &kurtarma_zarf, &aad_kurtarma(u)).unwrap();
        assert_eq!(geri, ozel.to_vec());

        // Kurtarma zarfı NORMAL bağlamda açılmamalı ve tersi.
        assert!(coz(&kka2, &normal_zarf, &aad_kurtarma(u)).is_err());
        assert!(coz(&kka2, &kurtarma_zarf, &aad_ozel_anahtar(u)).is_err());

        // Yanlış kurtarma anahtarı işe yaramamalı.
        let yanlis = kurtarma_kilit_anahtari(&kurtarma_uret(), &k_tuz).unwrap();
        assert!(coz(&yanlis, &kurtarma_zarf, &aad_kurtarma(u)).is_err());
    }

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
