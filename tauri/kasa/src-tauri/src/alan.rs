//! Odaktaki ALANIN ne olduğunu tespit eder.
//!
//! NEDEN GEREKLİ: doldurma klavye simülasyonuyla yapılıyor ve klavye, odakta ne
//! varsa oraya yazar. Uygulamanın "burası kullanıcı adı alanı mı" diye bir
//! kavramı yoktu; adresi eşleşen bir sayfada odak arama kutusundaysa parola
//! arama kutusuna yazılıyordu. Üstelik alanda zaten veri varsa üstüne yazıyordu.
//!
//! macOS'ta Erişilebilirlik API'si (AXUIElement) odaktaki öğenin ROLÜNÜ ve
//! DEĞERİNİ verir. Zaten Erişilebilirlik izni almış durumdayız — yazmak için
//! şart olan izin, okumak için de yeterli.
//!
//! Windows'ta karşılığı UI Automation'dır ve tek bir çağrıyla alınamıyor;
//! orada tespit şimdilik yapılmıyor (`Uygun::Bilinmiyor` döner) ve davranış
//! eskisi gibi kalır.

/// Odaktaki alanın doldurmaya uygunluğu.
#[derive(PartialEq, Debug)]
pub enum Uygun {
    /// Metin/parola alanı ve BOŞ — güvenle yazılabilir.
    Bos,
    /// Metin/parola alanı ama içinde veri var — üstüne yazmak veri kaybıdır.
    Dolu,
    /// Metin alanı değil (düğme, bağlantı, sayfa gövdesi, liste…).
    AlanDegil(String),
    /// Tespit edilemedi (izin yok, platform desteklemiyor) — engelleme.
    Bilinmiyor,
}

#[cfg(target_os = "macos")]
pub fn odakli_alan() -> Uygun {
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
            return Uygun::Bilinmiyor;
        }

        let anahtar = CFString::new("AXFocusedUIElement");
        let mut odak: CFTypeRef = std::ptr::null();
        let hata = AXUIElementCopyAttributeValue(sistem, anahtar.as_concrete_TypeRef(), &mut odak);
        CFRelease(sistem);
        if hata != 0 || odak.is_null() {
            return Uygun::Bilinmiyor;
        }

        // Bir metin özniteliğini güvenle okur.
        //
        // TİP KONTROLÜ ŞART: AXValue her zaman metin değildir (kaydırma çubuğunda
        // sayı, onay kutusunda mantıksal değer döner). Tip bakmadan CFString
        // sanmak tanımsız davranıştır.
        let metin_oku = |ad: &str| -> Option<String> {
            let k = CFString::new(ad);
            let mut v: CFTypeRef = std::ptr::null();
            if AXUIElementCopyAttributeValue(odak, k.as_concrete_TypeRef(), &mut v) != 0 || v.is_null()
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

        // Yazılabilir roller. AXComboBox ve AXTextArea de metin kabul eder.
        let yazilabilir = matches!(
            rol.as_str(),
            "AXTextField" | "AXSecureTextField" | "AXTextArea" | "AXComboBox"
        );
        if !yazilabilir {
            return Uygun::AlanDegil(if rol.is_empty() { "bilinmeyen".into() } else { rol });
        }

        // AXSecureTextField (parola alanı) değerini vermez — None döner.
        // Boş sayıp devam etmek doğru: parola alanına yazmak zaten amacımız.
        match deger {
            Some(d) if !d.trim().is_empty() => Uygun::Dolu,
            _ => Uygun::Bos,
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn odakli_alan() -> Uygun {
    Uygun::Bilinmiyor
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
