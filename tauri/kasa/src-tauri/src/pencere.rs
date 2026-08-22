//! Odaktaki pencerenin kimliği.
//!
//! Kasa, kimlik bilgisini yazmadan önce kullanıcıya HANGİ pencereye yazacağını
//! göstermek zorunda: parolanın yanlış yere gitmemesi için tek koruma budur.
//! Pencere/uygulama adını almak platforma özeldir, bu yüzden burada toplandı.

use serde::Serialize;

#[derive(Serialize, Clone, Default)]
pub struct Hedef {
    /// Pencere başlığı (macOS'ta uygulama adı)
    pub baslik: String,
    /// Program adı — tarayıcı olup olmadığını anlamak için
    pub program: String,
    /// Tarayıcıysa kullanıcı uyarılır: orada doldurmayı Chrome eklentisi yapar
    pub tarayici: bool,
}

/// Tarayıcı mı? Liste küçük ve sabit tutuldu; yanlış pozitif, yanlış negatiften iyidir.
fn tarayici_mi(program: &str) -> bool {
    let p = program.to_lowercase();
    ["chrome", "msedge", "edge", "firefox", "opera", "brave", "vivaldi", "safari", "iexplore"]
        .iter()
        .any(|t| p.contains(t))
}

#[cfg(windows)]
pub fn ondeki() -> Hedef {
    use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH};
    use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Hedef::default();
        }

        let mut tampon = [0u16; 512];
        let uzunluk = GetWindowTextW(hwnd, &mut tampon);
        let baslik = String::from_utf16_lossy(&tampon[..uzunluk.max(0) as usize]);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let mut program = String::new();
        if pid != 0 {
            if let Ok(kol) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) {
                let mut ad = [0u16; MAX_PATH as usize];
                let n = GetModuleBaseNameW(kol, None, &mut ad);
                if n > 0 {
                    program = String::from_utf16_lossy(&ad[..n as usize]);
                    // ".exe" uzantısı kullanıcıya gösterilirken gereksiz.
                    // Kırpma büyük/küçük harfi BOZMAMALI: "Chrome.exe" → "Chrome".
                    if program.to_lowercase().ends_with(".exe") {
                        program.truncate(program.len() - 4);
                    }
                }
                let _ = CloseHandle(kol);
            }
        }

        let tarayici = tarayici_mi(&program);
        Hedef { baslik, program, tarayici }
    }
}

#[cfg(target_os = "macos")]
pub fn ondeki() -> Hedef {
    // NSWorkspace'e Rust'tan bağlanmak objc köprüsü gerektiriyor; tek bir ad
    // öğrenmek için bütün bir bağımlılık eklemek yerine sistemin kendi aracı
    // kullanılır. Kısayola basıldığında bir kez çalışır, maliyeti önemsizdir.
    let cikti = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
        .output();

    let program = match cikti {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    };

    let tarayici = tarayici_mi(&program);
    Hedef { baslik: program.clone(), program, tarayici }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn ondeki() -> Hedef {
    Hedef::default()
}

/// macOS'ta tuş göndermek "Erişilebilirlik" (Accessibility) izni ister; izin
/// yoksa yazma SESSİZCE başarısız olur. Kullanıcıya "çalışmıyor" dedirtmemek
/// için doldurmadan önce sorulur.
///
/// DİKKAT — İKİ AYRI İZİN VAR, KARIŞTIRILMAMALI:
///   • Automation (Apple Events) → öndeki pencerenin adını okumak için,
///   • Accessibility            → başka programa tuş göndermek için.
/// Birincisi verilmiş olsa bile ikincisi verilmemiş olabilir. Bu yüzden burada
/// pencere adı sorulmaz; BOŞ bir keystroke denenir — hiçbir karakter yazmaz,
/// ama izin yoksa hata döner. Gerçekte kullanılacak yeteneği ölçen tek yol bu.
#[cfg(target_os = "macos")]
pub fn erisilebilirlik_izni_var() -> bool {
    std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"\"")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn erisilebilirlik_izni_var() -> bool {
    true
}
