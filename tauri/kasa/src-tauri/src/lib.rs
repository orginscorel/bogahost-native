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

mod guncelleme;
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

    // Doldurma /fill ucundan geçer: gizli kayıtta görüntüleme kapalı olsa
    // bile yazma çalışır ve her yazım denetime düşer.
    let (kullanici, sifre) = match kasa::doldurmak_icin_ac(&uygulama.state::<Durum>(), id) {
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

// ── Otomatik güncelleme ────────────────────────────────────────────────────

/// "Güncellemeleri denetle" — kullanıcı istediğinde. Arka plan turu sessizdir;
/// burada "güncel" sonucu da arayüze bildirilir.
#[tauri::command]
async fn guncelleme_ara(uygulama: tauri::AppHandle) -> guncelleme::Durum {
    guncelleme::denetle(uygulama, true).await
}

/// İndirilmiş sürümü uygula. Başarılıysa uygulama yeniden başlar.
#[tauri::command(async)]
fn guncelleme_uygula(uygulama: tauri::AppHandle) -> Result<(), String> {
    guncelleme::uygula(&uygulama)
}

/// macOS Erişilebilirlik ayarlarını aç — kullanıcı menülerde dolaşmasın.
#[tauri::command(async)]
fn izin_ayarlarini_ac() {
    pencere::erisilebilirlik_ayarlarini_ac();
}

/// Adrese göre eşleşen kayıtlar — arayüz bunu otomatik seçim için kullanır.
#[tauri::command(async)]
fn eslesenler(uygulama: tauri::AppHandle, url: String) -> Result<Vec<Kayit>, String> {
    kasa::eslesenler(&uygulama.state::<Durum>(), &url)
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
            kayitlar, hedef, izinler, pencereler, hedef_sec, eslesenler,
            doldur, kullanici_adi, panoya_sifre,
            guncelleme_ara, guncelleme_uygula, izin_ayarlarini_ac
        ])
        .setup(|uygulama| {
            // Kayıtlı oturumu belleğe al (geçerliliği arayüz açılışında sorulur)
            if let Some(t) = kasa::jeton_oku() {
                let durum = uygulama.state::<Durum>();
                *durum.jeton.lock().unwrap() = Some(t);
            }

            kisayol_kur(uygulama.handle())?;
            hedef_izle(uygulama.handle());
            guncelleme::zamanla(uygulama.handle());
            izin_izle(uygulama.handle());
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
        // Hedef, kısayola basıldığı ANDA yakalanır — kendi penceremizi hiç
        // göstermeden. Gösterseydik öndeki pencere biz olurduk.
        let h = pencere::ondeki();
        {
            let d = u.state::<HedefDurum>();
            *d.0.lock().unwrap() = h.clone();
        }

        // Ağ ve klavye işi olay işleyicisinde yapılmaz: bu işleyici ana olay
        // döngüsünde çalışıyor, orada beklemek arayüzü dondurur.
        let u2 = u.clone();
        std::thread::spawn(move || kisayol_isle(u2, h));
    })?;

    Ok(())
}

/// Kısayolun asıl işi: MÜMKÜNSE HİÇ PENCERE AÇMADAN DOLDUR.
///
/// Kullanıcıdan gelen geri bildirim buydu: "doldur demek için uygulamaya girmek
/// gerekiyor". Artık akış şu:
///   • Hedefte bir adres var ve tek/net bir eşleşme bulunduysa → doğrudan yaz.
///   • Eşleşme yok, birden fazla aday var ya da oturum yoksa → pencereyi aç,
///     kullanıcı seçsin. Yanlış kayda parola yazmaktansa sormak doğru.
fn kisayol_isle(uygulama: tauri::AppHandle, h: pencere::Hedef) {
    let pencereyi_ac = |neden: &str| {
        if let Some(p) = uygulama.get_webview_window("main") {
            let _ = p.show();
            let _ = p.unminimize();
            let _ = p.set_focus();
        }
        let _ = uygulama.emit("hedef-degisti", h.clone());
        if !neden.is_empty() {
            let _ = uygulama.emit("kisayol-notu", neden.to_string());
        }
    };

    // Erişilebilirlik izni yoksa yazmak SESSİZCE başarısız olur; kullanıcıyı
    // "dolduruldu" diye kandırmak yerine pencereyi açıp yolu gösteriyoruz.
    if !pencere::erisilebilirlik_izni_var() {
        pencereyi_ac("izin-yok");
        return;
    }

    let Some(url) = h.url.clone() else {
        pencereyi_ac("");
        return;
    };

    let durum = uygulama.state::<Durum>();
    let eslesenler = match kasa::eslesenler(&durum, &url) {
        Ok(v) => v,
        Err(_) => {
            pencereyi_ac("");
            return;
        }
    };

    // Tek aday yoksa karar kullanıcınındır.
    if eslesenler.len() != 1 {
        pencereyi_ac(if eslesenler.is_empty() { "eslesme-yok" } else { "coklu-eslesme" });
        return;
    }

    let kayit = &eslesenler[0];
    let (kullanici, sifre) = match kasa::doldurmak_icin_ac(&durum, kayit.id) {
        Ok(v) => v,
        Err(_) => {
            pencereyi_ac("");
            return;
        }
    };

    // Hedef zaten önde (pencere açmadık), yine de emin olalım.
    if !h.kimlik.is_empty() {
        pencere::one_getir(&h.kimlik);
        std::thread::sleep(std::time::Duration::from_millis(120));
    }

    let sonuc = yaz(&kullanici, &sifre, false);
    drop(sifre);

    match sonuc {
        Ok(()) => {
            let _ = uygulama.emit("kisayol-dolduruldu", kayit.label.clone());
            bildir(&uygulama, "Kasa", &format!("\"{}\" dolduruldu.", kayit.label));
        }
        Err(e) => {
            pencereyi_ac("");
            let _ = uygulama.emit("kisayol-notu", format!("yazma-hatasi:{e}"));
        }
    }
}

/// Erişilebilirlik iznini arka planda izler ve DEĞİŞTİĞİNDE arayüze haber verir.
///
/// NEDEN GEREKLİ: izin Sistem Ayarları'ndan veriliyor, uygulamanın dışında.
/// Yalnızca açılışta baksaydık, kullanıcı izni verdikten sonra da ekranda
/// "izin yok" yazmaya devam ederdi ve uygulamayı kapatıp açması gerekirdi.
fn izin_izle(uygulama: &tauri::AppHandle) {
    let u = uygulama.clone();
    std::thread::spawn(move || {
        let mut onceki = pencere::erisilebilirlik_izni_var();
        let _ = u.emit("izin-durumu", onceki);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let simdi = pencere::erisilebilirlik_izni_var();
            if simdi != onceki {
                onceki = simdi;
                let _ = u.emit("izin-durumu", simdi);
            }
        }
    });
}

/// Kısa sistem bildirimi — pencere açmadığımız için tek geri bildirim bu.
fn bildir(uygulama: &tauri::AppHandle, baslik: &str, govde: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = uygulama.notification().builder().title(baslik).body(govde).show();
}

/// Öndeki pencereyi arka planda izler ve KENDİMİZ DIŞINDAKİ son pencereyi
/// hedef olarak saklar.
///
/// NEDEN GEREKLİ: kullanıcının hedefi elle seçmesi ya da her seferinde kısayola
/// basması işi uzatıyordu. Profesyonel parola yöneticileri de böyle çalışır:
/// siz uygulamaya geçtiğinizde hedef ZATEN belirlenmiştir, çünkü ondan önceki
/// pencere biliniyordur.
///
/// KENDİ PENCEREMİZ NEDEN ATLANIYOR: kullanıcı Kasa'ya tıkladığı anda öndeki
/// pencere biz oluruz. O anki değeri yazsaydık hedef her seferinde "Bogahost
/// Kasa" olurdu — yani hiçbir zaman doğru olmazdı.
///
/// MALİYET: macOS'ta her tur bir `osascript` çağırıyor. Bu yüzden kendi
/// penceremiz odaktayken tur ATLANIYOR — kullanıcı zaten Kasa'ya bakıyorsa
/// hedefin değişmesi mümkün değil ve boşuna süreç açmanın anlamı yok.
fn hedef_izle(uygulama: &tauri::AppHandle) {
    const ARALIK_MS: u64 = 1500;
    let u = uygulama.clone();

    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(ARALIK_MS));

        // Kendi penceremiz odaktaysa tur atla.
        let bizdeyiz = u
            .get_webview_window("main")
            .and_then(|p| p.is_focused().ok())
            .unwrap_or(false);
        if bizdeyiz {
            continue;
        }

        let h = pencere::ondeki();
        if h.program.is_empty() || h.program == "Bogahost Kasa" {
            continue;
        }

        // Aynı hedefe tekrar tekrar olay göndermeyelim; arayüz boşuna
        // eşleşme sorgusu yapmasın diye kimlik VE adres birlikte kıyaslanıyor
        // (aynı tarayıcıda sekme değişince adres değişir, kimlik değişmez).
        let durum = u.state::<HedefDurum>();
        let degisti = {
            let mevcut = durum.0.lock().unwrap();
            mevcut.kimlik != h.kimlik || mevcut.url != h.url
        };
        if !degisti {
            continue;
        }

        *durum.0.lock().unwrap() = h.clone();
        let _ = u.emit("hedef-degisti", h);
    });
}