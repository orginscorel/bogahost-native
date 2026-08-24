//! Odaktaki pencerenin kimliği ve hedef seçimi.
//!
//! Kasa, kimlik bilgisini yazmadan önce kullanıcıya HANGİ pencereye yazacağını
//! göstermek zorunda: parolanın yanlış yere gitmemesi için tek koruma budur.
//!
//! HEDEF İKİ YOLDAN BELİRLENİR:
//!   1. Küresel kısayol — basıldığı anda öndeki pencere yakalanır.
//!   2. Liste — kullanıcı açık programlar arasından kendisi seçer.
//! İkincisi şart: kısayol yolu, Otomasyon izni verilmemişse veya kullanıcı
//! uygulamayı doğrudan açtıysa boş dönüyor ve ekranda seçilecek hiçbir şey
//! kalmıyordu.

use serde::Serialize;

/// Hedef bir tarayıcı mı? Liste küçük ve sabit; yanlış pozitif, yanlış
/// negatiften iyidir — tarayıcıda kullanıcıyı eklentiye yönlendirmek
/// zararsız, masaüstü programında yönlendirmemek ise işi yarım bırakır.
fn tarayici_mi(program: &str) -> bool {
    let p = program.to_lowercase();
    ["chrome", "msedge", "edge", "firefox", "opera", "brave", "vivaldi", "safari", "chromium"]
        .iter()
        .any(|t| p.contains(t))
}

#[derive(Serialize, Clone, Default)]
pub struct Hedef {
    /// Pencere başlığı (macOS'ta uygulama adı)
    pub baslik: String,
    /// Programın adı (Windows: çalıştırılabilir adı, macOS: uygulama adı)
    pub program: String,
    /// Pencereyi yeniden öne getirmek için tutamak.
    /// Windows'ta HWND'nin ondalık gösterimi, macOS'ta uygulama adı.
    pub kimlik: String,
    /// Hedef bir tarayıcı mı? Tarayıcıda doğru doldurma yeri sayfanın
    /// içidir: eklenti parola alanının altında kendi menüsünü açar ve
    /// hangi alan olduğunu GÖREREK doldurur. Uygulama klavye simülasyonu
    /// yaptığı için sayfanın içini göremez.
    pub tarayici: bool,
    /// Tarayıcıysa açık sekmenin adresi. Kaydı otomatik eşleştirmek için.
    /// Windows'ta okunamaz (aşağıdaki nota bakın), bu yüzden Option.
    pub url: Option<String>,
}

// ── Windows ────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use super::{tarayici_mi, Hedef};
    use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, MAX_PATH, TRUE};
    use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
        IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    /// Pencerenin başlığı. Boşsa kullanıcıya gösterilecek bir şey yok demektir.
    fn baslik(hwnd: HWND) -> String {
        let mut tampon = [0u16; 512];
        let uzunluk = unsafe { GetWindowTextW(hwnd, &mut tampon) };
        if uzunluk <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&tampon[..uzunluk as usize])
    }

    /// Pencereyi açan programın çalıştırılabilir adı (".exe" atılmış).
    fn program(hwnd: HWND) -> (String, u32) {
        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 {
            return (String::new(), 0);
        }
        let mut ad = String::new();
        unsafe {
            if let Ok(kol) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) {
                let mut tampon = [0u16; MAX_PATH as usize];
                let n = GetModuleBaseNameW(kol, None, &mut tampon);
                if n > 0 {
                    ad = String::from_utf16_lossy(&tampon[..n as usize]);
                    // ".exe" gereksiz; kırpma büyük/küçük harfi BOZMAMALI.
                    if ad.to_lowercase().ends_with(".exe") {
                        ad.truncate(ad.len() - 4);
                    }
                }
                let _ = CloseHandle(kol);
            }
        }
        (ad, pid)
    }

    fn hedef_yap(hwnd: HWND) -> Hedef {
        let b = baslik(hwnd);
        let (p, _) = program(hwnd);
        Hedef {
            tarayici: tarayici_mi(&p),
            baslik: b,
            program: p,
            kimlik: (hwnd.0 as isize).to_string(),
            url: None,
        }
    }

    pub fn ondeki() -> Hedef {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return Hedef::default();
        }
        hedef_yap(hwnd)
    }

    unsafe extern "system" fn topla(hwnd: HWND, veri: LPARAM) -> BOOL {
        let liste = &mut *(veri.0 as *mut Vec<Hedef>);

        // Görünmeyen, başlıksız ve kendi penceremiz elenir.
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        let b = baslik(hwnd);
        if b.trim().is_empty() {
            return TRUE;
        }
        let (p, pid) = program(hwnd);
        if pid == GetCurrentProcessId() || p.is_empty() {
            return TRUE;
        }

        liste.push(Hedef {
            tarayici: tarayici_mi(&p),
            baslik: b,
            program: p,
            kimlik: (hwnd.0 as isize).to_string(),
            url: None,
        });
        TRUE
    }

    pub fn listele() -> Vec<Hedef> {
        let mut liste: Vec<Hedef> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(topla), LPARAM(&mut liste as *mut _ as isize));
        }
        liste
    }

    pub fn one_getir(kimlik: &str) -> bool {
        let Ok(ham) = kimlik.parse::<isize>() else { return false };
        let hwnd = HWND(ham as *mut core::ffi::c_void);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd).as_bool()
        }
    }
}

// ── macOS ──────────────────────────────────────────────────────────────────
//
// NSWorkspace'e Rust'tan bağlanmak objc köprüsü gerektiriyor; birkaç ad
// öğrenmek için bütün bir bağımlılık eklemek yerine sistemin kendi aracı
// kullanılıyor. Kullanıcı etkileşimi başına bir kez çalışır, maliyeti önemsiz.

#[cfg(target_os = "macos")]
mod mac {
    use super::{tarayici_mi, Hedef};

    fn osa(betik: &str) -> Option<String> {
        let c = std::process::Command::new("osascript").arg("-e").arg(betik).output().ok()?;
        if !c.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&c.stdout).trim().to_string())
    }

    fn hedef_yap(ad: String) -> Hedef {
        Hedef {
            tarayici: tarayici_mi(&ad),
            baslik: ad.clone(),
            program: ad.clone(),
            kimlik: ad,
            url: None,
        }
    }

    pub fn ondeki() -> Hedef {
        match osa("tell application \"System Events\" to get name of first application process whose frontmost is true") {
            Some(ad) if !ad.is_empty() => {
                let mut h = hedef_yap(ad.clone());
                h.url = aktif_url(&ad);
                h
            }
            _ => Hedef::default(),
        }
    }

    pub fn listele() -> Vec<Hedef> {
        // "background only is false" = arayüzü olan uygulamalar.
        let cikti = osa(
            "tell application \"System Events\" to get name of every application process whose background only is false",
        );
        let Some(c) = cikti else { return Vec::new() };
        c.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "Bogahost Kasa")
            .map(hedef_yap)
            .collect()
    }

    /// Öndeki tarayıcının açık sekmesindeki adres.
    ///
    /// NEDEN APPLESCRIPT: tarayıcılar adresi işletim sistemine pencere başlığı
    /// olarak vermez; başlıkta sayfa BAŞLIĞI yazar. Adresi almanın desteklenen
    /// yolu uygulamanın kendi betik arayüzüdür.
    ///
    /// DİKKAT: macOS her hedef uygulama için AYRI Otomasyon izni sorar
    /// ("Bogahost Kasa, Google Chrome'u kontrol etmek istiyor"). İzin
    /// verilmezse burada None döner ve otomatik eşleşme sessizce devre dışı
    /// kalır — doldurma yine elle seçimle çalışır.
    pub fn aktif_url(program: &str) -> Option<String> {
        let p = program.to_lowercase();
        let betik = if p.contains("safari") {
            format!("tell application \"{program}\" to get URL of front document")
        } else if p.contains("chrome") || p.contains("brave") || p.contains("edge")
            || p.contains("vivaldi") || p.contains("chromium") || p.contains("opera")
        {
            format!("tell application \"{program}\" to get URL of active tab of front window")
        } else {
            // Firefox'un betik arayüzü adres vermiyor; zorlamanın anlamı yok.
            return None;
        };
        let u = osa(&betik)?;
        if u.is_empty() || u == "missing value" { None } else { Some(u) }
    }

    pub fn one_getir(kimlik: &str) -> bool {
        // Tırnak kaçışı: uygulama adında tırnak olması beklenmez ama betiğe
        // ham geçirmek kod enjeksiyonu olurdu.
        let g = kimlik.replace('\\', "\\\\").replace('"', "\\\"");
        osa(&format!(
            "tell application \"System Events\" to set frontmost of application process \"{g}\" to true"
        ))
        .is_some()
    }
}

// ── Ortak yüzey ────────────────────────────────────────────────────────────

#[cfg(windows)]
pub fn ondeki() -> Hedef { win::ondeki() }
#[cfg(target_os = "macos")]
pub fn ondeki() -> Hedef { mac::ondeki() }
#[cfg(not(any(windows, target_os = "macos")))]
pub fn ondeki() -> Hedef { Hedef::default() }

#[cfg(windows)]
pub fn listele() -> Vec<Hedef> { win::listele() }
#[cfg(target_os = "macos")]
pub fn listele() -> Vec<Hedef> { mac::listele() }
#[cfg(not(any(windows, target_os = "macos")))]
pub fn listele() -> Vec<Hedef> { Vec::new() }

#[cfg(windows)]
pub fn one_getir(kimlik: &str) -> bool { win::one_getir(kimlik) }
#[cfg(target_os = "macos")]
pub fn one_getir(kimlik: &str) -> bool { mac::one_getir(kimlik) }
#[cfg(not(any(windows, target_os = "macos")))]
pub fn one_getir(_kimlik: &str) -> bool { false }

/// macOS Erişilebilirlik (Accessibility) izni — BİZİM sürecimiz için.
///
/// KÖK NEDEN, ÖNCEKİ HÂLİ NEDEN YANLIŞTI:
/// Burada `osascript` ile boş bir keystroke deneniyordu. Ama osascript AYRI BİR
/// SÜREÇTİR: o test, System Events'in izni olup olmadığını ölçer — Bogahost
/// Kasa'nınkini değil. System Events'e izin çoğu makinede zaten verilmiş
/// olduğundan kontrol "izin var" diyordu, oysa bizim sürecimizin izni yoktu.
/// enigo'nun CGEvent çağrıları sessizce yutuluyor ve HATA DÖNDÜRMÜYOR; sonuç:
/// uygulama "Dolduruldu" diyor, hedefe tek karakter yazılmıyor.
///
/// AXIsProcessTrusted ÇAĞIRAN SÜRECİ sorar; ölçmek istediğimiz tam olarak bu.
#[cfg(target_os = "macos")]
pub fn erisilebilirlik_izni_var() -> bool {
    // ApplicationServices/HIServices. Boolean = unsigned char, bu yüzden u8.
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
    }
    unsafe { AXIsProcessTrusted() != 0 }
}

/// Erişilebilirlik ayarlarını doğrudan aç — kullanıcıyı menülerde dolaştırma.
#[cfg(target_os = "macos")]
pub fn erisilebilirlik_ayarlarini_ac() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn erisilebilirlik_ayarlarini_ac() {}
#[cfg(not(target_os = "macos"))]
pub fn erisilebilirlik_izni_var() -> bool {
    true
}

/// Otomasyon (Apple Events) izni — pencere listesini okuyabiliyor muyuz?
/// Liste boş dönüyorsa kullanıcıya "hiç pencere yok" değil, izin eksik demeliyiz.
#[cfg(target_os = "macos")]
pub fn otomasyon_izni_var() -> bool {
    std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of first application process")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn otomasyon_izni_var() -> bool {
    true
}
