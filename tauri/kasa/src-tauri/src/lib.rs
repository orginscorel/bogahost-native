//! Bogahost Kasa — masaüstü programlarına güvenli kimlik doldurma.
//!
//! NEDEN DİĞER UYGULAMALARDAN FARKLI: finans/dcim/chat/task/muh kabukları canlı
//! bir URL yükleyen WebView'lerdir. Kasa bunu yapamaz; işini görebilmesi için
//! işletim sistemi yeteneklerine ihtiyacı var — küresel kısayol, öndeki
//! pencereyi okuma ve BAŞKA bir programa tuş gönderme. Bu yüzden arayüz
//! yereldir (uzak URL değil) ve iş mantığı Rust tarafındadır.
//!
//! TARAYICI DAHİL HER PENCEREYE YAZAR. Klavye girdisi olarak gönderildiği için
//! hedefin ne olduğu fark etmez; ayrı bir tarayıcı istisnası yoktur.
//!
//! KOMUTLAR NEDEN `#[tauri::command(async)]`:
//! Tauri'de `async` İŞARETİ OLMAYAN komutlar ANA İŞ PARÇACIĞINDA çalışır.
//! Buradaki komutların çoğu ağa çıkıyor (20 sn zaman aşımı) ve `doldur` ayrıca
//! uyuyor; ana iş parçacığında çalışsalardı pencere o süre boyunca donardı.
//! `(async)` işareti gövdeyi ayrı bir görevde çalıştırır.
//!
//! DURUM NEDEN `AppHandle` ÜZERİNDEN OKUNUYOR:
//! `State<'_, T>` ödünç alınmış bir argümandır; komut ayrı bir göreve taşınınca
//! ömrü karışır. `AppHandle` 'static ve klonlanabilir olduğu için durum gövde
//! İÇİNDE `uygulama.state::<Durum>()` ile alınır — ödünç, gövdeden dışarı çıkmaz.

mod kasa;
mod pencere;

use kasa::{Durum, GirisSonuc, Kayit};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Kısayola basıldığı ANDAKİ hedef. Kullanıcı listeden seçim yaparken odak
/// bizim penceremize geçer; yazarken kullanılacak pencere bu değil, kısayol
/// anında yakalanan penceredir.
#[derive(Default)]
struct HedefDurum(Mutex<pencere::Hedef>);

#[derive(Serialize)]
struct DoldurSonuc {
    tamam: bool,
    mesaj: String,
}

// ── Komutlar ───────────────────────────────────────────────────────────────

#[tauri::command(async)]
fn giris_yap(uygulama: tauri::AppHandle, kullanici: String, sifre: String) -> GirisSonuc {
    let cihaz = format!(
        "{} — {}",
        bilgisayar_adi(),
        if cfg!(windows) { "Windows" } else { "macOS" }
    );
    kasa::giris(&uygulama.state::<Durum>(), &kullanici, &sifre, &cihaz)
}

#[tauri::command(async)]
fn kod_dogrula(uygulama: tauri::AppHandle, kod: String) -> GirisSonuc {
    kasa::dogrula(&uygulama.state::<Durum>(), &kod)
}

#[tauri::command(async)]
fn oturum_var(uygulama: tauri::AppHandle) -> bool {
    kasa::oturum_gecerli(&uygulama.state::<Durum>())
}

#[tauri::command(async)]
fn oturumu_kapat(uygulama: tauri::AppHandle) {
    kasa::jeton_sil();
    *uygulama.state::<Durum>().jeton.lock().unwrap() = None;
}

#[tauri::command(async)]
fn kayitlar(uygulama: tauri::AppHandle) -> Result<Vec<Kayit>, String> {
    kasa::liste(&uygulama.state::<Durum>())
}

#[tauri::command(async)]
fn hedef(uygulama: tauri::AppHandle) -> pencere::Hedef {
    uygulama.state::<HedefDurum>().0.lock().unwrap().clone()
}

#[derive(Serialize)]
struct Izinler {
    /// Pencere adlarını okumak / pencereyi öne getirmek (macOS: Apple Events)
    otomasyon: bool,
    /// Başka programa tuş göndermek (macOS: Accessibility)
    erisilebilirlik: bool,
}

/// İki izin AYRI AYRI sorulur. Biri verilip diğeri unutulduğunda ekranda
/// "hiç pencere yok" gibi yanıltıcı bir durum oluşuyordu; artık hangisinin
/// eksik olduğunu söyleyebiliyoruz.
#[tauri::command(async)]
fn izinler() -> Izinler {
    Izinler {
        otomasyon: pencere::otomasyon_izni_var(),
        erisilebilirlik: pencere::erisilebilirlik_izni_var(),
    }
}

/// Açık programların listesi — kullanıcı hedefi buradan seçebilsin diye.
///
/// NEDEN GEREKLİ: hedef yalnızca küresel kısayolla yakalanabiliyordu. Kullanıcı
/// uygulamayı doğrudan açtığında ya da Otomasyon izni verilmemişken ekranda
/// "Hedef pencere seçilmedi" yazıyor ve seçilecek hiçbir şey bulunmuyordu.
#[tauri::command(async)]
fn pencereler() -> Vec<pencere::Hedef> {
    pencere::listele()
}

/// Listeden seçilen pencereyi hedef yapar.
#[tauri::command(async)]
fn hedef_sec(uygulama: tauri::AppHandle, kimlik: String) -> Option<pencere::Hedef> {
    let secilen = pencere::listele().into_iter().find(|h| h.kimlik == kimlik)?;
    *uygulama.state::<HedefDurum>().0.lock().unwrap() = secilen.clone();
    Some(secilen)
}

/// Kullanıcı adını ve parolayı hedef pencereye YAZAR.
///
/// Parola buradan dışarı çıkmaz: sunucudan alınır, klavyeye yazılır, işlev
/// biterken bellekten düşer. Arayüz parolayı hiç görmez.
#[tauri::command(async)]
fn doldur(uygulama: tauri::AppHandle, id: i64, enter_bas: bool) -> DoldurSonuc {
    if !pencere::erisilebilirlik_izni_var() {
        return DoldurSonuc {
            tamam: false,
            mesaj: "macOS Erişilebilirlik izni gerekiyor: Sistem Ayarları → Gizlilik ve Güvenlik → Erişilebilirlik → Bogahost Kasa".into(),
        };
    }

    let h = uygulama.state::<HedefDurum>().0.lock().unwrap().clone();
    if h.program.is_empty() {
        return DoldurSonuc {
            tamam: false,
            mesaj: "Hedef pencere seçilmedi. Listeden bir program seçin ya da doldurulacak pencere öndeyken kısayola basın.".into(),
        };
    }

    let (kullanici, sifre) = match kasa::ac(&uygulama.state::<Durum>(), id) {
        Ok(v) => v,
        Err(e) => return DoldurSonuc { tamam: false, mesaj: format!("Alınamadı: {e}") },
    };

    // SIRA ÖNEMLİ: ÖNCE hedefi öne getir, SONRA kendi penceremizi gizle.
    // Windows'ta SetForegroundWindow yalnızca çağıran süreç ÖN PLANDAYKEN
    // çalışır; kendimizi önce gizlersek o hakkı kaybeder ve tuşlar yanlış
    // pencereye gider. Gizleme ikinci adımda, hedef zaten odaktayken yapılır.
    if !h.kimlik.is_empty() {
        pencere::one_getir(&h.kimlik);
        std::thread::sleep(std::time::Duration::from_millis(240));
    }

    let pencere_tauri = uygulama.get_webview_window("main");
    if let Some(p) = &pencere_tauri {
        let _ = p.hide();
    }
    std::thread::sleep(std::time::Duration::from_millis(140));

    let sonuc = yaz(&kullanici, &sifre, enter_bas);

    // Parola bellekten düşsün (Rust burada zaten bırakır; niyet açık olsun diye)
    drop(sifre);

    if let Some(p) = &pencere_tauri {
        let _ = p.show();
    }

    match sonuc {
        Ok(()) => DoldurSonuc { tamam: true, mesaj: "Dolduruldu.".into() },
        Err(e) => DoldurSonuc {
            tamam: false,
            mesaj: format!("Yazılamadı: {e}. Kopyala düğmelerini kullanabilirsiniz."),
        },
    }
}

/// Yalnızca kullanıcı adını döndürür — parola için ayrı komut yok, bilerek.
#[tauri::command(async)]
fn kullanici_adi(uygulama: tauri::AppHandle, id: i64) -> Result<String, String> {
    kasa::ac(&uygulama.state::<Durum>(), id).map(|(k, _)| k)
}

/// Parolayı panoya koyar ve süre sonunda temizler.
/// Parola yine arayüze verilmez; pano işlemi Rust tarafında yapılır.
#[tauri::command(async)]
fn panoya_sifre(uygulama: tauri::AppHandle, id: i64) -> Result<u64, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    const TEMIZLEME_SN: u64 = 30;

    let (_, sifre) = kasa::ac(&uygulama.state::<Durum>(), id)?;
    uygulama.clipboard().write_text(sifre.clone()).map_err(|e| e.to_string())?;

    // Süre sonunda temizle — ama araya başka bir şey kopyalandıysa DOKUNMA
    let u = uygulama.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(TEMIZLEME_SN));
        if let Ok(mevcut) = u.clipboard().read_text() {
            if mevcut == sifre {
                let _ = u.clipboard().write_text(String::new());
            }
        }
        let _ = u.emit("pano-temizlendi", ());
    });

    Ok(TEMIZLEME_SN)
}

// ── Tuş gönderme ───────────────────────────────────────────────────────────

/// Metni yazar: kullanıcı adı → Tab → parola → (isteğe bağlı) Enter.
///
/// Karakterler doğrudan gönderilir (sanal tuş kodu değil), böylece Türkçe Q/F
/// veya başka bir klavye düzeni parolayı bozmaz.
fn yaz(kullanici: &str, sifre: &str, enter_bas: bool) -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut e = Enigo::new(&Settings::default()).map_err(|x| x.to_string())?;

    if !kullanici.is_empty() {
        e.text(kullanici).map_err(|x| x.to_string())?;
        e.key(Key::Tab, Direction::Click).map_err(|x| x.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(70));
    }
    if !sifre.is_empty() {
        e.text(sifre).map_err(|x| x.to_string())?;
    }
    if enter_bas {
        std::thread::sleep(std::time::Duration::from_millis(70));
        e.key(Key::Return, Direction::Click).map_err(|x| x.to_string())?;
    }
    Ok(())
}

/// Cihaz adı — kasa oturum kaydında "hangi bilgisayar" olarak görünür.
///
/// macOS'ta GUI uygulamalarına HOSTNAME değişkeni GEÇMEZ; bu yüzden ortam
/// değişkeni boşsa `hostname` komutuna düşülür.
fn bilgisayar_adi() -> String {
    if let Ok(a) = std::env::var("COMPUTERNAME") {
        if !a.trim().is_empty() {
            return a;
        }
    }
    if let Ok(a) = std::env::var("HOSTNAME") {
        if !a.trim().is_empty() {
            return a;
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(c) = std::process::Command::new("/bin/hostname").arg("-s").output() {
            let a = String::from_utf8_lossy(&c.stdout).trim().to_string();
            if !a.is_empty() {
                return a;
            }
        }
    }
    "Bilgisayar".into()
}

// ── Uygulama ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Durum::default())
        .manage(HedefDurum::default())
        .invoke_handler(tauri::generate_handler![
            giris_yap, kod_dogrula, oturum_var, oturumu_kapat,
            kayitlar, hedef, izinler, pencereler, hedef_sec,
            doldur, kullanici_adi, panoya_sifre
        ])
        .setup(|uygulama| {
            // Kayıtlı oturumu belleğe al (geçerliliği arayüz açılışında sorulur)
            if let Some(t) = kasa::jeton_oku() {
                let durum = uygulama.state::<Durum>();
                *durum.jeton.lock().unwrap() = Some(t);
            }

            kisayol_kur(uygulama.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Bogahost Kasa baslatilamadi");
}

/// Küresel kısayol: Windows'ta Ctrl+Alt+K, macOS'ta Cmd+Alt+K.
///
/// Basıldığı anda ÖNCE hedef pencere yakalanır, SONRA kendi penceremiz açılır.
/// Sıra önemli: kendimizi önce gösterirsek öndeki pencere biz oluruz.
fn kisayol_kur(uygulama: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let degistirici = if cfg!(target_os = "macos") {
        Modifiers::SUPER | Modifiers::ALT
    } else {
        Modifiers::CONTROL | Modifiers::ALT
    };
    let kisayol = Shortcut::new(Some(degistirici), Code::KeyK);

    let u = uygulama.clone();
    uygulama.global_shortcut().on_shortcut(kisayol, move |_app, _sc, olay| {
        if olay.state() != ShortcutState::Pressed {
            return;
        }
        let h = pencere::ondeki();
        {
            let d = u.state::<HedefDurum>();
            *d.0.lock().unwrap() = h.clone();
        }
        if let Some(p) = u.get_webview_window("main") {
            let _ = p.show();
            let _ = p.unminimize();
            let _ = p.set_focus();
        }
        let _ = u.emit("hedef-degisti", h);
    })?;

    Ok(())
}
