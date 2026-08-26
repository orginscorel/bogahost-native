//! Odaktaki ALANIN ne olduğunu tespit eder.
//!
//! NEDEN GEREKLİ: doldurma klavye simülasyonuyla yapılıyor ve klavye, odakta ne
//! varsa oraya yazar. Uygulamanın "burası kullanıcı adı alanı mı" diye bir
//! kavramı yoktu; adresi eşleşen bir sayfada odak arama kutusundaysa parola
//! arama kutusuna yazılıyordu.
//!
//! macOS'ta Erişilebilirlik API'si (AXUIElement) odaktaki öğenin ROLÜNÜ ve
//! DEĞERİNİ verir. Yazmak için zaten Erişilebilirlik izni alınmış durumdayız;
//! okumak için ek izin gerekmiyor.
//!
//! Windows'ta karşılığı UI Automation'dır ve tek çağrıyla alınamıyor; orada
//! tespit yapılmıyor (`Bilinmiyor` döner) ve davranış eskisi gibi kalır.
//! Engellemek, çalışan bir akışı hiç çalıştırmamaktan iyi değil.

/// Odaktaki alanın doldurmaya uygunluğu.
#[derive(PartialEq, Debug)]
pub enum Uygun {
    /// Metin/parola alanı ve BOŞ — güvenle yazılabilir.
    Bos,
    /// Metin/parola alanı ama içinde veri var — üstüne yazmak veri kaybıdır.
    Dolu,
    /// Metin alanı değil (düğme, bağlantı, sayfa gövdesi, liste…).
    AlanDegil(String),
    /// Tespit edilemedi (izin yok, platform desteklemiyor).
    Bilinmiyor,
}

/// Parola alanının macOS'taki rolü. Giriş formunun en güvenilir işareti budur:
/// arama kutusu, adres çubuğu, not alanı asla bu rolü taşımaz.
pub const PAROLA_ROLU: &str = "AXSecureTextField";

/// Metin kabul eden roller. AXComboBox ve AXTextArea da yazılabilir.
const YAZILABILIR: [&str; 4] = ["AXTextField", PAROLA_ROLU, "AXTextArea", "AXComboBox"];

// ── macOS ──────────────────────────────────────────────────────────────────

/// Odaktaki öğenin (rol, değer) çifti — bütün okumalar buradan geçer.
#[cfg(target_os = "macos")]
fn odak_oku() -> Option<(String, Option<String>)> {
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFStringGetTypeID() -> usize;
    }

    unsafe {
        let sistem = AXUIElementCreateSystemWide();
        if sistem.is_null() {
            return None;
        }

        let anahtar = CFString::new("AXFocusedUIElement");
        let mut odak: CFTypeRef = std::ptr::null();
        let hata = AXUIElementCopyAttributeValue(sistem, anahtar.as_concrete_TypeRef(), &mut odak);
        CFRelease(sistem);
        if hata != 0 || odak.is_null() {
            return None;
        }

        // TİP KONTROLÜ ŞART: AXValue her zaman metin değildir (kaydırma
        // çubuğunda sayı, onay kutusunda mantıksal değer döner). Tip bakmadan
        // CFString saymak tanımsız davranıştır.
        let metin_oku = |ad: &str| -> Option<String> {
            let k = CFString::new(ad);
            let mut v: CFTypeRef = std::ptr::null();
            if AXUIElementCopyAttributeValue(odak, k.as_concrete_TypeRef(), &mut v) != 0
                || v.is_null()
            {
                return None;
            }
            if CFGetTypeID(v) != CFStringGetTypeID() {
                CFRelease(v);
                return None;
            }
            Some(CFString::wrap_under_create_rule(v as CFStringRef).to_string())
        };

        let rol = metin_oku("AXRole").unwrap_or_default();
        let deger = metin_oku("AXValue");
        CFRelease(odak);
        Some((rol, deger))
    }
}

#[cfg(not(target_os = "macos"))]
fn odak_oku() -> Option<(String, Option<String>)> {
    None
}

// ── Ortak yüzey ────────────────────────────────────────────────────────────

/// Odaktaki öğenin rolü.
pub fn odakli_rol() -> Option<String> {
    odak_oku().map(|(rol, _)| rol)
}

/// Odakta bir PAROLA alanı var mı?
pub fn parola_alani_mi() -> bool {
    odakli_rol().map(|r| r == PAROLA_ROLU).unwrap_or(false)
}

/// Odaktaki alan doldurmaya uygun mu?
pub fn odakli_alan() -> Uygun {
    let Some((rol, deger)) = odak_oku() else {
        return Uygun::Bilinmiyor;
    };

    if !YAZILABILIR.contains(&rol.as_str()) {
        return Uygun::AlanDegil(if rol.is_empty() { "bilinmeyen".into() } else { rol });
    }

    // Parola alanı değerini VERMEZ (None döner) — boş saymak doğru, oraya
    // yazmak zaten amacımız.
    match deger {
        Some(d) if !d.trim().is_empty() => Uygun::Dolu,
        _ => Uygun::Bos,
    }
}

/// Kullanıcıya gösterilecek açıklama.
pub fn aciklama(u: &Uygun) -> String {
    match u {
        Uygun::Bos => String::new(),
        Uygun::Dolu => {
            "Odaktaki alanda zaten veri var — üstüne yazmadım. Alanı temizleyip tekrar deneyin."
                .into()
        }
        Uygun::AlanDegil(rol) => format!(
            "Odakta bir metin alanı yok ({rol}). Doldurulacak kullanıcı adı alanına tıklayıp tekrar deneyin."
        ),
        Uygun::Bilinmiyor => String::new(),
    }
}

// ── Odaktaki alanın ekrandaki yeri ─────────────────────────────────────────

/// Odaktaki metin alanının ekran dikdörtgeni: (x, y, genişlik, yükseklik),
/// mantıksal nokta cinsinden, sol-üst köşe başlangıçlı.
///
/// NEDEN: panel ekranın sağ altında açılıyordu. Kullanıcının beğendiği davranış
/// tarayıcı eklentisininki — menü, yazacağı alanın hemen ALTINDA çıkıyor.
/// Masaüstü programında da aynısını yapabilmek için alanın nerede olduğunu
/// bilmek gerekiyor.
///
/// Erişilebilirlik API'si zaten açık (yazmak için alınmış izin); ek izin yok.
#[cfg(target_os = "macos")]
pub fn odak_konumu() -> Option<(f64, f64, f64, f64)> {
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};
    use std::os::raw::c_void;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct CGPoint { x: f64, y: f64 }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct CGSize { width: f64, height: f64 }

    // AXValueType: 1 = CGPoint, 2 = CGSize
    const AX_POINT: u32 = 1;
    const AX_SIZE: u32 = 2;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn AXValueGetValue(value: CFTypeRef, tur: u32, hedef: *mut c_void) -> bool;
        fn AXValueGetTypeID() -> usize;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
    }

    unsafe {
        let sistem = AXUIElementCreateSystemWide();
        if sistem.is_null() {
            return None;
        }
        let anahtar = CFString::new("AXFocusedUIElement");
        let mut odak: CFTypeRef = std::ptr::null();
        let hata = AXUIElementCopyAttributeValue(sistem, anahtar.as_concrete_TypeRef(), &mut odak);
        CFRelease(sistem);
        if hata != 0 || odak.is_null() {
            return None;
        }

        // TİP KONTROLÜ ŞART: AXPosition her öğede AXValue olarak gelmez.
        // Tip bakmadan AXValueGetValue çağırmak tanımsız davranıştır.
        let mut oku = |ad: &str, tur: u32, hedef: *mut c_void| -> bool {
            let k = CFString::new(ad);
            let mut v: CFTypeRef = std::ptr::null();
            if AXUIElementCopyAttributeValue(odak, k.as_concrete_TypeRef(), &mut v) != 0
                || v.is_null()
            {
                return false;
            }
            if CFGetTypeID(v) != AXValueGetTypeID() {
                CFRelease(v);
                return false;
            }
            let tamam = AXValueGetValue(v, tur, hedef);
            CFRelease(v);
            tamam
        };

        let mut nokta = CGPoint::default();
        let mut boyut = CGSize::default();
        let p_ok = oku("AXPosition", AX_POINT, &mut nokta as *mut _ as *mut c_void);
        let b_ok = oku("AXSize", AX_SIZE, &mut boyut as *mut _ as *mut c_void);
        CFRelease(odak);

        if !p_ok || !b_ok || boyut.width <= 1.0 || boyut.height <= 1.0 {
            return None;
        }
        Some((nokta.x, nokta.y, boyut.width, boyut.height))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn odak_konumu() -> Option<(f64, f64, f64, f64)> {
    None
}
