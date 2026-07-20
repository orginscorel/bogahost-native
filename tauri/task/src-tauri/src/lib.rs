// Bogahost Görevler — Tauri v2 masaustu kabugu.
//
// Bu dosya 4 uygulamada (finans/dcim/chat/task) AYNIDIR; yalnizca asagidaki
// APP_KEY / APP_TITLE sabitleri farklidir. Degistirirken hepsini birlikte guncelleyin.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::{Color, DownloadEvent, PageLoadEvent, WebviewWindowBuilder},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Url, WebviewUrl, WindowEvent, Wry,
};
// `ManagerExt` adi `tauri::Manager` ile karismasin diye yeniden adlandirildi.
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartExt};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::{NotificationExt, PermissionState};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_updater::UpdaterExt;

const APP_KEY: &str = "task";
const APP_TITLE: &str = "Bogahost Görevler";

/// Uygulamalar arasi gecis tablosu — `apps.config.json` ile BIREBIR ayni olmali.
/// (key, menu etiketi, canli URL)
const APPS: [(&str, &str, &str); 4] = [
    ("finans", "Finans", "https://finans.bogahost.com/admin"),
    ("dcim", "DCIM", "https://dcim.bogahost.com/admin"),
    ("chat", "Chat", "https://chat.bogahost.com/admin"),
    ("task", "Görevler", "https://task.bogahost.com/admin"),
];

/// Uygulama icinde kalmasi gereken alan adi. Bu alan adinin DISINDAKI her adres
/// sistem tarayicisinda acilir (bkz. `is_internal_url`).
const INTERNAL_DOMAIN: &str = "bogahost.com";

/// GERIYE DONUK manifest. Birincil yol `tauri-plugin-updater`dir (indir + kur +
/// yeniden baslat). Updater kullanilamazsa (imzasiz build / pubkey PLACEHOLDER /
/// ag hatasi) bu adres okunur ve yalnizca BILDIRIM gosterilir.
/// v1.1.0 istemcileri de bu adresi okudugu icin adres DEGISTIRILMEMELIDIR.
/// Ayrinti: docs/UPDATE.md
const VERSION_MANIFEST_URL: &str = "https://bogahost.com/native/latest.json";

/// Manifest `url` alani vermezse acilacak varsayilan indirme sayfasi.
const DOWNLOAD_PAGE_URL: &str = "https://github.com/orginscorel/bogahost-native/releases";

/// Menu ogesi id on eki (uygulama gecisi).
const APP_MENU_PREFIX: &str = "app:";

/// WebView2 (Windows) ek tarayici argumanlari.
///
/// `additional_browser_args` Tauri'nin VARSAYILANLARININ YERINE GECER; bu yuzden
/// varsayilan `--disable-features=...` listesi AYNEN korunur ve uzerine
/// otomatik oynatma izni eklenir (bildirim sesi / arama zili ilk tiklamayi
/// beklemesin).
///
/// ⚠ AYNI veri klasorunu paylasan TUM webview'lerde AYNI olmalidir; farkli
/// argumanlar + ayni klasor WebView2'de olusturma hatasina yol acar
/// (tauri#11144). Bu yuzden ana pencere ve splash AYNI sabiti kullanir.
#[cfg(target_os = "windows")]
const WEBVIEW2_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required";

/// Ag islemleri icin ust sinir — acilista ASLA uzun sure beklenmez.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(7);

/// Kopru uzerinden indirme (Parasut e-belge gibi YAVAS/BUYUK, dis API arkasi)
/// icin ust sinir. WKWebView `fetch`->blob yolu bu tur yanitlarda guvenilmez
/// oldugu icin indirme Rust'ta yapilir; Parasut dis API yavas oldugundan
/// timeout comert tutulur.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// Otomatik guncelleme yeni surumun indirilecegi taban adres (manuel yedek yol).
/// Dosya adi: `<app>-<surum>-macos.dmg` / `<app>-<surum>-windows-x86_64-setup.exe`.
const DOWNLOAD_BASE_URL: &str = "https://native.bogahost.com/downloads";

/// Acilis surum denetimi bu kadar gecikmeyle baslar (sayfa yuklenmesiyle yarismasin).
const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(5);

/// UYGULAMA ACIKKEN periyodik surum denetimi araligi.
///
/// NEDEN VAR: v1.8.1'e kadar denetim YALNIZCA acilista yapiliyordu; tepside
/// gunlerce acik duran bir uygulama yeni surumu HIC gormuyordu. 45 dakika
/// bilincli bir dengedir: gun icinde birkac denetim (sunucuya yuk bindirmez,
/// istek basina birkac KB JSON) ama pil/veri acisindan ihmal edilebilir.
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(45 * 60);

/// Kullanici MESGULKEN (arama/ekran paylasimi/doldurulmus form) indirme ertelenir;
/// bu kadar sonra tekrar denenir. Bkz. `page_busy`.
const UPDATE_BUSY_RETRY: Duration = Duration::from_secs(5 * 60);

/// Yeniden baslatmadan ONCE acik olan sayfa adresi bu dosyada saklanir.
/// Surec olecegi icin bellek yetmez — kalici depo SART (bkz. `save_resume_url`).
const RESUME_FILE: &str = "resume-url.json";

/// Saklanan adres bu sureden eskiyse KULLANILMAZ. (Guncelleme yeniden baslatmasi
/// saniyeler surer; Windows'ta kullanici installer'i bekletirse dakikalar.
/// Gunler sonra acilan uygulamanin eski bir sayfaya dusmesi ISTENMEZ.)
const RESUME_MAX_AGE_SECS: u64 = 6 * 60 * 60;

/// Sayfa yuklenmese bile pencere en gec bu sure sonunda gosterilir.
const WINDOW_REVEAL_FALLBACK: Duration = Duration::from_secs(8);

/// Pencere konumu/boyutu bu dosyada saklanir (uygulama yapilandirma klasoru).
const WINDOW_STATE_FILE: &str = "window-state.json";

/// Bildirim yoklama araligi.
///
/// Saati RUST tutar (bkz. `start_notify_clock`), sayfa DEGIL. NEDEN: WebView
/// zamanlayicilari (`setInterval`) pencere gizlendiginde/ortuldugunde isletim
/// sistemi tarafindan KISILIR — tam da uygulama tepside dururken, yani
/// bildirimin en cok beklendigi anda. Isletim sistemi is parcacigi kisilmez.
const NOTIFY_POLL_INTERVAL: Duration = Duration::from_secs(45);

/// Gosterilmis bildirim anahtarlarinin KALICI listesi (uygulama yapilandirma
/// klasoru). 4 uygulamanin paket kimligi farkli oldugu icin bu dosya da
/// uygulama basina AYRIDIR — biri digerinin bildirimini yutmaz.
const NOTIFY_STATE_FILE: &str = "notify-state.json";

/// Kalici olarak hatirlanan bildirim anahtari sayisi (halka tampon).
const NOTIFY_SEEN_MAX: usize = 300;

/// Tek turda EN FAZLA bu kadar bildirim gosterilir; fazlasi tek ozete duser
/// (uzun sure kapali kalmis uygulama masaustunu bildirimle doldurmasin).
const NOTIFY_BURST_MAX: usize = 4;

/// ILK calistirmada (kalici liste henuz yokken) yalnizca bu yastan (saniye)
/// GENC kayitlar duyurulur; gecmis besleme topluca patlamaz.
const NOTIFY_FIRST_RUN_MAX_AGE: i64 = 120;

/// Acilis/gecis yukleme katmani icin uygulama kimlikleri.
/// (hostname, kisa ad, vurgu rengi, gecis durum metni)
///
/// Vurgu renkleri `apps.config.json` icinde 4 uygulamada da AYNI (`#5443D2`)
/// oldugu icin kimlik ayrimi burada yapilir: ortak marka moru + uygulamaya OZEL
/// ikincil vurgu. Katman hedef sayfada cizildigi icin bu tablo 4 binary'de de
/// aynidir (hangi uygulamadan hangisine gecildigi fark etmez).
const OVERLAY_APPS: [(&str, &str, &str, &str); 4] = [
    ("finans.bogahost.com", "Finans", "#22c55e", "Finans'a geçiliyor…"),
    ("dcim.bogahost.com", "DCIM", "#6a58ea", "DCIM'e geçiliyor…"),
    ("chat.bogahost.com", "Chat", "#06b6d4", "Chat'e geçiliyor…"),
    ("task.bogahost.com", "Görevler", "#f59e0b", "Görevler'e geçiliyor…"),
];

/// Pencere arka plani — sayfa gelene kadar BEYAZ parlama (FOUC) olmasin.
/// `apps.config.json > backgroundColor` (#0e1015) ile ayni.
const WINDOW_BG: Color = Color(0x0e, 0x10, 0x15, 0xff);

/// Otomatik baslatmada (oturum acilisi) uygulamaya gecilen argüman.
/// Bu argümanla acildiysa ana pencere GOSTERILMEZ; uygulama yalnizca tepside durur.
const HIDDEN_LAUNCH_FLAG: &str = "--hidden";

/// Gecis/acilis katmanini hedef sayfada UYANDIRMA denemeleri.
///
/// NEDEN COKLU DENEME: `eval` ve `navigate` ayni olay dongusune SIRAYLA
/// kuyruklanan "gonder-unut" mesajlaridir; `evaluate_script` her platformda
/// ASENKRONDUR. Bu yuzden `navigate`den ONCE yapilan `eval` cogu zaman yeni
/// belge olusurken YOK EDILIR (v1.5.1'de gecis gostergesinin hic gorunmemesinin
/// KOK NEDENI budur). Katman artik `initialization_script` ile hedef sayfada
/// document-start'ta kurulur; asagidaki denemeler yalnizca "bu bir uygulama
/// gecisi" bilgisini iletir. Biri bile yeni belgeye dusse yeterlidir; hepsi
/// dusmezse katman gene de "Yükleniyor…" olarak gorunur.
const OVERLAY_WAKE_DELAYS_MS: [u64; 5] = [0, 60, 150, 320, 650];

/// 4 uygulamanin PAYLASTIGI WebView veri klasoru (cerez/oturum deposu).
///
/// Neden paylasimli: bu projede SSO YOKTUR — her uygulama WHMCS admin bilgisiyle
/// KENDI alan adinda ayri dogrulama yapar. Her uygulama kendi ozel veri klasorunu
/// kullanirsa, DCIM uygulamasinda alinan `dcim.bogahost.com` oturum cerezi Finans
/// uygulamasinin WebView'inde GORUNMEZ; "Uygulamalar" menusunden gecis yapinca
/// yeniden giris istenir. Ortak klasor sayesinde 4 kabuk ayni cerez kavanozunu
/// paylasir: her uygulamaya BIR KEZ giris yapilir, gecislerde tekrar sorulmaz.
/// (Bu SSO DEGILDIR — sunucu tarafi degismez, yalnizca cerezler paylasilir.)
///
/// DIKKAT: WebView2 (Windows) TEK bir surecteki TUM webview'lerin AYNI veri
/// klasorunu kullanmasini zorunlu kilar — bu yuzden hem `main` hem `splash`
/// penceresine ayni klasor verilir (bkz. `build_main_window` / `build_popup_window`).
const SHARED_WEBVIEW_DIR_NAME: &str = "BogahostNative";

/// Es zamanli/cift surum denetimini engeller.
static UPDATE_CHECK_RUNNING: AtomicBool = AtomicBool::new(false);

/// `tauri-plugin-updater` calisma aninda basariyla yuklendi mi?
/// Yuklenmediyse `app.updater()` cagrilmaz (yonetilmeyen state -> panic olurdu).
static UPDATER_READY: AtomicBool = AtomicBool::new(false);

// NOT: v1.8.1'e kadar burada bir `UPDATE_PROMPT_OPEN` bayragi vardi; guncelleme
// bulununca "Şimdi kurulsun mu?" DIYALOGU aciliyordu ve diyalogun cift acilmasini
// engelliyordu. Diyalog KALDIRILDI (kullaniciyi boluyordu, ozellikle gorusme
// sirasinda) — yerine sessiz indirme + sayfa ici serit geldi.

/// Arka planda indirme/kurulum SU AN suruyor mu? (ust uste binmeyi onler)
static UPDATE_INSTALLING: AtomicBool = AtomicBool::new(false);

/// Otomatik kurulum KALICI olarak basarisiz mi? (imzasiz macOS: calisan uygulama
/// kendi bundle'ini degistiremez.) True ise serit "Şimdi uygula" yerine "İndirme
/// sayfasını aç" (manuel indirme) gosterir — tekrar denemek ise yaramaz.
static UPDATE_MANUAL: AtomicBool = AtomicBool::new(false);

/// Sayfa "mesgulum" dedi mi? (gorusme / ekran paylasimi / doldurulmus form)
///
/// Sayfa tarafindan `bogahost_set_busy` ile bildirilir; bkz. `UPDATE_UI_JS`.
/// VARSAYILAN `false`'tir: sayfa hic haber vermezse "mesgul degil" kabul edilir.
/// Bu bilincli bir tercihtir — bayrak yalnizca OTOMATIK indirmeyi geciktirir,
/// yeniden baslatma zaten HICBIR zaman kendiliginden olmaz.
static PAGE_BUSY: AtomicBool = AtomicBool::new(false);

/// Yeniden baslatma AKISI basladi mi? Baslamis ise cikis uyarisi GOSTERILMEZ
/// (`app.restart()` de `ExitRequested` tetikler — uyari akisi restart'i
/// `handle.exit(0)`'a cevirip uygulamayi geri acmadan kapatirdi).
static RESTART_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Indirilmis (macOS/Linux'ta ayrica KURULMUS) ve kullanicinin onayini bekleyen surum.
static PENDING_UPDATE: Mutex<Option<PendingUpdate>> = Mutex::new(None);

/// Ana pencerede SU AN acik olan adres — yeniden baslatmada geri donulecek yer.
static CURRENT_PAGE_URL: Mutex<Option<String>> = Mutex::new(None);

/// Uygulanmayi bekleyen guncelleme.
struct PendingUpdate {
    /// Kullaniciya gosterilen surum ("1.8.2").
    version: String,
    /// YALNIZCA WINDOWS'ta doludur.
    ///
    /// NEDEN: `tauri-plugin-updater`in Windows kurulumu installer'i calistirip
    /// `std::process::exit(0)` ile SURECI OLDURUR. Yani Windows'ta "sessiz kur,
    /// sonra sor" MUMKUN DEGILDIR — arka planda yalnizca INDIRIRIZ, kurulum
    /// kullanici "Şimdi uygula" dedigi an yapilir.
    /// macOS/Linux'ta kurulum (uygulama paketinin degistirilmesi) surec
    /// calisirken tamamlanir, bu alan `None` kalir ve onay yalnizca yeniden
    /// baslatmayi tetikler.
    installer: Option<(tauri_plugin_updater::Update, Vec<u8>)>,
}

/// Pencere bir kez gosterildi mi? (beyaz ekran yerine "yuklenince goster")
static WINDOW_REVEALED: AtomicBool = AtomicBool::new(false);

/// Otomatik baslatma (`--hidden`) ile mi acildi? Oyleyse pencere gosterilmez.
static LAUNCHED_HIDDEN: AtomicBool = AtomicBool::new(false);

/// "Uygulamalar" menusunden gecis yapildi mi? Yapildiysa ILK sayfa yuklemesinden
/// sonra erisim denetimi (403/401) calistirilir — bkz. `ACCESS_CHECK_SCRIPT`.
static PENDING_ACCESS_CHECK: AtomicBool = AtomicBool::new(false);

/// Acilan onizleme (popup) pencerelerine benzersiz etiket uretir: `popup-0`, `popup-1` ...
/// Etiket deseni `capabilities/*.json` icindeki `popup-*` ile ESLESMELIDIR.
static POPUP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Uygulama gecisinde "Yükleniyor" katmani en gec bu sure sonunda kaldirilir.
///
/// Katman `PageLoadEvent::Finished` ile kaldiriliyordu; sayfa HIC yuklenmezse
/// (ag hatasi, 403, sunucu yanit vermiyor) ekranda KALICI "Yükleniyor…" kaliyor
/// ve kullanici "gecis yok / uygulama dondu" olarak goruyordu.
const SWITCH_LOADING_TIMEOUT: Duration = Duration::from_secs(15);

/// Pencere durumu diske en son ne zaman yazildi (asiri yazmayi onler).
static LAST_STATE_SAVE: Mutex<Option<Instant>> = Mutex::new(None);

/// Tepsi + macOS menu cubugundaki durum.
struct AppState {
    /// (uygulama anahtari, isaretlenebilir menu ogesi) — tepsi + menu cubugu.
    items: Mutex<Vec<(String, CheckMenuItem<Wry>)>>,
    /// Su an acik olan uygulama anahtari.
    current: Mutex<String>,
    /// Yedek guncelleme yolunda manifestten gelen indirme adresi.
    download_url: Mutex<String>,
    /// WebView yakinlastirma carpani.
    zoom: Mutex<f64>,
    /// En son indirilen dosyanin tam yolu ("Finder'da goster" icin).
    last_download: Mutex<Option<PathBuf>>,
    /// "Bildirimler: acik/kapali" tepsi ogeleri (durum guncellenebilsin diye).
    notify_items: Mutex<Vec<MenuItem<Wry>>>,
    /// "Bilgisayar acilinca baslat" isaretlenebilir tepsi ogeleri.
    autostart_items: Mutex<Vec<CheckMenuItem<Wry>>>,
    /// En son gosterilen bildirimin hedef adresi.
    ///
    /// NEDEN: masaustunde `tauri-plugin-notification` bildirime TIKLAMA olayi
    /// SUNMAZ. Bu yuzden "bildirime tiklayinca ilgili sayfaya git" yerine tepsi
    /// menusune "Son bildirimi ac" ogesi konuldu — ayni ise yarar, olmayan bir
    /// API uydurmaz.
    last_notify_url: Mutex<Option<String>>,
}

pub fn run() {
    // Oturum acilisinda otomatik baslatildiysa pencere GOSTERILMEZ (yalniz tepsi).
    // Argüman `tauri_plugin_autostart::init(...)` ile kayit girdisine yazilir.
    if std::env::args().any(|a| a == HIDDEN_LAUNCH_FLAG) {
        LAUNCHED_HIDDEN.store(true, Ordering::SeqCst);
    }

    tauri::Builder::default()
        // Harici linkleri sistem tarayicisinda acmak + genel shell erisimi.
        .plugin(tauri_plugin_shell::init())
        // Oturum acilisinda otomatik baslatma ("arka planda calisir kal").
        // macOS: LaunchAgent. Uygulama `--hidden` ile acilir -> sessizce tepsiye.
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![HIDDEN_LAUNCH_FLAG]),
        ))
        // Native masaustu bildirimleri.
        .plugin(tauri_plugin_notification::init())
        // Guncelleme onay diyalogu.
        .plugin(tauri_plugin_dialog::init())
        // Sayfadan (blob/data URL) gelen indirmeleri diske yazan kopru.
        .invoke_handler(tauri::generate_handler![
            bogahost_save_file,
            bogahost_open_external,
            bogahost_notify,
            bogahost_notify_feed,
            bogahost_notify_state,
            bogahost_notify_request,
            bogahost_open_popup,
            bogahost_close_window,
            bogahost_print,
            bogahost_open_settings,
            bogahost_focus_window,
            bogahost_set_fullscreen,
            bogahost_reveal_download,
            bogahost_set_busy,
            bogahost_apply_update,
            bogahost_update_state,
            bogahost_fetch_download,
            bogahost_open_download
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // ----- Otomatik guncelleme eklentisi -----
            // Builder zincirinde DEGIL, burada kayit ediliyor: pubkey PLACEHOLDER
            // veya bozuksa eklenti yuklenmez, hata YUTULUR ve uygulama normal
            // calismaya devam eder (asla kilitlenmez/cokmez).
            match handle.plugin(tauri_plugin_updater::Builder::new().build()) {
                Ok(()) => UPDATER_READY.store(true, Ordering::SeqCst),
                Err(e) => log_update(&format!(
                    "eklenti yuklenemedi ({e}) — eski manifest denetimine dusulecek"
                )),
            }

            // ----- Acilis yukleme ekrani -----
            // AYRI bir splash PENCERESI YOKTUR (v1.5.1'de o yaklasim calismadi).
            // Katman ana pencerenin webview'inde, hedef sayfada document-start'ta
            // cizilir — bkz. `LOADING_OVERLAY_JS` / `wake_overlay`.

            // ----- Ana pencere -----
            // Pencere tauri.conf.json'da DEGIL burada olusturuluyor; cunku
            // `on_download` / `on_navigation` / `on_page_load` yalnizca
            // WebviewWindowBuilder uzerinden baglanabilir (indirme destegi bunlara bagli).
            let window = build_main_window(&handle)?;

            // Acilis yukleme ekranini hedef sayfada uyandir ("Bogahost <ad>" +
            // bogahost.com + asamali durum metni). Katman zaten document-start'ta
            // cizilir; bu cagri onu tam ekran acilis kipine alir.
            wake_overlay(&handle, "boot");

            // ----- Otomatik baslatma (autostart) -----
            // ILK KURULUMDA VARSAYILAN: ACIK. Kullanici tepsiden kapatabilir;
            // karari isletim sistemi kaydinda tutulur, biz bir daha zorlamayiz.
            if mark_once(&handle, "autostart-default") {
                if let Err(e) = handle.autolaunch().enable() {
                    eprintln!("[{}][autostart] acilamadi: {}", APP_KEY, e);
                }
            }

            // Menulerde kullanilan isaretlenebilir "Uygulamalar" ogelerinin tamami.
            let mut switch_items: Vec<(String, CheckMenuItem<Wry>)> = Vec::new();
            let mut notify_items: Vec<MenuItem<Wry>> = Vec::new();
            let mut autostart_items: Vec<CheckMenuItem<Wry>> = Vec::new();

            // ----- Sistem tepsisi (tray) menusu -----
            // EN USTTE surum satiri: pasif (tiklanamaz) bilgi ogesi.
            // Kullanici giris yapmis olsun ya da olmasin surum HER ZAMAN buradan
            // gorunur — giris ekranindaki rozete bagimli degildir.
            let version_i = MenuItem::with_id(
                app,
                "version-info",
                format!("{APP_TITLE} v{}", env!("CARGO_PKG_VERSION")),
                false,
                None::<&str>,
            )?;
            let about_i = MenuItem::with_id(app, "about", "Hakkında…", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Goster / Show", true, None::<&str>)?;
            let hide_i = MenuItem::with_id(app, "hide", "Gizle / Hide", true, None::<&str>)?;
            let notify_i = MenuItem::with_id(
                app,
                "notify-status",
                "Bildirimler: denetleniyor…",
                true,
                None::<&str>,
            )?;
            let dlfolder_i = MenuItem::with_id(
                app,
                "downloads-folder",
                "İndirilenler klasörünü aç",
                true,
                None::<&str>,
            )?;
            let dllast_i = MenuItem::with_id(
                app,
                "downloads-last",
                "Son indirilen dosyayı göster",
                true,
                None::<&str>,
            )?;
            // Masaustunde bildirime TIKLAMA olayi yoktur (bkz. AppState::last_notify_url).
            let notiflast_i = MenuItem::with_id(
                app,
                "notify-last-open",
                "Son bildirimi aç",
                true,
                None::<&str>,
            )?;
            let upd_i = MenuItem::with_id(
                app,
                "check-update",
                "Güncellemeleri denetle",
                true,
                None::<&str>,
            )?;
            let dl_i = MenuItem::with_id(
                app,
                "open-downloads",
                "İndirme sayfasını aç",
                true,
                None::<&str>,
            )?;
            // Oturum acilisinda baslat — kullanici istedigi an kapatabilir.
            let autostart_i = CheckMenuItem::with_id(
                app,
                "autostart-toggle",
                "Bilgisayar açılınca başlat",
                true,
                handle.autolaunch().is_enabled().unwrap_or(false),
                None::<&str>,
            )?;
            let quit_i = MenuItem::with_id(app, "quit", "Cikis / Quit", true, None::<&str>)?;

            let sep_v = PredefinedMenuItem::separator(app)?;
            let sep_a = PredefinedMenuItem::separator(app)?;
            let sep_b = PredefinedMenuItem::separator(app)?;
            let sep_c = PredefinedMenuItem::separator(app)?;
            let sep_d = PredefinedMenuItem::separator(app)?;

            let (tray_apps_sub, tray_apps_items) = build_apps_submenu(&handle)?;
            switch_items.extend(tray_apps_items);
            // Windows'ta menu cubugu yok; Yenile/yakinlastirma tepsiden erisilebilsin.
            let tray_view_sub = build_view_submenu(&handle)?;

            let tray_items: Vec<&dyn IsMenuItem<Wry>> = vec![
                &version_i,
                &sep_v,
                &show_i,
                &hide_i,
                &sep_a,
                &tray_apps_sub,
                &tray_view_sub,
                &sep_b,
                &dlfolder_i,
                &dllast_i,
                &notify_i,
                &notiflast_i,
                &autostart_i,
                &sep_c,
                &about_i,
                &upd_i,
                &dl_i,
                &sep_d,
                &quit_i,
            ];
            let menu = Menu::with_items(app, &tray_items)?;
            drop(tray_items);
            notify_items.push(notify_i);
            autostart_items.push(autostart_i);

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(
                    app.default_window_icon()
                        .expect("default window icon (icons/icon.png) mevcut olmali")
                        .clone(),
                )
                .tooltip(APP_TITLE)
                .menu(&menu)
                .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
                .on_tray_icon_event(|tray, event| {
                    // Tepsi ikonuna sol tik -> pencereyi geri getir.
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            // ----- macOS menu cubugu -----
            // Standart menuler (Uygulama / Düzen / Pencere) ACIKCA kuruluyor:
            // Cmd+C / Cmd+V / Cmd+X / Cmd+A / Cmd+Z / Cmd+Q / Cmd+W / Cmd+M / Cmd+H
            // kaybolmamali. Uzerine "Görünüm" ve "Uygulamalar" ekleniyor.
            #[cfg(target_os = "macos")]
            {
                let (app_menu, mac_apps_items) = build_menu_bar(&handle)?;
                switch_items.extend(mac_apps_items);
                let _ = app.set_menu(app_menu)?;
            }

            app.manage(AppState {
                items: Mutex::new(switch_items),
                current: Mutex::new(APP_KEY.to_string()),
                download_url: Mutex::new(DOWNLOAD_PAGE_URL.to_string()),
                zoom: Mutex::new(1.0),
                last_download: Mutex::new(None),
                notify_items: Mutex::new(notify_items),
                autostart_items: Mutex::new(autostart_items),
                last_notify_url: Mutex::new(None),
            });

            // Menu cubugu (ve varsa pencere menusu) olaylari.
            app.on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()));

            // Kayitli pencere konumu/boyutu.
            restore_window_state(&window);

            // ----- Pencere gorunurlugu icin emniyet agi -----
            // Sayfa hic yuklenmese de (ag yok) pencere ortada kaybolmasin.
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(WINDOW_REVEAL_FALLBACK);
                    reveal_window(&h);
                });
            }

            // ----- Bildirim izni -----
            // macOS/Windows sistem onay penceresi ancak ACIKCA istenince cikar.
            // Arka planda calisir; acilisi BLOKLAMAZ.
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(2));
                    ensure_notification_permission(&h);
                    refresh_notification_menu(&h);
                    refresh_autostart_menu(&h);
                });
            }

            // ----- Bildirim yoklama saati -----
            // Tarayiciya / Apple'a (web-push, service worker, APNs) HIC bagli
            // olmayan yol: kabuk panelin bildirim ucunu kendisi yoklatir ve
            // sonucu NATIVE bildirim olarak gosterir. Bkz. `start_notify_clock`.
            start_notify_clock(&handle);

            // ----- ACILIS OTOMATIK GUNCELLEMESI (ASIL yol) -----
            //
            // Eski (v1.7/1.8) davranis: uygulama YENI ACILIRKEN, kullanici henuz
            // etkilesime girmeden, sessizce denetler; guncelleme varsa ONAY
            // SORMADAN indirip kurar ve yeniden baslatir ("kapatip acinca
            // guncellensin"). KOK NEDEN bunun ASIL yol olmasi: imzasiz macOS'ta
            // CALISAN uygulama kendi bundle'ini degistiremez (calisirken "Şimdi
            // uygula" install FAIL verir), ama ACILIS penceresinde bundle degisimi
            // calisir (kullanici dogruladi). Calisirken serit (asagidaki dongu)
            // yalnizca uzun acik kalanlar icin YEDEK yoldur.
            //
            // BLOKLAMAZ: `check` kisa timeout'ludur (NETWORK_TIMEOUT); guncelleme
            // yoksa ya da ag yoksa uygulama normal acilir. Ayri bir gorevde calisir,
            // pencere gosterimini bekletmez.
            {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    startup_auto_update(h).await;
                });
            }

            // ----- Guncelleme denetimi: UYGULAMA ACIKKEN periyodik (YEDEK) -----
            //
            // Acilis guncellemesinden SONRA dongu `UPDATE_CHECK_INTERVAL` araliyla
            // devam eder — boylece tepside gunlerce acik kalan uygulama da yeni
            // surumu gorur. Bu yol otomatik YENIDEN BASLATMAZ; hazir olunca sayfa
            // icinde serit cikar (kullanici "Şimdi uygula" der). Imzasiz macOS'ta
            // kurulum kalici basarisizsa serit "İndirme sayfasını aç"a doner.
            //
            // Denetim SESSIZDIR: guncelleme yoksa ya da hata olursa kullaniciya
            // HICBIR sey gosterilmez, yalnizca stderr'e yazilir.
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(UPDATE_CHECK_DELAY);
                    loop {
                        // Kullanici mesgulse (arama/ekran paylasimi/form) hem
                        // denetim hem indirme ERTELENIR: bant genisligi ve
                        // CPU o an kullanicinindir.
                        let busy = page_busy();
                        if !busy {
                            let h2 = h.clone();
                            tauri::async_runtime::spawn(async move {
                                run_update_flow(h2, false).await;
                            });
                        }
                        std::thread::sleep(if busy {
                            UPDATE_BUSY_RETRY
                        } else {
                            UPDATE_CHECK_INTERVAL
                        });
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Bu isleyici TUM pencereler icin calisir. Splash penceresi ne tepsiye
            // gizlenmeli ne de konumu/boyutu ana pencerenin durumu olarak
            // kaydedilmeli — bu yuzden once etiket denetlenir.
            if window.label() != "main" {
                return;
            }
            match event {
                // Pencere kapatilinca uygulamayi kapatma, tepsiye gizle (masaustu app hissi).
                WindowEvent::CloseRequested { api, .. } => {
                    save_window_state(window, true);
                    let _ = window.hide();
                    api.prevent_close();
                }
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    save_window_state(window, false);
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("Bogahost Tauri uygulamasi olusturulurken hata")
        .run(|app_handle, event| match event {
            // Cmd+Q / tepsi "Cikis": cikis ENGELLENMEZ, yalnizca ILK SEFER
            // kullaniciya kapaliyken bildirim gelmeyecegi hatirlatilir.
            // `mark_once` false donunce bu dal bir daha calismaz.
            //
            // `app.restart()` DE bu olayi tetikler. Guncelleme yeniden baslatmasi
            // sirasinda uyari akisi devreye girseydi, uyari diyalogunun sonundaki
            // `handle.exit(0)` restart'i duz bir CIKISA cevirir ve uygulama geri
            // acilmazdi — bu yuzden `RESTART_IN_PROGRESS` denetlenir.
            tauri::RunEvent::ExitRequested { api, .. } => {
                if !RESTART_IN_PROGRESS.load(Ordering::SeqCst) && mark_once(app_handle, "quit-notice")
                {
                    api.prevent_exit();
                    show_quit_notice(app_handle);
                }
            }
            // macOS: pencere kapatilinca uygulama Dock'ta calisir kalir (close-to-tray).
            // Dock ikonuna tiklaninca macOS "Reopen" olayi gonderir; BU ISLENMEZSE pencere
            // bir daha geri gelmez. Bildirilen "dock'ta duruyor ama acilmiyor" hatasi buydu.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { has_visible_windows, .. } => {
                if !has_visible_windows {
                    show_main_window(app_handle);
                }
            }
            _ => {}
        });
}

// ---------------------------------------------------------------------------
// Ana pencere: indirme + gezinme + yukleniyor gostergesi
// ---------------------------------------------------------------------------

/// Ana pencereyi olusturur. Pencere BASLANGICTA GIZLIDIR; ilk sayfa yuklenince
/// (veya `WINDOW_REVEAL_FALLBACK` dolunca) gosterilir — boylece acilista
/// beyaz/donuk bir kare gorunmez.
fn build_main_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow<Wry>> {
    let home = APPS
        .iter()
        .find(|e| e.0 == APP_KEY)
        .map(|e| e.2)
        .unwrap_or("https://bogahost.com/");

    // ----- Guncelleme yeniden baslatmasindan sonra KALDIGI YERE DON -----
    // `take_resume_url` dosyayi okuyup SILER (tek seferlik) ve yalnizca taze +
    // ic (bogahost.com) bir adres donerse kullanilir. Yoksa panel anasayfasi.
    let start = take_resume_url(app).unwrap_or_else(|| home.to_string());
    let url = Url::parse(&start)
        .or_else(|_| Url::parse(home))
        .expect("baslangic URL'i gecerli olmali");
    if let Ok(mut slot) = CURRENT_PAGE_URL.lock() {
        *slot = Some(url.to_string());
    }
    let nav_handle = app.clone();

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title(APP_TITLE)
        .inner_size(1280.0, 860.0)
        .min_inner_size(960.0, 640.0)
        .resizable(true)
        .center()
        .visible(false)
        // Sayfa gelene kadar pencere KOYU olsun; beyaz parlama (FOUC) olmasin.
        .background_color(WINDOW_BG)
        .theme(Some(tauri::Theme::Dark))
        .zoom_hotkeys_enabled(true)
        // ----- Surukle-birak ile dosya yukleme -----
        // Tauri'nin KENDI surukle-birak isleyicisi VARSAYILAN OLARAK aciktir ve
        // isletim sistemi olayini YUTAR. Bunun yan etkisi: sayfanin `dragover` /
        // `drop` olaylari HIC tetiklenmez, yani `<input type=file>` alanina veya
        // sohbet penceresine dosya SURUKLENEMEZ. wry'nin kendi belgesi bunu
        // acikca soyler ("...it won't be possible to drop files on
        // <input type=file> forms"). Bu davranis Windows'a OZGU DEGILDIR;
        // macOS ve Linux arka uclarinda da ayni sekilde engellenir.
        //
        // Kabuk zaten surukle-birakla bir sey YAPMIYOR (dinleyici yok), bu yuzden
        // isleyiciyi kapatmak hicbir ozelligi kaybettirmez, panelin kendi
        // yukleme alanlarini CALISIR HALE getirir.
        .disable_drag_drop_handler()
        // Surum bilgisi (giris ekranindaki rozet + panel sidebar'indaki
        // "Uygulama v…" satiri) JS'e burada aktarilir.
        // `initialization_script` HER GEZINMEDE, sayfanin kendi script'lerinden
        // ONCE (document-start) calisir — dolayisiyla panel kodu calistiginda
        // `window.__BOGAHOST_NATIVE_VERSION__` HAZIRDIR.
        .initialization_script(main_init_script().as_str())
        // WebView'in acamayacagi semalar (mailto:, tel:, ...) sistem uygulamasina
        // yollanir — tiklanip hicbir sey olmamasi ENGELLENIR.
        //
        // http/https gezinmeleri ENGELLENMEZ: bu geri cagirma iframe'ler ve
        // yonlendirmeler icin de calistigi icin (reCAPTCHA, gomulu video, oturum
        // yonlendirmeleri) korlemesine engellemek sayfalari bozardi. Harici LINK
        // tiklamalari, hangisinin gercek bir kullanici tiklamasi oldugunu bilen
        // sayfa koprusu (`INIT_SCRIPT` -> `bogahost_open_external`) tarafindan
        // sistem tarayicisina yollanir.
        .on_navigation(move |url| {
            match url.scheme() {
                "http" | "https" => {
                    // EMNIYET AGI: ust duzey gezinme bir BELGEYE gittiyse panele don.
                    guard_document_navigation(&nav_handle, url);
                    true
                }
                "tauri" | "file" | "about" | "data" | "blob" | "asset" | "ipc" => true,
                _ => {
                    let _ = nav_handle.shell().open(url.to_string(), None);
                    false
                }
            }
        })
        .on_page_load(|window, payload| {
            // Belge OLUSTU: yukleme katmani (document-start) cizilmis durumda.
            // Pencereyi burada acmak "hicbir sey yok" hissini onler; katman
            // boyanmasi icin kisa bir pay birakilir.
            if matches!(payload.event(), PageLoadEvent::Started) {
                // Yeni BELGE: onceki sayfanin "mesgulum" bayragi gecersizdir.
                // (Sayfa kendi bayragini yeniden bildirir; bkz. `UPDATE_UI_JS`.)
                // Bu sifirlama olmasaydi, arama sayfasindan cikildiginda bayrak
                // TAKILI kalir ve guncelleme indirmesi sonsuza dek ertelenirdi.
                PAGE_BUSY.store(false, Ordering::SeqCst);
                remember_page_url(payload.url());
                let h = window.app_handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(180));
                    reveal_window(&h);
                });
            }
            if matches!(payload.event(), PageLoadEvent::Finished) {
                remember_page_url(payload.url());
                reveal_window(window.app_handle());
                // Yukleme katmanini kaldir (katman kendi kendine de kalkar;
                // bu, sayfa `load` olayini hic vermezse ikinci guvencedir).
                let _ = window.eval(HIDE_OVERLAY_SCRIPT);
                // EMNIYET AGI: surum degiskenleri asil olarak
                // `initialization_script` ile (document-start) kurulur; sayfa
                // bunlari herhangi bir sebeple kaybederse burada tazelenir.
                let _ = window.eval(version_script().as_str());
                // "Uygulamalar" menusunden gecildiyse: hedef uygulama 403/401
                // donuyorsa (yonetimce erisim engellenmis) anlasilir bir ekran goster.
                if PENDING_ACCESS_CHECK.swap(false, Ordering::SeqCst) {
                    let _ = window.eval(ACCESS_CHECK_SCRIPT);
                }
                // Serit sayfanin DOM'una cizilir; gezinme onu yok eder.
                // Bekleyen bir guncelleme varsa yeni sayfada TEKRAR cizilir.
                // (Kullanici "Sonra" dediyse serit kendi kendini gizler —
                // karar `sessionStorage`da tutulur, bkz. `UPDATE_UI_JS`.)
                if let Some(version) = pending_version() {
                    show_update_banner(window.app_handle(), &version, false);
                }
            }
        })
        // Govde `download_requested` / `download_finished` icinde — onizleme
        // (popup) pencereleri de AYNI mantigi kullanir.
        .on_download(|webview, event| {
            let app = webview.app_handle().clone();
            match event {
                DownloadEvent::Requested { url, destination } => {
                    download_requested(&app, &url, destination)
                }
                DownloadEvent::Finished { url, path, success } => {
                    download_finished(&app, &url, path, success);
                    true
                }
                _ => true,
            }
        });

    // ----- Bildirim sesi / arama zili (Windows) -----
    // WebView2 varsayilan olarak "kullanici etkilesimi olmadan ses calma" yasagi
    // uygular; panelin arama zili ve bildirim sesi ilk tiklamaya kadar CALMAZ.
    // `additional_browser_args` Tauri'nin VARSAYILAN argumanlarinin YERINE GECER
    // (unwrap_or_else) — bu yuzden varsayilanlar AYNEN tekrar yazilmistir,
    // aksi halde msWebOOUI/msPdfOOUI/SmartScreen davranislari degisirdi.
    //
    // macOS/Linux'ta karsiligi YOKTUR: wry'de `with_autoplay` vardir ama Tauri
    // 2 bunu disari acmaz. Oralarda ses kilidi sayfa tarafinda ilk kullanici
    // hareketiyle acilir (bkz. EXTRA_SCRIPT -> "ses kilidi").
    #[cfg(target_os = "windows")]
    {
        builder = builder.additional_browser_args(WEBVIEW2_BROWSER_ARGS);
    }

    // Oturum cerezleri 4 uygulamada PAYLASILIR — bkz. `SHARED_WEBVIEW_DIR_NAME`.
    // Klasor hazirlanamazsa varsayilan (uygulamaya ozel) depo kullanilir:
    // gecislerde tekrar giris istenir ama uygulama CALISMAYA DEVAM EDER.
    //
    // ⚠ macOS SINIRI: `data_directory` yalnizca Windows (WebView2) ve Linux
    // (WebKitGTK) arka uclarinda ETKILIDIR. WKWebView'de karsiligi YOKTUR ve
    // wry bu degeri macOS'ta SESSIZCE YOK SAYAR — macOS'ta her uygulama
    // `WKWebsiteDataStore::defaultDataStore` kullanir, yani 4 kabuk cerezleri
    // PAYLASMAZ ve her birinde AYRI giris yapilir. Bu bir hata degil, ust akis
    // (Tauri 2.11) sinirdir; cozumu `with_data_store_identifier` (macOS 14+)
    // olurdu ama Tauri bunu da disari acmaz.
    if let Some(dir) = shared_webview_dir(app) {
        builder = builder.data_directory(dir);
    }

    builder.build()
}

/// Bu uygulamanin panel adresi (kurtarma hedefi) — `APPS` tablosundan.
fn app_home_url() -> &'static str {
    APPS.iter()
        .find(|e| e.0 == APP_KEY)
        .map(|e| e.2)
        .unwrap_or("https://bogahost.com/")
}

/// Adres BELGE gibi mi duruyor? (sunucunun `Content-Type`'ini burada goremeyiz.)
/// Sayfa koprusu icindeki `looksLikeDownload` ile ayni ailedendir; bu yol
/// gezinme IPTAL ETMEZ, yalnizca gerceklesmis bir gezinmeyi geri alir.
///
/// ONEMLI (KOK NEDEN duzeltmesi): Paraşüt fatura PDF uclari UZANTISIZDIR
/// (`/finans/parasut/fatura/sales/123?indir=1`, `/finans/parasut/e-fatura-pdf/123`).
/// Eski surum SADECE dosya uzantisina bakiyordu; bu uclari KACIRIYOR ve emniyet
/// agi HIC calismiyordu — kullanici PDF acilinca panele donemiyordu. Artik
/// "İndir" niyeti tasiyan bayraklar (`?indir=1` vb.) ve uzantisiz belge yollari
/// da taninir. (HTML yazdirma onizlemeleri — `/raporlar/pdf`, `/teklifler/{id}/pdf`
/// — burada KASITLI OLARAK eslenmez: onlar HTML doner ve ayri onizleme
/// penceresinde acilir; geri-alma bir HTML sayfayi yanlislikla kapatmasin.)
fn looks_like_document_url(url: &Url) -> bool {
    let path = url.path().to_ascii_lowercase();
    let query = url.query().unwrap_or("").to_ascii_lowercase();

    // 1) Uzantisiz ama KESIN indirme niyeti tasiyan bayraklar (deger onemsiz).
    let has_flag = |name: &str| {
        query.split(|c: char| c == '&' || c == ';').any(|kv| {
            kv.split('=').next().map(|k| k == name).unwrap_or(false)
        })
    };
    if has_flag("indir") || has_flag("download") || has_flag("dl") || has_flag("export") {
        return true;
    }

    // 2) Uzantisiz belge yollari (yolun icinde acik sinyal).
    if path.contains("/e-fatura-pdf")
        || path.contains("/e-arsiv-pdf")
        || path.ends_with("/dekont")
        || path.contains("/dekont/")
    {
        return true;
    }

    // 3) Klasik: yol bir dosya uzantisiyla bitiyorsa.
    let ext = match path.rsplit_once('.') {
        Some((_, e)) => e,
        None => return false,
    };
    matches!(
        ext,
        "pdf"
            | "csv"
            | "xls"
            | "xlsx"
            | "doc"
            | "docx"
            | "ppt"
            | "pptx"
            | "zip"
            | "rar"
            | "7z"
            | "gz"
            | "tgz"
            | "tar"
            | "ics"
            | "sql"
    )
}

/// EMNIYET AGI — "dosya uygulamada acildi, geri donemiyorum" sorununun son savunmasi.
///
/// Sayfa koprusu (`INIT_SCRIPT` -> `handleMaybeDownload`) indirme niyetli
/// tiklamalarin BUYUK COGUNLUGUNU zaten yakalar. Bu ag yalnizca koprunun
/// devrede olmadigi hallerde is gorur: sunucu yonlendirmesi, sayfa JS'inin
/// dogrudan `location.href` atamasi, ya da kopru betiginin hic calismadigi
/// durumlar.
///
/// ONEMLI: `on_navigation` IFRAME'ler icin de calisir; bu yuzden burada
/// HICBIR GEZINME ENGELLENMEZ. Kisa bir bekleyisin ardindan pencerenin
/// GERCEK (ust duzey) adresine bakilir — iframe ise adres degismemistir ve
/// hicbir sey yapilmaz. Boylece gomulu PDF onizlemeleri bozulmaz.
fn guard_document_navigation(app: &AppHandle, url: &Url) {
    if !looks_like_document_url(url) {
        return;
    }
    let target = url.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        // WKWebView belgeyi ANA CERCEVEDE yerlestirene kadar `window.url()`
        // guncellenmemis olabilir; kisa araliklarla (12 x 250ms ≈ 3sn) bakariz.
        //
        // iframe AYRIMI (gomulu PDF onizlemeleri bozulmasin): yalnizca UST
        // CERCEVE adresi GERCEKTEN bir belge ise geri aliriz. Gezinme bir
        // iframe'deyse ust cerceve adresi HALA panelin HTML adresidir
        // (`looks_like_document_url` false) -> dokunmayiz. Bu, eski surumdeki
        // "adres birebir esitligi" kapisindan DAHA saglamdir (yonlendirme /
        // sorgu normalizasyonu / WKWebView'in URL'i gec bildirmesi durumlarinda
        // eski kapi erken cikip geri-donusu HIC yapmiyordu — kilitlenme buydu).
        let mut returned = false;
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(250));
            let Some(window) = app.get_webview_window("main") else {
                return;
            };
            match window.url() {
                Ok(current) if looks_like_document_url(&current) => {
                    return_to_panel(&window);
                    returned = true;
                    break;
                }
                Ok(current) if is_internal_url(&current) => {
                    // Ust cerceve panel sayfasi -> gezinme iframe'deydi ya da
                    // kullanici zaten donduruldu. Dokunma.
                    return;
                }
                // Adres henuz belirsiz (about:blank / bos) -> tekrar dene.
                _ => {}
            }
        }
        if !returned {
            return;
        }
        // Panele donuldukten sonra dosyayi kopru uzerinden indir (oturum
        // cerezleriyle) -> kullanici tiklamasi bosa gitmesin.
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            if let Some(w) = app2.get_webview_window("main") {
                let js = format!(
                    "try {{ window.__bogahostDownload && window.__bogahostDownload({:?}); }} catch (e) {{}}",
                    target.as_str()
                );
                let _ = w.eval(js);
            }
        });
    });
}

/// Ana pencereyi panele geri getirir — kullanici hicbir kosulda belgede kilitli
/// kalmaz.
///
/// macOS/WKWebView'de ust duzey bir PDF ANA CERCEVEDE acildiginda dahili PDFKit
/// goruntuleyicisi devreye girer: bu goruntuleyicide HTML/DOM YOKTUR, bu yuzden
/// oraya gorunur bir "geri" seridi CIZILEMEZ. Geri donusun GARANTISI bu yuzden
/// `navigate` (WKWebView.load) ile paneli yeniden yuklemektir; `load` PDF
/// goruntuleyicisini de degistirir. WKWebView ilk `load`'u nadiren yutabildigi
/// icin kisa bir gecikmeyle, ust cerceve HALA belge ise, bir kez daha denenir.
/// (HTML belge — yazdirma onizlemesi — sayfalari icin ayrica sayfa koprusu
/// `__bogahostShowBackStrip` ile sabit bir "‹ Panele dön" seridi cizer.)
fn return_to_panel(window: &tauri::WebviewWindow) {
    let Ok(home) = Url::parse(app_home_url()) else {
        return;
    };
    // HTML belge ise gorunur serit ciz (native PDF'te sessizce no-op).
    let _ = window.eval(
        "try { window.__bogahostShowBackStrip && window.__bogahostShowBackStrip(); } catch (e) {}",
    );
    // Her kosulda panele don (PDF/HTML fark etmez) — GARANTILI kacis.
    let _ = window.navigate(home.clone());
    // Yeniden deneme: is parcacigina yalnizca `AppHandle` (Send) tasinir; pencere
    // parcacik ICINDE yeniden alinir (dosyanin her yerinde kullanilan guvenli
    // kalip — bkz. `guard_document_navigation`). WKWebView ilk `load`'u PDF
    // goruntuleyici aktifken yutarsa, ust cerceve HALA belge ise bir kez daha.
    let app = window.app_handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(450));
        if let Some(w) = app.get_webview_window("main") {
            if let Ok(current) = w.url() {
                if looks_like_document_url(&current) {
                    let _ = w.navigate(home);
                }
            }
        }
    });
}

/// Indirme baslamadan once hedefi belirler: Indirilenler klasoru,
/// ad cakismasinda "-1", "-2" ...
fn download_requested(app: &AppHandle, url: &Url, destination: &mut PathBuf) -> bool {
    let suggested = destination
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| file_name_from_url(url));
    let target = unique_path(&downloads_dir(app), &sanitize_file_name(&suggested));
    remember_download(app, &target);
    *destination = target;
    true
}

/// Indirme bitti: kullaniciya dosyanin TAM KONUMUNU bildir.
fn download_finished(app: &AppHandle, url: &Url, path: Option<PathBuf>, success: bool) {
    if success {
        // macOS'ta `path` None olabilir; o zaman istekte kaydettigimiz yolu kullaniriz.
        let saved = path.or_else(|| last_download(app));
        match saved {
            Some(p) => {
                remember_download(app, &p);
                notify_download_saved(app, &p);
            }
            None => notify(app, "İndirildi", "Dosya İndirilenler klasörüne kaydedildi."),
        }
    } else {
        let name = file_name_from_url(url);
        let message = format!("Dosya indirilemedi: {name}");
        notify(app, "İndirme başarısız", &message);
        page_toast(app, &message);
    }
}

/// Ana pencerede kisa bir bilgi mesaji gosterir (INIT_SCRIPT icindeki `toast`).
/// Kopru hazir degilse SESSIZCE gecilir — hicbir sey bozulmaz.
fn page_toast(app: &AppHandle, message: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let js = format!(
            "try {{ window.__bogahostToast && window.__bogahostToast({:?}); }} catch (e) {{}}",
            message
        );
        let _ = w.eval(js);
    }
}

/// Dosyayi sistem dosya yoneticisinde SECILI olarak gosterir
/// (macOS: Finder'da göster, Windows: Explorer'da seç). Basarisiz olursa
/// dosyanin bulundugu klasoru acar.
fn reveal_in_file_manager(app: &AppHandle, path: &Path) {
    #[cfg(target_os = "macos")]
    {
        if std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .is_ok()
        {
            return;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .is_ok()
        {
            return;
        }
    }

    let dir = path
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| downloads_dir(app));
    let _ = app.shell().open(dir.to_string_lossy().to_string(), None);
}

/// 4 kabugun ortak kullandigi WebView veri klasoru (cerez/oturum deposu).
///
/// `local_data_dir` (Windows: `%LOCALAPPDATA%`, macOS: `~/Library/Application Support`)
/// altinda uygulamadan BAGIMSIZ tek bir klasordur — bu yuzden Finans/DCIM/Chat/Görevler
/// kabuklari ayni cerezleri gorur.
///
/// Klasor olusturulamazsa `None` doner ve cagiran taraf varsayilan depoya duser
/// (hicbir kosulda acilis engellenmez).
fn shared_webview_dir(app: &AppHandle) -> Option<PathBuf> {
    let base = app
        .path()
        .local_data_dir()
        .or_else(|_| app.path().data_dir())
        .or_else(|_| app.path().home_dir())
        .ok()?;
    let dir = base.join(SHARED_WEBVIEW_DIR_NAME).join("webview");
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    Some(dir)
}

// ---------------------------------------------------------------------------
// Onizleme (popup) penceresi — "PDF acildi, uygulamaya geri donemiyorum" cozumu
// ---------------------------------------------------------------------------

/// `target="_blank"` / `window.open` ile acilmak istenen IC adresler icin AYRI,
/// CERCEVELI ve KAPATILABILIR bir pencere acar.
///
/// KOK NEDEN: bu adresler eskiden ANA pencerede aciliyordu (`location.href`).
/// Sunucu PDF/gorsel dondurdugunde WebView dosyayi yerinde goruntuluyor, panel
/// kayboluyor ve gorunur bir "geri" yolu kalmiyordu. Ayri pencerede:
///   * baslik cubugu + KAPAT dugmesi vardir (`decorations(true)`),
///   * macOS'ta Cmd+W calisir,
///   * ESC kapatir (bkz. `POPUP_INIT_SCRIPT`),
///   * ANA pencere panelde OLDUGU GIBI kalir.
fn open_popup_window(app: &AppHandle, url: Url) -> tauri::Result<()> {
    let index = POPUP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let label = format!("popup-{index}");

    // `label` String olarak GECILIR (`&String` -> `Into<String>` garantisi yok).
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::External(url))
        .title(format!("{APP_TITLE} — Önizleme"))
        .inner_size(1100.0, 780.0)
        .min_inner_size(480.0, 360.0)
        .resizable(true)
        .center()
        // Baslik cubugu + kapat dugmesi: kullanici HER ZAMAN kapatabilir.
        .decorations(true)
        .visible(true)
        .focused(true)
        .theme(Some(tauri::Theme::Dark))
        .zoom_hotkeys_enabled(true)
        // Ana pencereyle ayni gerekce: OS surukle-birak isleyicisi sayfanin
        // `drop` olayini yutmasin (bkz. `build_main_window`).
        .disable_drag_drop_handler()
        .initialization_script(popup_init_script().as_str())
        .on_download(|webview, event| {
            let app = webview.app_handle().clone();
            match event {
                DownloadEvent::Requested { url, destination } => {
                    download_requested(&app, &url, destination)
                }
                DownloadEvent::Finished { url, path, success } => {
                    download_finished(&app, &url, path, success);
                    true
                }
                _ => true,
            }
        });

    // Ana pencereyle AYNI cerez deposu — onizleme penceresi de oturumu gorur.
    if let Some(dir) = shared_webview_dir(app) {
        builder = builder.data_directory(dir);
    }

    // Ayni veri klasoru -> AYNI tarayici argumanlari (bkz. WEBVIEW2_BROWSER_ARGS).
    #[cfg(target_os = "windows")]
    {
        builder = builder.additional_browser_args(WEBVIEW2_BROWSER_ARGS);
    }

    builder.build()?;
    Ok(())
}

/// Sayfa koprusunun cagirdigi komut: ic adresi onizleme penceresinde acar.
/// Harici adres gelirse sistem tarayicisina yollanir (guvenlik).
#[tauri::command]
fn bogahost_open_popup(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|e| e.to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("desteklenmeyen adres".to_string()),
    }

    if !is_internal_url(&parsed) {
        return app.shell().open(url, None).map_err(|e| e.to_string());
    }

    open_popup_window(&app, parsed).map_err(|e| e.to_string())
}

/// Onizleme penceresini kendi icinden kapatir (ESC / "Kapat" dugmesi).
/// ANA pencere kapatilmaz — orada bu komut yok sayilir.
#[tauri::command]
fn bogahost_close_window(window: tauri::WebviewWindow<Wry>) -> Result<(), String> {
    if window.label() == "main" {
        return Ok(());
    }
    window.close().map_err(|e| e.to_string())
}

/// Sayfanin `window.print()` cagrisini native yazdirma akisina baglar.
///
/// `WebviewWindow::print()` (native yazdirma diyalogu) wry'de YALNIZCA macOS'ta
/// desteklenir; JS `window.print()` ise tum platformlarda calisir. Bu yuzden
/// macOS'ta once native yol, diger platformlarda sayfa tarafi kullanilir
/// (bkz. `trigger_print`) — HER DURUMDA TEK bir yazdirma diyalogu acilir.
#[tauri::command]
fn bogahost_print(window: tauri::WebviewWindow<Wry>) -> Result<(), String> {
    trigger_print(&window);
    Ok(())
}

/// Yazdirma akisini baslatir (menu ve sayfa koprusu ayni yolu kullanir).
/// TEK bir yazdirma diyalogu acilir: macOS'ta native, digerlerinde sayfa tarafi.
fn trigger_print(window: &tauri::WebviewWindow<Wry>) {
    // macOS: WebView'in kendi yazdirma diyalogu (wry yalnizca burada destekler).
    #[cfg(target_os = "macos")]
    {
        if window.print().is_ok() {
            return;
        }
    }

    // Windows/Linux (ve macOS yedek yolu): sayfanin KENDI (override edilmemis)
    // print fonksiyonu — INIT_SCRIPT bunu `__bogahostNativePrint` olarak saklar.
    let _ = window.eval(
        "try { (window.__bogahostNativePrint || window.print).call(window); } catch (e) {}",
    );
}

/// Acilis/gecis yukleme katmanini hedef sayfada UYANDIRIR.
///
/// Katmanin KENDISI `initialization_script` ile (document-start) kurulur; bu
/// fonksiyon yalnizca "bu bir uygulama gecisi mi, acilis mi" bilgisini iletir.
/// Cagri, katman henuz kurulmadiysa ya da sayfa çoktan yuklendiyse SESSIZCE
/// bosa duser — bu yuzden birkac kez denenmesi zararsizdir.
fn wake_overlay(app: &AppHandle, mode: &'static str) {
    let handle = app.clone();
    std::thread::spawn(move || {
        for delay in OVERLAY_WAKE_DELAYS_MS.iter() {
            if *delay > 0 {
                std::thread::sleep(Duration::from_millis(*delay));
            }
            let Some(w) = handle.get_webview_window("main") else {
                return;
            };
            let _ = w.eval(format!(
                "try {{ window.__bogahostLoadingFull && window.__bogahostLoadingFull({:?}); }} catch (e) {{}}",
                mode
            ));
        }
    });
}

/// Yukleme katmanini kaldirir (sayfa yuklendi ya da emniyet suresi doldu).
fn hide_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(HIDE_OVERLAY_SCRIPT);
    }
}

/// Pencereyi (bir kez) gorunur yapar.
///
/// AYRI bir splash PENCERESI ARTIK YOKTUR: yukleme ekrani ana pencerenin KENDI
/// webview'inde, hedef sayfaya document-start'ta cizilir. Pencere arka plani
/// koyu (`WINDOW_BG`) oldugu icin katman boyanana kadar da beyaz parlama olmaz.
///
/// Otomatik baslatmada (`--hidden`) pencere GOSTERILMEZ; uygulama tepside
/// sessizce calisir ve bildirim koprusu isler.
fn reveal_window(app: &AppHandle) {
    if LAUNCHED_HIDDEN.load(Ordering::SeqCst) {
        return;
    }
    if WINDOW_REVEALED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Pencereyi her cagrilista gosterir + one getirir.
/// (Dock ikonuna tiklama / tepsiden "Goster" icin — reveal_window tek seferliktir.)
fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Adres uygulamanin kendi alan adinda mi?
/// (`bogahost.com` ve tum alt alan adlari uygulama icinde kalir; WHMCS/oturum
/// yonlendirmeleri de buna dahildir.)
fn is_internal_url(url: &Url) -> bool {
    match url.scheme() {
        "http" | "https" => match url.host_str() {
            Some(host) => {
                let host = host.trim_end_matches('.').to_ascii_lowercase();
                host == INTERNAL_DOMAIN || host.ends_with(&format!(".{INTERNAL_DOMAIN}"))
            }
            None => false,
        },
        _ => false,
    }
}

/// Sayfa koprusunun cagirdigi komut: harici bir adresi SISTEM TARAYICISINDA acar.
/// Kendi alan adimizdaki adresler uygulama icinde acilir (yanlis siniflandirmaya karsi).
#[tauri::command]
fn bogahost_open_external(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|e| e.to_string())?;
    match parsed.scheme() {
        "http" | "https" | "mailto" | "tel" => {}
        _ => return Err("desteklenmeyen adres".to_string()),
    }

    if is_internal_url(&parsed) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.navigate(parsed);
        }
        return Ok(());
    }

    app.shell().open(url, None).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Bildirim koprusu (sayfadaki `window.Notification` shim'i buraya baglanir)
// ---------------------------------------------------------------------------
//
// WebView'de `window.Notification` YOKTUR; paneller bu yuzden "Bu tarayıcı
// bildirimi desteklemiyor" diyordu. Asagidaki 3 komut, `INIT_SCRIPT` icindeki
// `Notification` shim'i tarafindan cagrilir ve NATIVE masaustu bildirimine baglanir.
//
// NOT: Bu, gercek web-push DEGILDIR (WebView'de `PushManager` yoktur, uygulama
// kapaliyken sunucudan bildirim gelmez). Panel acikken uretilen her bildirim
// masaustunde gorunur. Ayrinti: docs/PUSH.md

/// Bildirim izninin su anki durumu — SORMADAN okur (`Notification.permission`).
/// Henuz izin verilmemisse `"default"` doner ki panel "Bildirim aç" dugmesini
/// gostermeye devam etsin ("denied" deseydik panel dugmeyi gizlerdi).
#[tauri::command]
fn bogahost_notify_state(app: AppHandle) -> String {
    if notification_granted(&app) {
        "granted".to_string()
    } else {
        "default".to_string()
    }
}

/// Bildirim iznini ister (`Notification.requestPermission()`).
///
/// ANINDA doner: sistem izin penceresi ARKA PLAN is parcaciginda acilir. Boylece
/// WebView'in JS is parcacigi (ve macOS'ta ana calisma dongusu) BLOKLANMAZ.
/// Sayfa tarafi, sonucu `bogahost_notify_state` ile kisa araliklarla yoklar.
///
/// Izin verilirse kullanici GORSUN diye bir TEST bildirimi gosterilir.
#[tauri::command]
fn bogahost_notify_request(app: AppHandle) -> String {
    if notification_granted(&app) {
        return "granted".to_string();
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let granted = matches!(
            handle.notification().request_permission(),
            Ok(PermissionState::Granted)
        );
        if granted {
            notify(
                &handle,
                APP_TITLE,
                "Bildirimler açıldı. Bu bir test bildirimidir.",
            );
        }
        refresh_notification_menu(&handle);
    });

    "default".to_string()
}

/// Sayfanin olusturdugu bildirimi (`new Notification(...)`) ya da panel
/// beslemesinden yakalanan yeni kaydi masaustunde gosterir.
///
/// `url` verilirse "Son bildirimi ac" tepsi ogesi bu adrese gider (masaustunde
/// bildirimin KENDISINE tiklama olayi yoktur — bkz. `AppState::last_notify_url`).
#[tauri::command]
fn bogahost_notify(
    app: AppHandle,
    title: Option<String>,
    body: Option<String>,
    url: Option<String>,
) -> Result<(), String> {
    remember_notify_url(&app, url.as_deref());

    let raw_title = title.unwrap_or_default();
    let final_title = if raw_title.trim().is_empty() {
        APP_TITLE.to_string()
    } else {
        raw_title
    };

    // Izin henuz yoksa arka planda iste — bildirim sessizce yutulmasin.
    if !notification_granted(&app) {
        let handle = app.clone();
        std::thread::spawn(move || {
            let _ = handle.notification().request_permission();
            refresh_notification_menu(&handle);
        });
    }

    // `notify` gosterimi ana thread'e kuyruklar; bu komut BEKLEMEZ.
    notify(&app, &final_title, &body.unwrap_or_default());
    Ok(())
}

/// "Son bildirimi ac" tepsi ogesinin hedefini gunceller.
///
/// Yalnizca KENDI alan adimizdaki adresler saklanir; sayfa keyfi bir adrese
/// yonlendirme yaptiramaz (bkz. `resolve_internal_url`).
fn remember_notify_url(app: &AppHandle, url: Option<&str>) {
    let Some(target) = url.and_then(resolve_internal_url) else {
        return;
    };
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut slot) = state.last_notify_url.lock() {
            *slot = Some(target);
        }
    }
}

// ---------------------------------------------------------------------------
// Panel bildirim beslemesi -> native bildirim
// ---------------------------------------------------------------------------
//
// MIMARI: saati RUST tutar, istegi SAYFA atar, kalicilik ve gosterim yine
// RUST'tadir.
//
//   `start_notify_clock` (OS is parcacigi, kisilmaz)
//        -> webview.eval("__bogahostFeedTick()")
//             -> sayfa panelin bildirim ucunu `fetch` eder (oturum cerezi,
//                CSRF ve yetki SAYFADA zaten cozulmustur)
//                  -> invoke('bogahost_notify_feed', {items, unread})
//                       -> BURASI: kalici tekillestirme + native bildirim + rozet
//
// Tarayici bildirim/push yiginina (service worker, PushManager, APNs) HICBIR
// bagimlilik YOKTUR. Ayrinti ve sinirlar: docs/PUSH.md

/// Sayfanin normallestirdigi tek besleme kaydi.
#[derive(serde::Deserialize)]
struct FeedItem {
    /// Kayit basina BENZERSIZ anahtar ("feed:123", "msg:45", "conv:9").
    key: String,
    title: Option<String>,
    body: Option<String>,
    url: Option<String>,
    /// Kaydin yasi (saniye) — yalnizca ILK calistirmada eskiyi elemek icin.
    age_s: Option<i64>,
}

/// Sayfadan gelen besleme kayitlarini native bildirime cevirir.
///
/// SESSIZDIR: sayfa tarafi ag hatasi / 401 / 403 durumunda BURAYI HIC cagirmaz
/// (bkz. `EXTRA_SCRIPT`), bu yuzden "bildirim alinamadi" tarzi bir uyari asla
/// cikmaz. Burada da hicbir hata kullaniciya gosterilmez.
#[tauri::command]
fn bogahost_notify_feed(
    app: AppHandle,
    items: Vec<FeedItem>,
    unread: Option<i64>,
) -> Result<(), String> {
    set_badge(&app, unread);

    if items.is_empty() {
        return Ok(());
    }

    // `first_run` = kalici liste HENUZ YOK (ilk kurulum ya da temizlenmis
    // yapilandirma). O turda gecmis besleme TOPLUCA duyurulmaz.
    let (mut seen, first_run) = notify_seen_load(&app);

    let mut fresh: Vec<&FeedItem> = Vec::new();
    for item in &items {
        if item.key.trim().is_empty() || seen.iter().any(|k| k == &item.key) {
            continue;
        }
        // Anahtar, GOSTERILSIN YA DA GOSTERILMESIN isaretlenir: ilk turda
        // elenen eski kayit sonraki turda geri gelmesin.
        seen.push(item.key.clone());
        if first_run && item.age_s.unwrap_or(0) > NOTIFY_FIRST_RUN_MAX_AGE {
            continue;
        }
        fresh.push(item);
    }

    // Halka tampon: en eski anahtarlar dusuruluyor.
    if seen.len() > NOTIFY_SEEN_MAX {
        seen.drain(..seen.len() - NOTIFY_SEEN_MAX);
    }
    notify_seen_save(&app, &seen);

    if fresh.is_empty() {
        return Ok(());
    }

    // Cok birikmisse masaustunu doldurma: tek ozet + en yeni birkac tanesi.
    if fresh.len() > NOTIFY_BURST_MAX {
        let total = fresh.len();
        notify(&app, APP_TITLE, &format!("{total} yeni bildirim var."));
        fresh = fresh.split_off(total - NOTIFY_BURST_MAX);
    }

    for item in fresh {
        // Bildirimin KENDISINE tiklama olayi masaustunde YOKTUR (bkz.
        // `AppState::last_notify_url`); hedef adres tepsideki "Son bildirimi
        // aç" ogesine baglanir.
        remember_notify_url(&app, item.url.as_deref());

        let title = item
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .unwrap_or(APP_TITLE);
        notify(&app, title, item.body.as_deref().unwrap_or_default());
    }

    Ok(())
}

/// Okunmamis sayisini Dock / gorev cubugu rozetinde gosterir (0 ise kaldirir).
///
/// `set_badge_count` macOS ve Linux'ta calisir; Windows'ta desteklenmez ve
/// hata SESSIZCE yutulur (rozet olmamasi bir arıza degildir).
fn set_badge(app: &AppHandle, unread: Option<i64>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let count = match unread {
        Some(n) if n > 0 => Some(n),
        _ => None,
    };
    let _ = window.set_badge_count(count);
}

fn notify_state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(NOTIFY_STATE_FILE))
}

/// Kalici "gosterildi" listesini okur.
///
/// Ikinci deger ILK CALISTIRMA bayragidir (dosya yok ya da okunamadi) — cagiran
/// o turda gecmis kayitlari duyurmaz.
fn notify_seen_load(app: &AppHandle) -> (Vec<String>, bool) {
    let Some(path) = notify_state_path(app) else {
        return (Vec::new(), true);
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (Vec::new(), true);
    };
    let keys = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|value| value.get("keys")?.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    (keys, false)
}

fn notify_seen_save(app: &AppHandle, keys: &[String]) {
    let Some(path) = notify_state_path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    if let Ok(text) = serde_json::to_string(&serde_json::json!({ "keys": keys })) {
        let _ = std::fs::write(&path, text);
    }
}

/// Bildirim yoklamasinin SAATI.
///
/// Isletim sistemi is parcacigidir; WebView'in arka plan kisitlamalarindan
/// ETKILENMEZ. Her turda sayfadaki `__bogahostFeedTick` ACIKCA calistirilir —
/// bu bir zamanlayici degil, dogrudan calistirmadir; pencere GIZLIYKEN de
/// aninda kosar.
fn start_notify_clock(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(NOTIFY_POLL_INTERVAL);

        // Guncelleme kuruluyorsa / yeniden baslatiliyorsa karisma.
        if RESTART_IN_PROGRESS.load(Ordering::SeqCst) || UPDATE_INSTALLING.load(Ordering::SeqCst) {
            continue;
        }
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.eval(
                "try { window.__bogahostFeedTick && window.__bogahostFeedTick(); } catch (e) {}",
            );
        }
    });
}

// ---------------------------------------------------------------------------
// Sistem ayarlari / pencere komutlari (medya izni geri bildirimi icin)
// ---------------------------------------------------------------------------

/// Ilgili sistem gizlilik/bildirim ayarini acar.
///
/// Kamera veya mikrofon izni REDDEDILDIGINDE sayfa tarafi kullaniciya bir
/// dugme gosterir; dugme buraya baglanir. Boylece "izin verin" denip
/// kullanicinin nereye gidecegini bilememesi ONLENIR.
///
/// Bilinmeyen `kind` degerleri SESSIZCE yok sayilir (sayfadan gelen veriye
/// guvenilmez; asagidaki liste kapali bir beyaz listedir).
#[tauri::command]
fn bogahost_open_settings(app: AppHandle, kind: String) -> Result<(), String> {
    let Some(target) = settings_url(&kind) else {
        return Ok(());
    };
    app.shell()
        .open(target.to_string(), None)
        .map_err(|e| e.to_string())
}

/// `bogahost_open_settings` icin platforma gore ayar adresi.
/// Beyaz liste disindaki her deger `None` doner.
///
/// NOT: platform ayrimi BLOK degil OGE (item) duzeyinde yapilir. Fonksiyon
/// govdesinin sonunda duran `#[cfg] { ... }` bloklari tail-expression
/// konumunda kalir ve kararsiz (`stmt_expr_attributes`) ozellik isteyebilir;
/// oge uzerindeki `#[cfg]` her zaman kararlidir.
#[cfg(target_os = "macos")]
fn settings_url(kind: &str) -> Option<&'static str> {
    match kind {
        // Panel adlari macOS 13+ (Ventura) Sistem Ayarlari'nda da gecerlidir.
        "camera" => Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Camera"),
        "microphone" => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        }
        "screen" => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        }
        "notifications" => Some("x-apple.systempreferences:com.apple.preference.notifications"),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn settings_url(kind: &str) -> Option<&'static str> {
    match kind {
        // NOT: Windows ayar anahtari "webcam"dir, arayuzde "Kamera" yazar.
        "camera" => Some("ms-settings:privacy-webcam"),
        "microphone" => Some("ms-settings:privacy-microphone"),
        // Windows'ta ekran paylasimi icin ayri bir gizlilik anahtari YOKTUR.
        "screen" => Some("ms-settings:privacy"),
        "notifications" => Some("ms-settings:notifications"),
        _ => None,
    }
}

/// Linux/diger: ortak bir ayar adresi YOKTUR — sayfa tarafi dugmeyi gizler.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn settings_url(_kind: &str) -> Option<&'static str> {
    None
}

/// Ana pencereyi one getirir (bildirim/arama geldiginde).
#[tauri::command]
fn bogahost_focus_window(app: AppHandle) -> Result<(), String> {
    show_main_window(&app);
    Ok(())
}

/// Pencereyi tam ekrana alir/cikarir.
///
/// NEDEN: macOS'ta HTML `element.requestFullscreen()` CALISMAZ — wry ilgili
/// WKPreferences anahtarini yalnizca `fullscreen` ozelligi (Tauri'de
/// `macos-private-api`) acikken kurar, o da OZEL (private) API oldugu icin
/// acilmadi. Sayfa tarafi tam ekran istegi basarisiz olursa bu komuta duser ve
/// PENCEREYI tam ekran yapar — kullanici icin sonuc buyuk olcude aynidir.
#[tauri::command]
fn bogahost_set_fullscreen(window: tauri::WebviewWindow<Wry>, on: bool) -> Result<(), String> {
    window.set_fullscreen(on).map_err(|e| e.to_string())
}

/// Sayfadan gelen adresi mutlak hale getirir ve IC adres degilse `None` doner.
///
/// Besleme kayitlari cogunlukla goreli adres verir ("/admin/chats?c=12"); bunlar
/// bu uygulamanin kendi kok adresine gore cozulur. Sonuc `bogahost.com` alan
/// adinda DEGILSE reddedilir — panel, kabugu yabanci bir adrese goturemez.
fn resolve_internal_url(raw: &str) -> Option<String> {
    let base = APPS.iter().find(|e| e.0 == APP_KEY).map(|e| e.2)?;
    let absolute = match Url::parse(raw) {
        Ok(u) => u,
        Err(_) => Url::parse(base).ok()?.join(raw).ok()?,
    };
    if is_internal_url(&absolute) {
        Some(absolute.to_string())
    } else {
        None
    }
}

/// Verilen IC adresi ANA pencerede acar ve pencereyi one getirir.
fn open_in_main_window(app: &AppHandle, raw: &str) {
    let Ok(url) = Url::parse(raw) else {
        return;
    };
    if !is_internal_url(&url) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.navigate(url);
    }
    show_main_window(app);
}

// ---------------------------------------------------------------------------
// Sayfa tarafi kopru (initialization script)
// ---------------------------------------------------------------------------

/// Sayfaya (hem uzak panel hem yerel splash) enjekte edilen surum bilgisi.
/// `{:?}` bicimlendirmesi tirnaklari/kacislari kendisi ekler — gecerli bir JS
/// dize sabiti uretir.
fn version_script() -> String {
    format!(
        "window.__BOGAHOST_NATIVE_VERSION__ = {:?};\nwindow.__BOGAHOST_APP_TITLE__ = {:?};\nwindow.__BOGAHOST_APP_KEY__ = {:?};\n",
        env!("CARGO_PKG_VERSION"),
        APP_TITLE,
        APP_KEY
    )
}

/// Ana pencereye enjekte edilen tam betik: surum bilgisi + sayfa koprusu.
fn init_script() -> String {
    let mut script = version_script();
    script.push_str(INIT_SCRIPT);
    // Medya izinleri, panel bildirim beslemesi, pano, ses kilidi ve tam ekran.
    // AYRI bir IIFE'dir; INIT_SCRIPT bozulsa bile bagimsiz calisir.
    script.push_str(EXTRA_SCRIPT);
    script
}

/// ANA pencereye enjekte edilen betik: kopru + acilis/gecis yukleme katmani.
///
/// Katman YALNIZCA ana pencereye eklenir; onizleme (popup) pencerelerinde
/// yukleme ekrani istenmez.
fn main_init_script() -> String {
    let mut script = init_script();
    script.push_str(&loading_overlay_script());
    // Guncelleme seridi + "mesgulum" tespiti — YALNIZCA ana pencerede.
    script.push_str(UPDATE_UI_JS);
    script
}

/// Onizleme (popup) penceresine enjekte edilen betik.
/// Ana pencereyle AYNI kopruye ek olarak "kapatma" yollarini ekler.
fn popup_init_script() -> String {
    let mut script = init_script();
    script.push_str(POPUP_INIT_SCRIPT);
    script
}

/// Her sayfa yuklemesinde WebView'e enjekte edilir:
///  * `target="_blank"` linkleri sessizce yutulmaz (ayni pencerede acilir;
///    harici ise Rust tarafi sistem tarayicisina yollar),
///  * `blob:` / `data:` indirmeleri (PDF/CSV uretimi) native olarak diske yazilir,
///  * `window.open` bosa dusmez.
const INIT_SCRIPT: &str = r#"
(function () {
  if (window.__BOGAHOST_NATIVE__) { return; }
  window.__BOGAHOST_NATIVE__ = true;

  function invoke(cmd, args) {
    try {
      var t = window.__TAURI__;
      if (t && t.core && typeof t.core.invoke === 'function') { return t.core.invoke(cmd, args); }
      if (t && typeof t.invoke === 'function') { return t.invoke(cmd, args); }
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
        return window.__TAURI_INTERNALS__.invoke(cmd, args);
      }
    } catch (e) {}
    return Promise.reject(new Error('ipc-yok'));
  }

  function toBase64(buffer) {
    var bytes = new Uint8Array(buffer);
    var chunk = 0x8000;
    var parts = [];
    for (var i = 0; i < bytes.length; i += chunk) {
      parts.push(String.fromCharCode.apply(null, bytes.subarray(i, i + chunk)));
    }
    return btoa(parts.join(''));
  }

  function isInternal(host) {
    var h = String(host || '').toLowerCase();
    return h === 'bogahost.com' || h.slice(-13) === '.bogahost.com';
  }

  // Panel-ici "Uygulamalar" menusundeki KARDES uygulama adresleri (dcim/finans/
  // chat/task alt alan adlari) — su anki uygulamanin KENDI adresi HARIC. Bu
  // linkler target="_blank" tasidigi icin asagidaki "yeni sekme -> onizleme
  // penceresi" mantigina duserse hedef panel kucuk bir onizleme penceresinde
  // acilir ve gecis splash'i HIC gorunmez. Kardes panele YERINDE gecilmesi icin
  // ayrica taninir (bkz. tiklama isleyicisi).
  function isSiblingApp(u) {
    try {
      var h = String((u && u.hostname) || '').toLowerCase();
      var cur = String(location.hostname || '').toLowerCase();
      if (!h || h === cur) { return false; }
      return h === 'dcim.bogahost.com' || h === 'finans.bogahost.com'
          || h === 'chat.bogahost.com' || h === 'task.bogahost.com';
    } catch (e) { return false; }
  }

  function openExternal(href) {
    return invoke('bogahost_open_external', { url: href });
  }

  // Yeni sekmede acilmak istenen IC adresler AYRI, KAPATILABILIR bir pencerede
  // acilir. Eskiden ANA pencere oraya gidiyordu; sunucu PDF/gorsel dondurunce
  // panel kayboluyor ve kullanici geri donemiyordu.
  function openPopup(href) {
    return invoke('bogahost_open_popup', { url: href });
  }

  // Panelin yazdirma dugmeleri icin: sayfanin GERCEK print fonksiyonu saklanir
  // (menudeki "Yazdır…" bunu cagirir; bkz. Rust `trigger_print`).
  try { window.__bogahostNativePrint = window.print; } catch (e) {}

  // `window.print()` KOPRUSU.
  // macOS (WKWebView) JS `window.print()` cagrisini SESSIZCE YOK SAYAR —
  // panellerdeki "Yazdır" dugmeleri orada HICBIR SEY yapmiyordu. Cagriyi
  // native yazdirma akisina yolluyoruz: `trigger_print` macOS'ta
  // `WebviewWindow::print()` (native diyalog), diger platformlarda ise
  // asagida sakladigimiz ORIJINAL fonksiyonu kullanir — yani cift diyalog
  // acilmaz, sonsuz dongu olusmaz.
  try {
    window.print = function () {
      // `invoke` HICBIR ZAMAN throw etmez; kopru yoksa REDDEDEN promise doner.
      // Bu yuzden yedek yol `catch` zincirindedir.
      invoke('bogahost_print', {}).catch(function () {
        try { window.__bogahostNativePrint.call(window); } catch (e) {}
      });
    };
  } catch (e) {}

  function guessName(u, fallback) {
    try {
      var path = (u.pathname || '').split('/').filter(Boolean);
      var last = path.length ? decodeURIComponent(path[path.length - 1]) : '';
      if (last && last.indexOf('.') > 0) { return last; }
    } catch (e) {}
    return fallback || 'indirilen-dosya';
  }

  function saveLocal(href, name, openAfter) {
    return fetch(href)
      .then(function (r) { return r.blob(); })
      .then(function (blob) {
        var finalName = name;
        if (!finalName || finalName.indexOf('.') < 0) {
          var ext = '';
          if (blob.type === 'application/pdf') { ext = '.pdf'; }
          else if (blob.type.indexOf('csv') >= 0) { ext = '.csv'; }
          else if (blob.type.indexOf('zip') >= 0) { ext = '.zip'; }
          else if (blob.type.indexOf('excel') >= 0 || blob.type.indexOf('sheet') >= 0) { ext = '.xlsx'; }
          else if (blob.type.indexOf('json') >= 0) { ext = '.json'; }
          else if (blob.type.indexOf('png') >= 0) { ext = '.png'; }
          else if (blob.type.indexOf('jpeg') >= 0) { ext = '.jpg'; }
          finalName = (finalName || 'indirilen-dosya') + ext;
        }
        return blob.arrayBuffer().then(function (buf) {
          return invoke('bogahost_save_file', {
            name: finalName,
            b64: toBase64(buf),
            openAfter: !!openAfter
          });
        });
      });
  }

  // ---- Kisa bilgi mesaji (sessiz hata yerine) ----
  // `actionLabel` + `actionFn` verilirse mesajin sagina tiklanabilir bir
  // baglanti eklenir (ornegin "Klasörde göster") ve mesaj daha uzun durur.
  function toast(msg, actionLabel, actionFn) {
    try {
      var id = 'bogahost-native-toast';
      var old = document.getElementById(id);
      if (old && old.parentNode) { old.parentNode.removeChild(old); }
      var d = document.createElement('div');
      d.id = id;
      d.setAttribute('style', 'position:fixed;left:50%;top:18px;transform:translateX(-50%);z-index:2147483647;max-width:80vw;padding:11px 18px;border-radius:10px;background:#22242c;color:#e6e8ee;border:1px solid rgba(255,255,255,.12);box-shadow:0 8px 28px rgba(0,0,0,.35);font:13px/1.5 -apple-system,"Segoe UI",Roboto,Arial,sans-serif;display:flex;align-items:center;gap:14px;');
      var span = document.createElement('span');
      span.textContent = msg;
      d.appendChild(span);
      var life = 6000;
      if (actionLabel && typeof actionFn === 'function') {
        var b = document.createElement('button');
        b.type = 'button';
        b.textContent = actionLabel;
        b.setAttribute('style', 'flex:0 0 auto;cursor:pointer;background:rgba(255,255,255,.10);color:#cfe3ff;border:1px solid rgba(255,255,255,.18);border-radius:7px;padding:5px 11px;font:inherit;');
        b.onclick = function () {
          try { actionFn(); } catch (e) {}
          try { if (d.parentNode) { d.parentNode.removeChild(d); } } catch (e2) {}
        };
        d.appendChild(b);
        life = 12000;
      }
      (document.body || document.documentElement).appendChild(d);
      setTimeout(function () { try { if (d.parentNode) { d.parentNode.removeChild(d); } } catch (e) {} }, life);
    } catch (e) {}
  }

  // Rust tarafi indirme sonucunu buradan bildirir (bkz. `page_toast`).
  try { window.__bogahostToast = toast; } catch (e) {}

  // ---- "‹ Panele dön" emniyet seridi ----
  // Ust duzey gezinme bir BELGEYE ( or. HTML yazdirma onizlemesi) gidip paneli
  // kapatirsa kullanici KILITLI kalmasin: sabit, en ust katmanda, kapatilamayan
  // bir "Panele dön" seridi cizilir. Rust emniyet agi (`return_to_panel`) bunu
  // `__bogahostShowBackStrip` ile tetikler; ayrica bir belge adresi UST DUZEY
  // yuklenirse (kopru atlanmis olabilir) sayfa kendisi de gosterir.
  //
  // NOT: macOS/WKWebView native PDF goruntuleyicisinde HTML/DOM YOKTUR; serit
  // orada CIZILEMEZ. O senaryonun garantisi Rust `navigate` ile paneli geri
  // yuklemektir (bkz. `return_to_panel`).
  var BACK_STRIP_ID = 'bogahost-native-back-strip';
  function panelHomeHref() {
    try { return location.origin + '/'; } catch (e) { return '/'; }
  }
  function goBackToPanel() {
    try { if (window.history && history.length > 1) { history.back(); return; } } catch (e) {}
    try { location.href = panelHomeHref(); } catch (e2) {}
  }
  function showBackStrip() {
    try {
      if (document.getElementById(BACK_STRIP_ID)) { return; }
      var bar = document.createElement('div');
      bar.id = BACK_STRIP_ID;
      bar.setAttribute('style', 'position:fixed;left:0;right:0;top:0;z-index:2147483647;min-height:44px;display:flex;align-items:center;gap:12px;padding:8px 14px;background:#14161d;border-bottom:1px solid rgba(255,255,255,.14);box-shadow:0 4px 16px rgba(0,0,0,.4);font:14px/1.3 -apple-system,"Segoe UI",Roboto,Arial,sans-serif;');
      var btn = document.createElement('button');
      btn.type = 'button';
      btn.textContent = '‹ Panele dön';
      btn.setAttribute('style', 'flex:0 0 auto;cursor:pointer;background:#5443D2;color:#fff;border:0;border-radius:8px;padding:8px 16px;font:600 13px -apple-system,"Segoe UI",Roboto,Arial,sans-serif;');
      btn.onclick = goBackToPanel;
      var label = document.createElement('span');
      label.textContent = 'Belge görünümü — panele dönmek için soldaki düğmeyi kullanın.';
      label.setAttribute('style', 'color:#aeb4c2;');
      bar.appendChild(btn);
      bar.appendChild(label);
      (document.body || document.documentElement).appendChild(bar);
      try { document.documentElement.style.scrollPaddingTop = '52px'; } catch (e) {}
    } catch (e) {}
  }
  try { window.__bogahostShowBackStrip = showBackStrip; } catch (e) {}

  // Indirme BASARILI bilgisi (bkz. Rust `notify_download_saved`):
  // dosya adi + kaydedildigi klasor + tek tikla "Klasörde göster".
  try {
    window.__bogahostDownloadDone = function (name, folder) {
      toast(name + ' → İndirilenler klasörüne kaydedildi', 'Klasörde göster', function () {
        invoke('bogahost_reveal_download', {}).catch(function () {});
      });
      try { console.log('[bogahost] indirildi:', folder); } catch (e) {}
    };
  } catch (e) {}

  // ---- window.Notification koprusu ----
  // WebView `Notification` SUNMAZ; paneller "Bu tarayıcı bildirimi desteklemiyor"
  // diyordu. Asagidaki shim, standart Notification API'sini native masaustu
  // bildirimine baglar. GERCEK WEB-PUSH DEGILDIR: `PushManager` yoktur, bu yuzden
  // panel kendi fallback'ine (yoklama/SSE) duser — bu kasitlidir.
  if (!window.Notification) {
    var BogahostNotification = function (title, options) {
      options = options || {};
      this.title = title;
      this.body = options.body || '';
      this.tag = options.tag || '';
      this.data = options.data;
      this.onclick = null;
      this.onshow = null;
      this.onerror = null;
      this.onclose = null;
      var self = this;
      // Panelin KENDI `new Notification()` cagrisi ile besleme dinleyicisi AYNI
      // tekillestirmeden gecer (EXTRA_SCRIPT) — ayni bildirim iki kez cikmaz.
      // `tag` varsa anahtar odur; yoksa baslik+govde ozeti kullanilir.
      var dedupKey = 'shim:' + (this.tag || (String(title) + '|' + String(this.body)));
      var targetUrl = null;
      try { targetUrl = (options.data && options.data.url) || options.url || null; } catch (eU) {}

      if (typeof window.__bogahostNotifyOnce === 'function') {
        try {
          window.__bogahostNotifyOnce(dedupKey, String(title == null ? '' : title), String(this.body), targetUrl);
          if (typeof self.onshow === 'function') { self.onshow(); }
        } catch (eN) {}
        return;
      }

      invoke('bogahost_notify', { title: String(title == null ? '' : title), body: String(this.body), url: targetUrl })
        .then(function () {
          try { if (typeof self.onshow === 'function') { self.onshow(); } } catch (e) {}
        })
        .catch(function () {
          try { if (typeof self.onerror === 'function') { self.onerror(); } } catch (e) {}
        });
    };
    BogahostNotification.prototype.close = function () {
      try { if (typeof this.onclose === 'function') { this.onclose(); } } catch (e) {}
    };
    BogahostNotification.prototype.addEventListener = function (type, fn) {
      if (type && typeof fn === 'function') { this['on' + type] = fn; }
    };
    BogahostNotification.prototype.removeEventListener = function (type) {
      if (type) { this['on' + type] = null; }
    };
    BogahostNotification.prototype.dispatchEvent = function () { return true; };

    BogahostNotification.permission = 'default';
    BogahostNotification.maxActions = 0;

    // Gercek durumu (SORMADAN) oku — panel dogru dugmeyi gostersin.
    invoke('bogahost_notify_state', {})
      .then(function (state) { BogahostNotification.permission = (state === 'granted') ? 'granted' : 'default'; })
      .catch(function () {});

    // `requestPermission()` hem Promise hem callback bicimini destekler.
    // Native taraf ANINDA doner (izin penceresi arka planda acilir); bu yuzden
    // sonucu kisa araliklarla yokluyoruz.
    BogahostNotification.requestPermission = function (callback) {
      function finish(value) {
        BogahostNotification.permission = value;
        try { if (typeof callback === 'function') { callback(value); } } catch (e) {}
        return value;
      }
      return invoke('bogahost_notify_request', {}).then(function (immediate) {
        if (immediate === 'granted') { return finish('granted'); }
        return new Promise(function (resolve) {
          var tries = 0;
          var timer = setInterval(function () {
            tries++;
            invoke('bogahost_notify_state', {})
              .then(function (state) {
                if (state === 'granted') {
                  clearInterval(timer);
                  resolve(finish('granted'));
                } else if (tries >= 30) {
                  // ~12 sn icinde onaylanmadi: panel "kapalı" gosterebilsin.
                  clearInterval(timer);
                  resolve(finish('denied'));
                }
              })
              .catch(function () {
                clearInterval(timer);
                resolve(finish('denied'));
              });
          }, 400);
        });
      }).catch(function () { return finish('denied'); });
    };

    try { window.Notification = BogahostNotification; } catch (e) {}
  }

  // Panellerin "native kabuk icindeyiz" ayrimini yapabilmesi icin isaret.
  // (Web-push kurulumunu atlayip dogrudan Notification'a duserler.)
  try {
    window.__BOGAHOST_NATIVE_NOTIFY__ = true;
    window.__bogahostNotify = function (title, body) {
      return invoke('bogahost_notify', { title: String(title || ''), body: String(body || ''), url: null });
    };
  } catch (e) {}

  // ---- Sunucu tarafli indirmeler (PDF / CSV / XLSX ...) ----
  // Bu bolum OLMADAN: `target="_blank"` tasiyan indirme linkleri ve POST form'lari
  // ana pencereyi indirme adresine GOTURUYOR, sayfa 404/hata ekranina dusuyordu.
  // Artik dosya `fetch` ile (oturum cerezleriyle) alinip `bogahost_save_file`
  // koprusu uzerinden diske yazilir; panel sayfasi YERINDE KALIR.
  var DOWNLOAD_EXT = /\.(pdf|csv|xlsx?|docx?|pptx?|zip|rar|7z|gz|tgz|tar|txt|json|xml|ics|sql|log|bak)$/i;
  var DOWNLOAD_PATH = /(^|\/)(pdf|csv|excel|xls|xlsx|export|download|indir|rapor|fatura)(\/|$)/i;
  var DOWNLOAD_QUERY = /[?&](format|export|download|output|type)=(pdf|csv|xlsx?|excel)(&|$)/i;
  // "İndir" niyeti tasiyan bayraklar (deger onemsiz). Uygulamadaki "İndir"
  // dugmeleri `?indir=1` ekler; UZANTISIZ uclarda (Parasut fatura / e-fatura
  // PDF'leri) TEK guvenilir sinyal budur. Bu olmadan link ana pencereyi belgeye
  // goturur, WKWebView PDF'i gomulu acar ve kullanici panele DONEMEZ.
  var DOWNLOAD_FLAG = /[?&](indir|download|dl|export)(=|&|$)/i;

  // Base64'e cevrilirken bellekte ~4/3 kat yer kaplar; buyuk dosyalarda
  // WebView'in KENDI indirme akisina (on_download) birakiriz.
  var MAX_BRIDGE_BYTES = 48 * 1024 * 1024;

  function looksLikeDownload(u, el) {
    try {
      if (el && el.hasAttribute && el.hasAttribute('download')) { return true; }
      var path = String(u.pathname || '');
      if (DOWNLOAD_EXT.test(path)) { return true; }
      if (DOWNLOAD_PATH.test(path)) { return true; }
      if (DOWNLOAD_QUERY.test(String(u.search || ''))) { return true; }
      if (DOWNLOAD_FLAG.test(String(u.search || ''))) { return true; }
    } catch (e) {}
    return false;
  }

  // DUZ linkler (target/`download` YOK) icin KESIN sinyal: gercek bir dosya
  // uzantisi ya da acik bir disa-aktarma parametresi.
  //
  // `looksLikeDownload`'daki yol-sozcugu eslesmesi (`/fatura/`, `/rapor/` ...)
  // burada KASITLI OLARAK kullanilmaz: duz linkte turu ogrenmek icin ek bir
  // GET atilir ve o adresler cogu zaman siradan HTML sayfalaridir — gereksiz
  // ikinci istek (ve varsa denetim kaydi) atilmasin diye kapsam dar tutulur.
  // O sozcukler zaten `target="_blank"` / `download` tasiyan baglantilarda
  // devrededir.
  function looksLikeFile(u) {
    try {
      if (DOWNLOAD_EXT.test(String(u.pathname || ''))) { return true; }
      if (DOWNLOAD_QUERY.test(String(u.search || ''))) { return true; }
      // `?indir=1` gibi bayraklar: uzantisiz "İndir" uclarinin (Parasut PDF)
      // TEK sinyali. Bu olmadan duz link ana pencereyi belgeye goturur.
      if (DOWNLOAD_FLAG.test(String(u.search || ''))) { return true; }
    } catch (e) {}
    return false;
  }

  // Content-Disposition basligindaki gercek dosya adini kullan (varsa).
  function stripQuotes(v) {
    return String(v == null ? '' : v).split('"').join('').split("'").join('').trim();
  }

  function nameFromResponse(res, u, fallback) {
    try {
      var cd = res.headers.get('content-disposition') || '';
      var m = /filename\*=\s*UTF-8''([^;]+)/i.exec(cd);
      if (m && m[1]) {
        try { return decodeURIComponent(stripQuotes(m[1])); } catch (e1) { return stripQuotes(m[1]); }
      }
      m = /filename\s*=\s*([^;]+)/i.exec(cd);
      if (m && m[1]) { return stripQuotes(m[1]); }
    } catch (e) {}
    return fallback || guessName(u, null);
  }

  function saveResponse(res, u, fallback, openAfter) {
    var len = 0;
    try { len = parseInt(res.headers.get('content-length') || '0', 10) || 0; } catch (e) {}
    if (len > MAX_BRIDGE_BYTES) { return Promise.reject(new Error('cok-buyuk')); }

    return res.blob().then(function (blob) {
      if (blob.size > MAX_BRIDGE_BYTES) { throw new Error('cok-buyuk'); }
      var name = nameFromResponse(res, u, fallback);
      if (!name || name.indexOf('.') < 0) {
        var ext = '';
        var t = blob.type || '';
        if (t.indexOf('pdf') >= 0) { ext = '.pdf'; }
        else if (t.indexOf('csv') >= 0) { ext = '.csv'; }
        else if (t.indexOf('zip') >= 0) { ext = '.zip'; }
        else if (t.indexOf('excel') >= 0 || t.indexOf('sheet') >= 0) { ext = '.xlsx'; }
        else if (t.indexOf('json') >= 0) { ext = '.json'; }
        name = (name || 'indirilen-dosya') + ext;
      }
      return blob.arrayBuffer().then(function (buf) {
        return invoke('bogahost_save_file', {
          name: name,
          b64: toBase64(buf),
          openAfter: !!openAfter
        });
      });
    });
  }

  // Sunucudan indirir ve diske yazar. Hata olursa REDDEDER — cagiran taraf ya
  // WebView'in kendi akisina duser ya da kullaniciya anlasilir mesaj gosterir.
  function downloadViaBridge(u, fallbackName, openAfter) {
    return fetch(u.href, { credentials: 'include' }).then(function (res) {
      if (!res.ok) {
        var err = new Error('http-' + res.status);
        err.status = res.status;
        throw err;
      }
      return saveResponse(res, u, fallbackName, openAfter);
    });
  }

  // Yanit bir BELGE mi (PDF/CSV/XLSX/ZIP ...), yoksa gercek bir HTML sayfasi mi?
  //
  // Sunucular indirme amacli dosyalari HER ZAMAN
  // `Content-Disposition: attachment` ile gondermez — Paraşüt fatura PDF'leri
  // gibi bircok uc nokta `inline` kullanir. `inline` gelen bir PDF'i WebView
  // GEZINME sayar: ana pencere belgeye gider ve kullanici panele DONEMEZ.
  // Bu yuzden karar `Content-Type`'a gore verilir, `Content-Disposition`'a degil.
  var DOC_TYPE = /(pdf|csv|excel|spreadsheet|officedocument|msword|zip|rar|7z-compressed|x-tar|gzip|octet-stream)/i;

  function isDocumentResponse(res) {
    try {
      if (/attachment/i.test(String(res.headers.get('content-disposition') || ''))) { return true; }
      var ct = String(res.headers.get('content-type') || '').toLowerCase();
      if (!ct) { return false; }
      // HTML her zaman gezinmedir (hata sayfalari, yazdirma onizlemeleri ...).
      if (ct.indexOf('text/html') >= 0 || ct.indexOf('xhtml') >= 0) { return false; }
      return DOC_TYPE.test(ct);
    } catch (e) { return false; }
  }

  // YAVAS/BUYUK (dis API arkasi) indirme uclari: Parasut e-belge PDF gibi.
  // Bunlar WKWebView `fetch`->blob ile GUVENILMEZ indirilir (yavas yanitta
  // baglanti kopar, fetch reddeder). KESIN olarak Rust yolundan gecerler.
  function isHeavyDoc(u) {
    try {
      var p = String(u.pathname || '').toLowerCase();
      return p.indexOf('/parasut/') >= 0
          || p.indexOf('/e-fatura-pdf') >= 0
          || p.indexOf('/e-arsiv-pdf') >= 0
          || p.indexOf('/e-belge') >= 0;
    } catch (e) { return false; }
  }

  // Rust tarafi indirme (uzun timeout + diske streaming + webview oturum cerezi
  // + ayni User-Agent). Basarisizlikta Rust Err(String) kodu -> anlasilir hata.
  function downloadViaRust(u, fallbackName, openAfter, allowNavigate) {
    return invoke('bogahost_fetch_download', {
      url: u.href,
      name: fallbackName || null,
      ua: (function () { try { return navigator.userAgent || null; } catch (e) { return null; } })(),
      cookie: (function () { try { return document.cookie || null; } catch (e) { return null; } })(),
      openAfter: !!openAfter,
      allowNavigate: !!allowNavigate
    }).catch(function (code) {
      var s = String(code == null ? 'network' : code);
      var e = new Error(s);
      if (s.indexOf('status:') === 0) { e.status = parseInt(s.slice(7), 10) || 0; }
      else { e.code = s; }
      throw e;
    });
  }

  // Indirme niyetli bir adres icin DOGRU davranisi secer:
  //   * Yavas/buyuk uc (Parasut) -> Rust yolu (uzun timeout + streaming).
  //   * `download` niteligi varsa -> dogrudan diske yaz (turu sormaya gerek yok).
  //   * Yanit belge ise           -> diske yaz + sistem uygulamasinda ac; panel YERINDE KALIR.
  //   * Yanit HTML ise            -> yeni sekme istendiyse kapatilabilir onizleme
  //                                  penceresi, aksi halde normal gezinme.
  function handleMaybeDownload(u, dlAttr, newTab) {
    if (isHeavyDoc(u)) {
      var save = (dlAttr !== null && dlAttr !== undefined);
      // save ise dosyayi yalnizca kaydet; degilse belgeyi indir + sistem
      // uygulamasinda ac. HTML donerse (allowNavigate) gezin/onizle.
      return downloadViaRust(u, save ? dlAttr : null, !save, true)
        .then(function (res) {
          if (res === '__NAVIGATE__') {
            if (newTab) { return openPopup(u.href); }
            try { location.href = u.href; } catch (e) {}
          }
          return null;
        })
        // Yavas/buyuk uclarda WebView'in kendi akisina DUSME (panel kaybolur);
        // hatayi dogrudan bildir.
        .catch(function (err) { reportDownloadError(err); return null; });
    }
    if (dlAttr !== null && dlAttr !== undefined) {
      return downloadViaBridge(u, dlAttr, false);
    }
    return fetch(u.href, { credentials: 'include' }).then(function (res) {
      if (!res.ok) {
        var err = new Error('http-' + res.status);
        err.status = res.status;
        throw err;
      }
      if (isDocumentResponse(res)) { return saveResponse(res, u, null, true); }
      if (newTab) { return openPopup(u.href); }
      try { location.href = u.href; } catch (e) {}
      return null;
    });
  }

  // Rust emniyet agi (bkz. `guard_document_navigation`) buradan cagirir:
  // pencere bir belgeye gidip panele geri dondurulduyse dosya yine de insin.
  try {
    window.__bogahostDownload = function (href) {
      var u;
      try { u = new URL(String(href), location.href); } catch (e) { return; }
      handleMaybeDownload(u, null, false).catch(function (err) { reportDownloadError(err); });
    };
  } catch (e) {}

  // Indirme hatasini kullaniciya ANLASILIR bicimde bildirir (sessiz 404 yerine).
  function reportDownloadError(err) {
    var status = err && err.status;
    var code = err && err.code;
    var msg;
    if (status === 404) {
      msg = 'Dosya bulunamadı (404). E-belge/rapor sunucuda henüz oluşturulmamış olabilir.';
    } else if (status === 403 || status === 401) {
      msg = 'Bu dosyayı indirme izniniz yok.';
    } else if (status) {
      msg = 'Dosya indirilemedi (sunucu hatası ' + status + ').';
    } else if (code === 'timeout') {
      msg = 'İndirme zaman aşımına uğradı, lütfen tekrar deneyin.';
    } else if (code === 'empty') {
      msg = 'Dosya boş geldi (sunucu içerik döndürmedi).';
    } else {
      msg = 'Dosya indirilemedi. Bağlantınızı denetleyip yeniden deneyin.';
    }
    toast(msg);
    try { invoke('bogahost_notify', { title: 'İndirme başarısız', body: msg, url: null }); } catch (e) {}
  }

  document.addEventListener('click', function (ev) {
    if (ev.defaultPrevented || ev.button !== 0 || ev.metaKey || ev.ctrlKey) { return; }
    var el = ev.target;
    var a = null;
    while (el && el !== document) {
      if (el.tagName && el.tagName.toLowerCase() === 'a' && el.getAttribute('href')) { a = el; break; }
      el = el.parentNode;
    }
    if (!a) { return; }
    if (a.__bogahostSkip) { a.__bogahostSkip = false; return; }
    var raw = a.getAttribute('href') || '';
    if (!raw || raw.charAt(0) === '#' || raw.toLowerCase().indexOf('javascript:') === 0) { return; }
    var abs;
    try { abs = new URL(a.href, location.href); } catch (e) { return; }

    if (abs.protocol === 'blob:' || abs.protocol === 'data:') {
      ev.preventDefault();
      var dl = a.getAttribute('download');
      saveLocal(abs.href, dl || guessName(abs, null), !dl)
        .catch(function () {
          // Kopru calismazsa WebView'in kendi indirme akisina birak.
          try { a.__bogahostSkip = true; a.click(); } catch (e2) {}
        });
      return;
    }

    var isHttp = abs.protocol === 'http:' || abs.protocol === 'https:';
    if (isHttp && !isInternal(abs.hostname)) {
      // Harici adres -> sistem tarayicisi (kopru yoksa uygulama icinde acilir).
      ev.preventDefault();
      openExternal(abs.href).catch(function () {
        try { a.__bogahostSkip = true; a.click(); } catch (e3) {}
      });
      return;
    }

    // ----- Kardes uygulama gecisi (panel-ici "Uygulamalar" menusu) -----
    // Bu linkler target="_blank" tasir; ASAGIDAKI "yeni sekme -> onizleme
    // penceresi" dalina duserse hedef panel kucuk bir ONIZLEME penceresinde
    // acilir, ana pencere gezinmez ve gecis splash'i HIC gorunmez ("arada
    // kalma"). Kardes panele HER ZAMAN ana pencerede, YERINDE gecilir: hedef
    // sayfanin document-start yukleme katmani tam ekran gecis splash'ini
    // deterministik olarak cizer (paylasilan .bogahost.com cerezi ile "onceki
    // != su anki uygulama" tespiti; bkz. LOADING_OVERLAY_JS).
    if (isHttp && isSiblingApp(abs)) {
      ev.preventDefault();
      try { location.href = abs.href; } catch (e6) {}
      return;
    }

    var target = (a.getAttribute('target') || '').toLowerCase();
    var newTab = !!(target && target !== '_self' && target !== '_top' && target !== '_parent');
    var dlAttr = a.getAttribute('download');

    // Kendi alan adimizdaki INDIRME linkleri: sayfayi indirme adresine
    // GOTURMEDEN dosyayi al ve diske yaz.
    //
    // DUZ linkler de (target/`download` olmadan) buraya girer: Rust
    // `on_download` YALNIZCA `Content-Disposition: attachment` gelen yanitlar
    // icin tetiklenir. `inline` gelen bir PDF/CSV o yolu HIC kullanmaz —
    // WebView onu gezinme sayar, ana pencere belgeye gider ve kullanici
    // panele donemez. Turu `handleMaybeDownload` sunucuya sorarak belirler,
    // boylece gercek HTML sayfalari normal sekilde acilmaya devam eder.
    if (isHttp && (dlAttr !== null || newTab ? looksLikeDownload(abs, a) : looksLikeFile(abs))) {
      ev.preventDefault();
      handleMaybeDownload(abs, dlAttr, newTab).catch(function (err) {
        if (err && err.status) {
          // Sunucu gercekten hata dondu -> kullaniciya soyle (sessiz 404 yok).
          reportDownloadError(err);
          return;
        }
        // Kopru/boyut sorunu -> WebView'in kendi indirme akisina birak.
        try { a.__bogahostSkip = true; a.click(); } catch (e5) {}
      });
      return;
    }

    if (isHttp && newTab) {
      // Yeni sekme WebView'de acilmaz. ANA pencereyi GOTURMEK yerine ayri,
      // kapatilabilir bir onizleme penceresi ac — panel yerinde kalsin.
      ev.preventDefault();
      openPopup(abs.href).catch(function () {
        // Kopru yoksa eski davranis (en azindan link calissin).
        try { location.href = abs.href; } catch (e4) {}
      });
    }
  }, true);

  // ---- Yeni sekmeye gonderilen form'lar (POST ile uretilen PDF/CSV) ----
  // `target="_blank"` tasiyan form'lar WebView'de HICBIR SEY yapmiyordu (yeni
  // pencere acilamaz). Artik form verisiyle istek atilip sonuc diske yazilir.
  document.addEventListener('submit', function (ev) {
    try {
      if (ev.defaultPrevented) { return; }
      var form = ev.target;
      if (!form || !form.tagName || form.tagName.toLowerCase() !== 'form') { return; }
      if (form.__bogahostSkip) { form.__bogahostSkip = false; return; }

      var t = (form.getAttribute('target') || '').toLowerCase();
      if (!t || t === '_self' || t === '_top' || t === '_parent') { return; }

      var abs;
      try { abs = new URL(form.getAttribute('action') || location.href, location.href); } catch (e) { return; }
      if (abs.protocol !== 'http:' && abs.protocol !== 'https:') { return; }
      if (!isInternal(abs.hostname)) { return; }

      var method = (form.getAttribute('method') || 'get').toLowerCase() === 'post' ? 'POST' : 'GET';
      var url = abs.href;
      var opts = { credentials: 'include', method: method };
      var data;
      try { data = new FormData(form); } catch (e2) { return; }

      if (method === 'POST') {
        opts.body = data;
      } else {
        try {
          var q = new URLSearchParams(data).toString();
          if (q) { url = abs.href + (abs.search ? '&' : '?') + q; }
        } catch (e3) {}
      }

      ev.preventDefault();
      fetch(url, opts).then(function (res) {
        if (!res.ok) {
          var err = new Error('http-' + res.status);
          err.status = res.status;
          throw err;
        }
        return saveResponse(res, abs, null, true);
      }).catch(function (err) {
        if (err && err.status) {
          reportDownloadError(err);
          return;
        }
        // Kopru calismadi: form'u AYNI pencerede gonder (eski davranis).
        try {
          form.__bogahostSkip = true;
          form.setAttribute('target', '_self');
          form.submit();
        } catch (e4) {}
      });
    } catch (e) {}
  }, true);

  var nativeOpen = window.open;
  window.open = function (u, name, features) {
    if (!u) { return null; }
    var abs;
    try { abs = new URL(String(u), location.href); } catch (e) {
      try { return nativeOpen.call(window, u, name, features); } catch (e2) { return null; }
    }
    if (abs.protocol === 'blob:' || abs.protocol === 'data:') {
      // Uretilen PDF/CSV: kaydet ve sistem uygulamasinda ac.
      saveLocal(abs.href, guessName(abs, null), true).catch(function () {});
      return null;
    }
    var isHttp2 = abs.protocol === 'http:' || abs.protocol === 'https:';
    if (isHttp2 && !isInternal(abs.hostname)) {
      openExternal(abs.href).catch(function () {
        try { location.href = abs.href; } catch (e4) {}
      });
      return null;
    }
    // Panellerin cok kullandigi bicim: window.open('/admin/.../pdf').
    // Ana pencereyi oraya GOTURMEK yerine dosyayi al ve diske yaz.
    if (isHttp2 && looksLikeDownload(abs, null)) {
      downloadViaBridge(abs, null, true).catch(function (err) {
        if (err && err.status) { reportDownloadError(err); return; }
        try { location.href = abs.href; } catch (e5) {}
      });
      return null;
    }
    // Kalan ic adresler: ANA pencereyi ele gecirmesin diye ayri pencerede acilir.
    openPopup(abs.href).catch(function () {
      try { location.href = abs.href; } catch (e3) {}
    });
    return null;
  };

  // ---- Erisimi engellenmis uygulama icin anlasilir ekran ----
  // Kabuk HTTP durum kodunu goremez; bu yuzden "Uygulamalar" menusunden gecis
  // yapildiktan SONRA (Rust `ACCESS_CHECK_SCRIPT` ile tetiklenir) hedef adres
  // bir kez daha sorgulanir. 403/401 ise beyaz/404 sayfa yerine bilgi ekrani cikar.
  function showAccessDenied(message) {
    try {
      if (document.getElementById('bogahost-native-denied')) { return; }
      var wrap = document.createElement('div');
      wrap.id = 'bogahost-native-denied';
      wrap.setAttribute('style', 'position:fixed;inset:0;z-index:2147483646;display:flex;align-items:center;justify-content:center;background:#0e1015;color:#e6e8ee;font:15px/1.6 -apple-system,"Segoe UI",Roboto,Arial,sans-serif;text-align:center;padding:32px;');
      var box = document.createElement('div');
      box.setAttribute('style', 'max-width:460px;');
      var head = document.createElement('div');
      head.setAttribute('style', 'font-size:19px;font-weight:600;margin-bottom:10px;');
      head.textContent = 'Erişim izniniz yok';
      var text = document.createElement('div');
      text.setAttribute('style', 'opacity:.78;margin-bottom:22px;');
      text.textContent = message;
      var btn = document.createElement('button');
      btn.setAttribute('style', 'background:#5443D2;color:#fff;border:0;border-radius:8px;padding:10px 22px;font:14px -apple-system,"Segoe UI",Roboto,Arial,sans-serif;cursor:pointer;');
      btn.textContent = 'Geri dön';
      btn.onclick = function () {
        try { if (wrap.parentNode) { wrap.parentNode.removeChild(wrap); } } catch (e) {}
        try { history.back(); } catch (e2) {}
      };
      box.appendChild(head);
      box.appendChild(text);
      box.appendChild(btn);
      wrap.appendChild(box);
      (document.body || document.documentElement).appendChild(wrap);
    } catch (e) {}
  }

  window.__bogahostAccessCheck = function (mode) {
    var msg = (mode === 'switch')
      ? 'Bu uygulamaya geçiş izniniz yok. Yöneticiniz bu uygulamaya erişiminizi kapatmış olabilir.'
      : 'Bu sayfaya erişim izniniz yok.';

    function evaluate(status) {
      if (status === 403 || status === 401) { showAccessDenied(msg); }
    }

    try {
      // Once HEAD (govde indirmeden durum kodu). Sunucu HEAD desteklemezse GET.
      fetch(location.href, { credentials: 'include', method: 'HEAD' })
        .then(function (res) {
          if (res.status === 405 || res.status === 501) {
            return fetch(location.href, { credentials: 'include' })
              .then(function (r2) { evaluate(r2.status); });
          }
          evaluate(res.status);
        })
        .catch(function () {});
    } catch (e) {}
  };

  // ---- Giris ekraninda surum rozeti ----
  // YALNIZCA giris sayfasinda gosterilir (panel arayuzu kirlenmesin).
  // Tiklanamaz (`pointer-events:none`) ve panelin kendi ogelerinin altinda kalir
  // (z-index, "Yükleniyor" katmanindan DUSUKTUR).
  var BADGE_ID = 'bogahost-native-version-badge';

  function isLoginPage() {
    try {
      var p = String(location.pathname || '').toLowerCase();
      if (p.indexOf('/login') >= 0 || p.indexOf('/giris') >= 0) { return true; }
      if (document.querySelector('input[type="password"]')) { return true; }
    } catch (e) {}
    try {
      if (document.querySelector('form[action*="login"]')) { return true; }
    } catch (e2) {}
    return false;
  }

  function renderBadge() {
    try {
      if (!document.body) { return; }
      var existing = document.getElementById(BADGE_ID);
      if (!isLoginPage()) {
        // Giristen panele gecildiyse (SPA/yonlendirme) rozet kaldirilir.
        if (existing && existing.parentNode) { existing.parentNode.removeChild(existing); }
        return;
      }
      if (existing) { return; }
      var v = window.__BOGAHOST_NATIVE_VERSION__ || '';
      var el = document.createElement('div');
      el.id = BADGE_ID;
      el.setAttribute('style', [
        'position:fixed',
        'left:50%',
        'bottom:14px',
        'transform:translateX(-50%)',
        'z-index:2147483000',
        'pointer-events:none',
        'user-select:none',
        '-webkit-user-select:none',
        'padding:4px 11px',
        'border-radius:999px',
        'border:1px solid rgba(128,132,145,.20)',
        'background:rgba(128,132,145,.10)',
        // Orta gri: hem koyu hem acik temada okunur.
        'color:#8a8f9c',
        'font:11px/1.4 -apple-system,"Segoe UI",Roboto,Arial,sans-serif',
        'letter-spacing:.3px',
        'white-space:nowrap',
        'opacity:.72'
      ].join(';'));
      el.textContent = (v ? 'v' + v + ' · ' : '') + 'Powered by Bogahost';
      document.body.appendChild(el);
    } catch (e) {}
  }

  // Ust duzey adres KENDISI bir belge ucu mu? (kopru atlandi ve sayfa belgeye
  // gitti.) Oyleyse gorunur kacis seridini goster — kullanici kilitli kalmasin.
  function looksLikeDocLocation() {
    try {
      var u = new URL(location.href);
      if (DOWNLOAD_FLAG.test(String(u.search || ''))) { return true; }
      if (DOWNLOAD_EXT.test(String(u.pathname || ''))) { return true; }
      if (/\/e-fatura-pdf|\/e-arsiv-pdf|\/dekont(\/|$)/i.test(String(u.pathname || ''))) { return true; }
    } catch (e) {}
    return false;
  }

  function scheduleBadge() {
    renderBadge();
    if (looksLikeDocLocation()) { showBackStrip(); }
    // Giris formu sonradan cizilirse (SPA) yakalanir; ~7 sn sonra durur.
    var tries = 0;
    var timer = setInterval(function () {
      tries++;
      renderBadge();
      if (tries >= 10) { clearInterval(timer); }
    }, 700);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', scheduleBadge);
  } else {
    scheduleBadge();
  }
})();
"#;

/// INIT_SCRIPT'ten SONRA enjekte edilen ikinci kopru.
///
/// Ayri bir IIFE ve ayri bir koruma bayragi kullanir; boylece iki betikten biri
/// hata verse bile digeri calismaya devam eder. Icerik:
///   1. eylemli uyari kutusu (izin reddinde "Sistem Ayarlarini Ac" dugmesi),
///   2. tekillestirilmis native bildirim (`__bogahostNotifyOnce`),
///   3. PANEL BILDIRIMLERI -> NATIVE BILDIRIM (asil duzeltme; ek ag yuku yok),
///   4. kamera/mikrofon/ekran hatalarinda anlasilir geri bildirim,
///   5. pano kopyalama yedegi,
///   6. ses kilidi (macOS'ta autoplay politikasi icin),
///   7. tam ekran yedegi (macOS'ta HTML fullscreen calismaz),
///   8. bildirim izni reddedildiginde ayar dugmesi.
const EXTRA_SCRIPT: &str = r#"
(function () {
  if (window.__BOGAHOST_NATIVE_EXTRA__) { return; }
  window.__BOGAHOST_NATIVE_EXTRA__ = true;

  var APP_KEY = String(window.__BOGAHOST_APP_KEY__ || '');

  function invoke(cmd, args) {
    try {
      var t = window.__TAURI__;
      if (t && t.core && typeof t.core.invoke === 'function') { return t.core.invoke(cmd, args); }
      if (t && typeof t.invoke === 'function') { return t.invoke(cmd, args); }
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
        return window.__TAURI_INTERNALS__.invoke(cmd, args);
      }
    } catch (e) {}
    return Promise.reject(new Error('ipc-yok'));
  }

  function toast(msg) {
    try { if (typeof window.__bogahostToast === 'function') { window.__bogahostToast(msg); return; } } catch (e) {}
    try { console.warn('[bogahost]', msg); } catch (e2) {}
  }

  // =========================================================================
  // 1) EYLEMLI UYARI KUTUSU
  // =========================================================================
  // Sessiz hata YOK: izin reddedildiginde kullaniciya ne oldugu SOYLENIR ve
  // ilgili sistem ayarini acan bir dugme verilir.
  var BOX_ID = 'bogahost-native-actionbox';

  function actionBox(title, message, buttonLabel, settingsKind) {
    try {
      var old = document.getElementById(BOX_ID);
      if (old && old.parentNode) { old.parentNode.removeChild(old); }

      var host = document.body || document.documentElement;
      if (!host) { return; }

      var wrap = document.createElement('div');
      wrap.id = BOX_ID;
      wrap.setAttribute('style', 'position:fixed;left:50%;top:22px;transform:translateX(-50%);z-index:2147483647;max-width:min(92vw,460px);padding:15px 18px;border-radius:12px;background:#22242c;color:#e6e8ee;border:1px solid rgba(255,255,255,.13);box-shadow:0 10px 34px rgba(0,0,0,.42);font:13px/1.55 -apple-system,Segoe UI,Roboto,Arial,sans-serif;');

      var h = document.createElement('div');
      h.setAttribute('style', 'font-weight:600;font-size:14px;margin-bottom:5px;');
      h.textContent = title;
      wrap.appendChild(h);

      var p = document.createElement('div');
      p.setAttribute('style', 'opacity:.82;');
      p.textContent = message;
      wrap.appendChild(p);

      var row = document.createElement('div');
      row.setAttribute('style', 'margin-top:13px;display:flex;gap:8px;justify-content:flex-end;');

      // Ayar dugmesi YALNIZCA platformda gercekten bir ayar adresi varsa anlamli.
      // Adres yoksa Rust tarafi sessizce hicbir sey yapmaz; bu yuzden dugme
      // ancak bir "kind" verildiginde cizilir.
      if (settingsKind) {
        var b1 = document.createElement('button');
        b1.type = 'button';
        b1.textContent = buttonLabel || 'Sistem Ayarlarını Aç';
        b1.setAttribute('style', 'background:#5443D2;color:#fff;border:0;border-radius:8px;padding:8px 14px;font:13px -apple-system,Segoe UI,Roboto,Arial,sans-serif;cursor:pointer;');
        b1.onclick = function () { invoke('bogahost_open_settings', { kind: settingsKind }).catch(function () {}); };
        row.appendChild(b1);
      }

      var b2 = document.createElement('button');
      b2.type = 'button';
      b2.textContent = 'Kapat';
      b2.setAttribute('style', 'background:transparent;color:#c9cdd8;border:1px solid rgba(255,255,255,.16);border-radius:8px;padding:8px 14px;font:13px -apple-system,Segoe UI,Roboto,Arial,sans-serif;cursor:pointer;');
      b2.onclick = function () { try { if (wrap.parentNode) { wrap.parentNode.removeChild(wrap); } } catch (e) {} };
      row.appendChild(b2);

      wrap.appendChild(row);
      host.appendChild(wrap);

      setTimeout(function () { try { if (wrap.parentNode) { wrap.parentNode.removeChild(wrap); } } catch (e) {} }, 20000);
    } catch (e) {}
  }
  try { window.__bogahostActionBox = actionBox; } catch (e) {}

  // =========================================================================
  // 2) TEKILLESTIRILMIS NATIVE BILDIRIM
  // =========================================================================
  // Hem panelin `new Notification()` cagrisi hem de asagidaki besleme dinleyicisi
  // BURADAN gecer — ayni bildirim IKI KEZ gosterilmez.
  var seen = Object.create(null);
  var seenOrder = [];
  var SEEN_MAX = 500;

  window.__bogahostNotifyOnce = function (key, title, body, url) {
    try {
      if (key) {
        if (seen[key]) { return false; }
        seen[key] = 1;
        seenOrder.push(key);
        while (seenOrder.length > SEEN_MAX) { delete seen[seenOrder.shift()]; }
      }
      invoke('bogahost_notify', {
        title: String(title == null ? '' : title),
        body: String(body == null ? '' : body),
        url: url ? String(url) : null
      }).catch(function () {});
      return true;
    } catch (e) { return false; }
  };

  // =========================================================================
  // 3) PANEL BILDIRIMLERI -> NATIVE BILDIRIM   (ASIL DUZELTME)
  // =========================================================================
  // KOK NEDEN: paneller masaustu bildirimini SERVICE WORKER + WEB PUSH ile
  // gosteriyor. Native WebView'de `serviceWorker`/`PushManager` YOKTUR; panelin
  // `pushInit()` fonksiyonu ilk satirda sessizce cikiyor. Panel bildirimi
  // yalnizca sayfa ICI baloncuk (toast) + bip olarak gosteriyor, `new
  // Notification()` HIC cagrilmiyor — bu yuzden v1.5.0'daki Notification
  // koprusu de hicbir zaman tetiklenmiyordu. Sonuc: uygulamada HICBIR bildirim.
  //
  // COZUM (mimari): saati RUST tutar, istegi SAYFA atar, tekillestirme ve
  // gosterim yine RUST'tadir.
  //
  //   Rust `start_notify_clock` (OS is parcacigi)
  //     -> eval -> BURADAKI `__bogahostFeedTick`
  //          -> panelin bildirim ucuna `fetch` (oturum cerezi + CSRF + yetki
  //             sayfada ZATEN cozulmus)
  //               -> invoke('bogahost_notify_feed') -> native bildirim + rozet
  //
  // NEDEN saat Rust'ta: `setInterval` pencere gizlendiginde isletim sistemi
  // tarafindan KISILIR — tam da uygulama tepside dururken. `eval` ile ACIKCA
  // calistirilan betik zamanlayici degildir, aninda kosar.
  //
  // NEDEN istek sayfada: oturum cerezi HttpOnly'dir ve panelin kendi yetki
  // katmani (auth/perm ara katmanlari) sayfa baglaminda zaten gecerlidir;
  // cerezi Rust'a tasimak fazladan kirilma noktasi olurdu.
  //
  // Panel PENCERE ACIKKEN kendisi zaten yokluyor: o yanit `fetch` sarmalayicisi
  // ile ucretsiz dinlenir, kendi istegimiz ATLANIR (sunucuya ek yuk binmez).
  var FEED_RE = /\/notifications(\/feed)?(\?|$)/;
  var feedSeenAt = 0;
  var chatCursor = null;
  var halted = false;
  var backoff = 0;

  function APP_TITLE_SAFE() {
    try { return String(window.__BOGAHOST_APP_TITLE__ || 'Bogahost'); } catch (e) { return 'Bogahost'; }
  }

  // Normallestirilmis kayitlari Rust'a verir.
  // Oturum ici `seen` yalnizca AYNI yanitin iki yoldan (sarmalayici + kendi
  // yoklamamiz) gelmesini eler; KALICI tekillestirme Rust'tadir.
  function pushFeed(list, unread) {
    var fresh = [];
    for (var i = 0; i < list.length; i++) {
      var it = list[i];
      if (!it.key || seen[it.key]) { continue; }
      seen[it.key] = 1;
      seenOrder.push(it.key);
      while (seenOrder.length > SEEN_MAX) { delete seen[seenOrder.shift()]; }
      fresh.push(it);
    }
    var count = (typeof unread === 'number') ? unread : null;
    // Yeni kayit yoksa bile rozeti tazelemek icin sayiyi gonder.
    if (!fresh.length && count === null) { return; }
    try { invoke('bogahost_notify_feed', { items: fresh, unread: count }).catch(function () {}); } catch (e) {}
  }

  // DCIM / Finans / Görevler bicimi: {unread, items:[{id,title,body,url,read,age_s}]}
  function handleStandardFeed(d) {
    var items = (d && d.items) || [];
    var out = [];
    for (var i = 0; i < items.length; i++) {
      var n = items[i];
      // Panelde ZATEN okunmus kaydi masaustunde duyurma.
      if (n.read) { continue; }
      out.push({
        key: 'feed:' + n.id,
        title: n.title || APP_TITLE_SAFE(),
        body: n.body || '',
        url: n.url || null,
        age_s: n.age_s || 0
      });
    }
    pushFeed(out, typeof d.unread === 'number' ? d.unread : null);
  }

  // Chat bicimi: {ok, init, messages:[], new_conversations:[], internal_messages:[], max_*}
  function handleChatFeed(d) {
    if (!d || d.ok !== true) { return; }
    // Imleci HER yanittan tazele (yedek yoklama bunu kullanir).
    chatCursor = {
      msg: d.max_msg || 0,
      conv: d.max_conv || 0,
      internal: d.max_internal || 0
    };
    // Ilk cagri yalnizca imlec kurar — gecmis TOPLUCA gosterilmez.
    if (d.init) { return; }

    var out = [];
    var convs = d.new_conversations || [];
    for (var a = 0; a < convs.length; a++) {
      out.push({
        key: 'conv:' + convs[a].id,
        title: 'Yeni sohbet · ' + (convs[a].visitor || 'Ziyaretçi'),
        body: convs[a].preview || 'Sohbet başladı',
        url: '/admin/chats?c=' + convs[a].id,
        age_s: 0
      });
    }
    var msgs = d.messages || [];
    for (var b = 0; b < msgs.length; b++) {
      out.push({
        key: 'msg:' + msgs[b].id,
        title: msgs[b].visitor || 'Ziyaretçi',
        body: msgs[b].preview || 'Yeni mesaj',
        url: '/admin/chats?c=' + msgs[b].conversation_id,
        age_s: 0
      });
    }
    var ints = d.internal_messages || [];
    for (var c = 0; c < ints.length; c++) {
      out.push({
        key: 'int:' + ints[c].id,
        title: 'Personel · ' + (ints[c].from || 'Ekip'),
        body: ints[c].preview || '',
        url: '/admin/internal',
        age_s: 0
      });
    }

    // Rozet: bekleyen (karsilanmamis) sohbet sayisi.
    pushFeed(out, typeof d.waiting === 'number' ? d.waiting : null);
  }

  // `fromPanel` = yaniti PANEL istedi (biz degil). Yalnizca o durumda
  // `feedSeenAt` tazelenir; kendi istegimiz kendini bastirmasin diye
  // (aksi halde bir tur atlanir ve aralik iki katina cikardi).
  function consumeFeed(data, fromPanel) {
    try {
      if (!data || typeof data !== 'object') { return; }
      if (fromPanel) { feedSeenAt = Date.now(); }
      // Bicimi ALANA gore ayirt et (URL'e degil): Chat farkli bir sema dondurur.
      if (Object.prototype.hasOwnProperty.call(data, 'items')) { handleStandardFeed(data); }
      else if (Object.prototype.hasOwnProperty.call(data, 'max_msg')) { handleChatFeed(data); }
    } catch (e) {}
  }

  // ---- Panelin KENDI yoklamasini dinle (ek istek yok) ----
  var nativeFetch = window.fetch;
  if (typeof nativeFetch === 'function') {
    window.fetch = function (input, init) {
      var promise = nativeFetch.apply(this, arguments);
      try {
        var method = 'GET';
        var u = '';
        if (typeof input === 'string') { u = input; }
        else if (input && typeof input === 'object') { u = input.url || ''; method = input.method || 'GET'; }
        if (init && init.method) { method = init.method; }

        if (u && String(method).toUpperCase() === 'GET' && FEED_RE.test(String(u))) {
          promise.then(function (res) {
            try {
              if (!res || !res.ok) { return; }
              // Govdeyi TUKETME: panel ayni yaniti kendi okuyacak.
              res.clone().json()
                .then(function (d) { consumeFeed(d, true); })
                .catch(function () {});
            } catch (e) {}
          }).catch(function () {});
        }
      } catch (e) {}
      return promise;
    };
  }

  // ---- Kabugun KENDI yoklamasi (saat Rust'ta) ----
  function feedUrl() {
    if (APP_KEY === 'chat') {
      if (!chatCursor) { return '/admin/notifications'; }
      return '/admin/notifications?after_msg=' + chatCursor.msg +
             '&after_conv=' + chatCursor.conv +
             '&after_internal=' + chatCursor.internal;
    }
    return '/admin/notifications/feed';
  }

  function looksLikePanel() {
    try {
      // Giris ekraninda yoklama YAPMA (401/302 dongusu olusmasin).
      if (document.querySelector('input[type="password"]')) { return false; }
      return String(location.pathname || '').indexOf('/admin') === 0;
    } catch (e) { return false; }
  }

  // Rust saatinin her turda cagirdigi giris noktasi (`start_notify_clock`).
  //
  // TAMAMEN SESSIZDIR: ag hatasi, oturum dususu, 401/403 -> hicbir bildirim
  // gosterilmez, yalnizca konsola yazilir. Her turda hata bildirimi CIKMAZ.
  window.__bogahostFeedTick = function () {
    try {
      // Oturum/yetki yok: yoklama TAMAMEN durdu (sayfa yenilenince sifirlanir).
      if (halted) { return; }
      // Giris ekrani / panel disi sayfa.
      if (!looksLikePanel()) { return; }
      // Sunucu bogulmus -> ustel geri cekilme suresi dolmadi.
      if (backoff > Date.now()) { return; }
      // Panel PENCERE ACIKKEN kendisi yokluyor (son 60 sn icinde yanit gorduk):
      // ayni ucu ikinci kez cagirip sunucuyu yorma.
      if (feedSeenAt && (Date.now() - feedSeenAt) < 60000) { return; }

      nativeFetch(feedUrl(), {
        credentials: 'same-origin',
        headers: { 'X-Requested-With': 'XMLHttpRequest' }
      }).then(function (res) {
        if (res.status === 401 || res.status === 403) {
          halted = true;
          console.warn('[bogahost] bildirim yoklamasi durdu: oturum/yetki yok');
          return null;
        }
        if (!res.ok) { backoffBump(res.status); return null; }
        backoff = 0;
        backoffStep = 0;
        return res.json();
      }).then(function (d) {
        if (d) { consumeFeed(d, false); }
      }).catch(function (e) {
        // Ag yok / DNS / TLS: sessizce geri cekil.
        backoffBump(0);
        console.warn('[bogahost] bildirim yoklamasi basarisiz:', e);
      });
    } catch (e) {}
  };

  // Ustel geri cekilme: 90 sn -> 3 dk -> 6 dk ... en fazla 15 dk.
  var backoffStep = 0;
  function backoffBump(status) {
    backoffStep = Math.min(backoffStep ? backoffStep * 2 : 90000, 900000);
    backoff = Date.now() + backoffStep;
    console.warn('[bogahost] bildirim yoklamasi ertelendi (' + status + ')');
  }

  // =========================================================================
  // 4) KAMERA / MIKROFON / EKRAN PAYLASIMI
  // =========================================================================
  // macOS: wry, WKWebView izin istegini KENDISI onaylar; asil kapi isletim
  // sistemi (TCC) ve Hardened Runtime entitlement'laridir — bkz.
  // src-tauri/Info.plist ve src-tauri/Bogahost.entitlements.
  // Windows: WebView2 KENDI izin penceresini gosterir. Kullanici bir kez
  // "Engelle" derse secim profile YAZILIR ve pencere BIR DAHA CIKMAZ; asagidaki
  // mesaj bu durumu acikca anlatir.
  function mediaMessage(err, isDisplay) {
    var name = (err && (err.name || err.constructor && err.constructor.name)) || '';
    var what = isDisplay ? 'Ekran paylaşımı' : 'Kamera/mikrofon';

    if (name === 'NotAllowedError' || name === 'PermissionDeniedError' || name === 'SecurityError') {
      return {
        title: what + ' izni verilmedi',
        msg: 'İzin reddedildi. Sistem Ayarları > Gizlilik bölümünden bu uygulamaya izin verip uygulamayı yeniden başlatın.',
        kind: isDisplay ? 'screen' : 'camera'
      };
    }
    if (name === 'NotFoundError' || name === 'DevicesNotFoundError' || name === 'OverconstrainedError') {
      return { title: what + ' bulunamadı', msg: 'Bilgisayarda uygun bir kamera/mikrofon bulunamadı ya da istenen ayarlar desteklenmiyor.', kind: null };
    }
    if (name === 'NotReadableError' || name === 'TrackStartError') {
      return { title: what + ' kullanılamıyor', msg: 'Cihaz başka bir uygulama tarafından kullanılıyor olabilir. Diğer uygulamaları kapatıp yeniden deneyin.', kind: isDisplay ? 'screen' : 'camera' };
    }
    if (name === 'AbortError') {
      return { title: what + ' başlatılamadı', msg: 'Donanım hatası nedeniyle başlatılamadı. Uygulamayı yeniden başlatmayı deneyin.', kind: null };
    }
    return {
      title: what + ' başlatılamadı',
      msg: 'Beklenmeyen bir hata oluştu' + (name ? ' (' + name + ')' : '') + '. İzinleri denetleyip yeniden deneyin.',
      kind: isDisplay ? 'screen' : 'camera'
    };
  }

  function reportMediaError(err, isDisplay) {
    var info = mediaMessage(err, isDisplay);
    actionBox(info.title, info.msg, 'Sistem Ayarlarını Aç', info.kind);
  }
  try { window.__bogahostMediaError = reportMediaError; } catch (e) {}

  var md = navigator.mediaDevices;

  if (!md || typeof md.getUserMedia !== 'function') {
    // WebView medya API'sini HIC sunmuyor: sessiz kalma, sebebini soyle.
    var stub = {
      getUserMedia: function () {
        var e = new Error('Bu sürümde kamera/mikrofon desteklenmiyor.');
        e.name = 'NotSupportedError';
        actionBox('Kamera/mikrofon desteklenmiyor',
          'Bu masaüstü sürümü medya yakalamayı desteklemiyor. Sesli/görüntülü arama için tarayıcıdan veya mobil uygulamadan girin.', null, null);
        return Promise.reject(e);
      },
      enumerateDevices: function () { return Promise.resolve([]); }
    };
    stub.getDisplayMedia = stub.getUserMedia;
    try { Object.defineProperty(navigator, 'mediaDevices', { value: stub, configurable: true }); } catch (e) {}
  } else {
    // Ornek uzerine KENDI ozelligimizi koyariz (prototip degistirilmez).
    var origGUM = md.getUserMedia.bind(md);
    md.getUserMedia = function (constraints) {
      return origGUM(constraints).catch(function (err) {
        reportMediaError(err, false);
        throw err;
      });
    };

    if (typeof md.getDisplayMedia === 'function') {
      var origGDM = md.getDisplayMedia.bind(md);
      md.getDisplayMedia = function (constraints) {
        return origGDM(constraints).catch(function (err) {
          reportMediaError(err, true);
          throw err;
        });
      };
    } else {
      // macOS 14.0-14.5 araliginda WKWebView + wry'nin izin temsilcisi
      // birlikte ekran paylasimini kapatabiliyor (wry#1195, HALA ACIK).
      md.getDisplayMedia = function () {
        var e = new Error('Ekran paylaşımı bu sürümde kullanılamıyor.');
        e.name = 'NotSupportedError';
        actionBox('Ekran paylaşımı kullanılamıyor',
          'Bu masaüstü sürümünde ekran paylaşımı desteklenmiyor. Tarayıcıdan girerek paylaşabilirsiniz.', null, null);
        return Promise.reject(e);
      };
    }
  }

  // Eski cagri bicimi (`navigator.getUserMedia`) kullanan paneller icin kopru.
  try {
    if (typeof navigator.getUserMedia !== 'function' && navigator.mediaDevices) {
      navigator.getUserMedia = function (c, ok, fail) {
        navigator.mediaDevices.getUserMedia(c).then(ok).catch(fail || function () {});
      };
    }
  } catch (e) {}

  // =========================================================================
  // 5) PANO (kopyala/yapistir)
  // =========================================================================
  // WebView2'de `navigator.clipboard.readText()` izin ister ve Tauri bu istegi
  // ele almadigi icin REDDEDILIR. `writeText` ise kullanici hareketi olmadan
  // basarisiz olabilir. Her iki durumda da eski `execCommand` yoluna duseriz —
  // "Kopyala" dugmesi sessizce calismamis gibi gorunmesin.
  function execCopy(text) {
    try {
      var ta = document.createElement('textarea');
      ta.value = String(text == null ? '' : text);
      ta.setAttribute('style', 'position:fixed;top:-1000px;left:-1000px;opacity:0;');
      (document.body || document.documentElement).appendChild(ta);
      ta.focus();
      ta.select();
      var ok = false;
      try { ok = document.execCommand('copy'); } catch (e) {}
      try { if (ta.parentNode) { ta.parentNode.removeChild(ta); } } catch (e2) {}
      return ok;
    } catch (e3) { return false; }
  }

  try {
    if (!navigator.clipboard) {
      Object.defineProperty(navigator, 'clipboard', {
        value: {
          writeText: function (t) { return execCopy(t) ? Promise.resolve() : Promise.reject(new Error('kopyalanamadi')); },
          readText: function () { return Promise.reject(new Error('okuma-desteklenmiyor')); }
        },
        configurable: true
      });
    } else if (typeof navigator.clipboard.writeText === 'function') {
      var origWrite = navigator.clipboard.writeText.bind(navigator.clipboard);
      navigator.clipboard.writeText = function (t) {
        return origWrite(t).catch(function (err) {
          if (execCopy(t)) { return; }
          toast('Panoya kopyalanamadı.');
          throw err;
        });
      };
    }
  } catch (e) {}

  // =========================================================================
  // 6) SES KILIDI (bildirim sesi / arama zili)
  // =========================================================================
  // Windows'ta `--autoplay-policy=no-user-gesture-required` ile cozuldu.
  // macOS'ta karsiligi YOKTUR (wry'de `with_autoplay` var ama Tauri 2 disari
  // acmaz), bu yuzden ILK kullanici hareketinde AudioContext uyandirilir ve
  // sessiz bir ses calinir — sonraki zil/bip sesleri engellenmez.
  (function () {
    var unlocked = false;
    function unlock() {
      if (unlocked) { return; }
      unlocked = true;
      try {
        var Ctx = window.AudioContext || window.webkitAudioContext;
        if (Ctx) {
          if (!window.__bogahostAudioCtx) { window.__bogahostAudioCtx = new Ctx(); }
          var ac = window.__bogahostAudioCtx;
          if (ac.state === 'suspended') { ac.resume(); }
          var b = ac.createBuffer(1, 1, 22050);
          var src = ac.createBufferSource();
          src.buffer = b;
          src.connect(ac.destination);
          src.start(0);
        }
      } catch (e) {}
      window.removeEventListener('pointerdown', unlock, true);
      window.removeEventListener('keydown', unlock, true);
    }
    window.addEventListener('pointerdown', unlock, true);
    window.addEventListener('keydown', unlock, true);
  })();

  // =========================================================================
  // 7) TAM EKRAN
  // =========================================================================
  // macOS'ta HTML `element.requestFullscreen()` CALISMAZ: wry ilgili
  // WKPreferences anahtarini yalnizca `fullscreen` ozelligi (Tauri'de
  // `macos-private-api`) acikken kurar; o da OZEL API oldugu ve App Store
  // riski tasidigi icin ACILMADI. Istek basarisiz olursa PENCERE tam ekran
  // yapilir — kullanici acisindan sonuc buyuk olcude aynidir.
  try {
    var El = window.Element;
    if (El && El.prototype) {
      var origRFS = El.prototype.requestFullscreen ||
                    El.prototype.webkitRequestFullscreen ||
                    El.prototype.mozRequestFullScreen;

      El.prototype.requestFullscreen = function () {
        var self = this;
        try {
          if (origRFS) {
            var r = origRFS.apply(self, arguments);
            if (r && typeof r.then === 'function') {
              return r.catch(function () { return invoke('bogahost_set_fullscreen', { on: true }); });
            }
            return Promise.resolve(r);
          }
        } catch (e) {}
        return invoke('bogahost_set_fullscreen', { on: true });
      };

      var origExit = document.exitFullscreen;
      document.exitFullscreen = function () {
        try {
          if (origExit) {
            var r2 = origExit.apply(document, arguments);
            if (r2 && typeof r2.then === 'function') {
              return r2.catch(function () { return invoke('bogahost_set_fullscreen', { on: false }); });
            }
            return Promise.resolve(r2);
          }
        } catch (e) {}
        return invoke('bogahost_set_fullscreen', { on: false });
      };
    }
  } catch (e) {}

  // =========================================================================
  // 8) BILDIRIM IZNI REDDEDILDIYSE ACIKLAMA + AYAR DUGMESI
  // =========================================================================
  try {
    window.__bogahostNotifySettings = function () {
      return invoke('bogahost_open_settings', { kind: 'notifications' });
    };
    window.__bogahostNotifyDeniedBox = function () {
      actionBox('Bildirimler kapalı',
        'Masaüstü bildirimleri için bu uygulamaya bildirim izni verilmeli. Sistem Ayarları > Bildirimler bölümünden açabilirsiniz.',
        'Bildirim Ayarlarını Aç', 'notifications');
    };
  } catch (e) {}
})();
"#;

/// Guncelleme seridi + "mesgulum" tespiti (ANA pencere).
///
/// IKI isi vardir:
///  1. **Mesgul tespiti** — gorusme (getUserMedia/getDisplayMedia), acik
///     `RTCPeerConnection` ve doldurulmus form alanlari izlenir; durum
///     degistikce `bogahost_set_busy` ile Rust'a bildirilir. Rust bu bayrakla
///     arka plan INDIRMESINI erteler.
///  2. **Serit** — Rust `window.__bogahostUpdateReady(surum, force)` cagirinca
///     sag altta "Güncelleme hazır (vX) — Şimdi uygula / Sonra" kutusu cizilir.
///     Serit KENDILIGINDEN hicbir sey yapmaz; mesgulken "Şimdi uygula" ilk
///     tiklamada ONAY ister (tek tikla gorusme ortasinda yeniden baslatilmasin).
///
/// "Sonra" karari `sessionStorage`dadir: sayfa gezinmelerinde korunur, uygulama
/// yeniden acilinca sifirlanir (o zaman zaten yeni surum kurulu olur).
const UPDATE_UI_JS: &str = r#"
(function () {
  // `initialization_script` Windows'ta (WebView2) ALT CERCEVELERE de enjekte
  // edilir. Serit ve mesgul bildirimi YALNIZCA en ust cercevede calismalidir.
  try {
    if (window.top !== window.self) { return; }
  } catch (e) { return; }
  if (window.__BOGAHOST_UPDATE_UI__) { return; }
  window.__BOGAHOST_UPDATE_UI__ = true;

  function invoke(cmd, args) {
    try {
      var t = window.__TAURI__;
      if (t && t.core && typeof t.core.invoke === 'function') { return t.core.invoke(cmd, args); }
      if (t && typeof t.invoke === 'function') { return t.invoke(cmd, args); }
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
        return window.__TAURI_INTERNALS__.invoke(cmd, args);
      }
    } catch (e) {}
    return Promise.reject(new Error('ipc-yok'));
  }

  // =========================================================================
  // 1) "MESGULUM" TESPITI
  // =========================================================================
  // Amac: kullanici GORUSMEDEYKEN, EKRAN PAYLASIRKEN ya da FORM DOLDURURKEN
  // arka planda indirme baslamasin ve serit ISRARCI olmasin.
  //
  // Tespit UC kaynaktan beslenir; hicbiri sayfanin isbirligini SART kosmaz:
  //   a) canli medya yakalama (getUserMedia / getDisplayMedia)
  //   b) acik RTCPeerConnection (kapatilmamis WebRTC oturumu)
  //   c) kullanicinin degistirdigi, henuz gonderilmemis form alanlari
  // Ayrica sayfa isterse `window.__bogahostSetBusy(true/false)` ile ACIKCA
  // bildirebilir (en guvenilir kaynak; panel kendi arama durumunu bilir).

  var liveTracks = 0;   // (a)
  var openPeers = 0;    // (b)
  var dirty = [];       // (c) kullanicinin dokundugu form alanlari
  var declared = null;  // sayfanin acik beyani (null = beyan yok)

  function watchStream(stream) {
    try {
      var tracks = stream.getTracks();
      for (var i = 0; i < tracks.length; i++) {
        (function (track) {
          if (track.__bogahostWatched) { return; }
          track.__bogahostWatched = true;
          if (track.readyState === 'ended') { return; }
          liveTracks++;
          var done = false;
          function end() {
            if (done) { return; }
            done = true;
            liveTracks = Math.max(0, liveTracks - 1);
            report();
          }
          track.addEventListener('ended', end);
          // `stop()` 'ended' olayini TETIKLEMEZ (spec geregi) — sarmalanmali.
          var origStop = track.stop;
          track.stop = function () {
            try { return origStop.apply(track, arguments); } finally { end(); }
          };
        })(tracks[i]);
      }
    } catch (e) {}
    report();
    return stream;
  }

  try {
    var md = navigator.mediaDevices;
    if (md && typeof md.getUserMedia === 'function') {
      // NOT: bu sarmalayici EXTRA_SCRIPT'in sarmalayicisinin USTUNE gelir
      // (o hata mesajlarini gosterir, bu yalnizca sayar) — zincir bozulmaz.
      var gum = md.getUserMedia.bind(md);
      md.getUserMedia = function () {
        return gum.apply(null, arguments).then(watchStream);
      };
      if (typeof md.getDisplayMedia === 'function') {
        var gdm = md.getDisplayMedia.bind(md);
        md.getDisplayMedia = function () {
          return gdm.apply(null, arguments).then(watchStream);
        };
      }
    }
  } catch (e) {}

  try {
    var PC = window.RTCPeerConnection;
    if (typeof PC === 'function') {
      var Wrapped = function () {
        var pc = new (Function.prototype.bind.apply(PC, [null].concat([].slice.call(arguments))))();
        openPeers++;
        var closed = false;
        function shut() {
          if (closed) { return; }
          closed = true;
          openPeers = Math.max(0, openPeers - 1);
          report();
        }
        try {
          var origClose = pc.close;
          pc.close = function () {
            try { return origClose.apply(pc, arguments); } finally { shut(); }
          };
          pc.addEventListener('connectionstatechange', function () {
            if (pc.connectionState === 'closed' || pc.connectionState === 'failed') { shut(); }
          });
        } catch (e) {}
        report();
        return pc;
      };
      Wrapped.prototype = PC.prototype;
      // `RTCPeerConnection.generateCertificate` gibi statikler kaybolmasin.
      try {
        for (var k in PC) { if (!(k in Wrapped)) { Wrapped[k] = PC[k]; } }
      } catch (e) {}
      window.RTCPeerConnection = Wrapped;
      try { window.webkitRTCPeerConnection = Wrapped; } catch (e) {}
    }
  } catch (e) {}

  // Form alani: yalnizca GERCEK veri girisi sayilir. Arama/filtre kutulari ve
  // form disindaki tek tuk input'lar "mesgul" saymaz — aksi halde bayrak surekli
  // takili kalir ve guncelleme hic inmez.
  function countsAsForm(el) {
    try {
      if (!el || !el.tagName) { return false; }
      if (el.hasAttribute && el.hasAttribute('data-bhx-nobusy')) { return false; }
      if (el.isContentEditable) { return true; }
      var tag = el.tagName.toUpperCase();
      if (tag === 'TEXTAREA') { return true; }
      if (tag !== 'INPUT' && tag !== 'SELECT') { return false; }
      var type = String(el.type || '').toLowerCase();
      if (type === 'search' || type === 'hidden' || type === 'submit' || type === 'button') { return false; }
      if (el.readOnly || el.disabled) { return false; }
      return !!(el.form || (el.closest && el.closest('form')));
    } catch (e) { return false; }
  }

  function markDirty(ev) {
    var el = ev && ev.target;
    if (!countsAsForm(el)) { return; }
    if (dirty.indexOf(el) === -1) {
      dirty.push(el);
      report();
    }
  }

  function clearDirty() {
    if (dirty.length) { dirty = []; report(); }
  }

  try {
    document.addEventListener('input', markDirty, true);
    document.addEventListener('change', markDirty, true);
    // Gonderildi -> artik "yarim kalmis is" yok.
    document.addEventListener('submit', clearDirty, true);
  } catch (e) {}

  function hasDirty() {
    // DOM'dan kaldirilmis alanlar (kapanmis modal, yeniden cizilmis liste)
    // sayilmaz; aksi halde bayrak sonsuza dek takili kalir.
    var live = [];
    for (var i = 0; i < dirty.length; i++) {
      var el = dirty[i];
      try {
        if (el && el.isConnected) { live.push(el); }
      } catch (e) {}
    }
    dirty = live;
    return dirty.length > 0;
  }

  // Sayfanin ACIK beyani — panel kendi arama/kayit durumunu en iyi bilir.
  window.__bogahostSetBusy = function (value) {
    declared = value === true ? true : (value === false ? false : null);
    report();
    return true;
  };

  function isBusy() {
    if (declared === true) { return true; }
    if (liveTracks > 0 || openPeers > 0) { return true; }
    if (window.__BOGAHOST_BUSY__ === true) { return true; }
    if (declared === false) { return false; }
    return hasDirty();
  }

  var lastSent = null;
  function report() {
    var busy = isBusy();
    if (busy === lastSent) { return; }
    lastSent = busy;
    invoke('bogahost_set_busy', { busy: busy }).catch(function () {});
    try { window.__BOGAHOST_BUSY_STATE__ = busy; } catch (e) {}
    render();
  }

  // Sarmalayicilarin kacirdigi durumlar (dogrudan `pc.close()` yerine sayfa
  // yenilemesi, DOM'dan silinen form, vb.) icin dusuk maliyetli emniyet turu.
  try { setInterval(report, 5000); } catch (e) {}

  // =========================================================================
  // 2) GUNCELLEME SERIDI
  // =========================================================================
  var BAR_ID = '__bogahost_update_bar__';
  var pendingVersion = '';
  var confirmArmed = false;
  // Otomatik kurulum KALICI basarisiz (imzasiz macOS): serit "Şimdi uygula"
  // yerine "İndirme sayfasını aç" gosterir. Rust `__bogahostUpdateManual` ile kurar.
  var manualMode = false;

  function dismissKey(v) { return 'bogahost_update_dismissed_' + v; }

  function dismissed(v) {
    try { return window.sessionStorage.getItem(dismissKey(v)) === '1'; } catch (e) { return false; }
  }

  function dismiss(v) {
    // `sessionStorage` bilincli secim: karar SAYFA GEZINMELERINDE korunur ama
    // uygulama yeniden acilinca sifirlanir. "Sonra" diyen kullaniciya ayni
    // oturumda bir daha sorulmaz; zaten bir sonraki dogal acilista kurulu olur.
    try { window.sessionStorage.setItem(dismissKey(v), '1'); } catch (e) {}
  }

  function remove() {
    try {
      var old = document.getElementById(BAR_ID);
      if (old && old.parentNode) { old.parentNode.removeChild(old); }
    } catch (e) {}
  }

  function render() {
    if (!pendingVersion || dismissed(pendingVersion)) { remove(); return; }
    if (!document.body) {
      try { document.addEventListener('DOMContentLoaded', render, { once: true }); } catch (e) {}
      return;
    }

    var busy = isBusy();
    var box = document.getElementById(BAR_ID);
    if (!box) {
      box = document.createElement('div');
      box.id = BAR_ID;
      box.setAttribute('role', 'status');
      box.setAttribute('style', [
        'position:fixed', 'right:18px', 'bottom:18px', 'z-index:2147483600',
        'max-width:340px', 'box-sizing:border-box', 'padding:14px 16px',
        'border-radius:12px', 'border:1px solid rgba(255,255,255,.14)',
        'background:#161a23', 'color:#e8ecf5', 'box-shadow:0 12px 32px rgba(0,0,0,.45)',
        'font:13px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Arial,sans-serif'
      ].join(';'));
      document.body.appendChild(box);
    }

    var title = document.createElement('div');
    title.setAttribute('style', 'font-weight:600;margin-bottom:4px;');
    title.textContent = 'Güncelleme hazır (v' + pendingVersion + ')';

    var note = document.createElement('div');
    note.setAttribute('style', 'opacity:.75;margin-bottom:12px;');
    if (manualMode) {
      note.textContent = 'Otomatik güncelleme bu kurulumda uygulanamıyor; yeni sürümü indirip kurun.';
    } else if (confirmArmed) {
      note.textContent = 'Görüşme veya doldurulmuş form var. Uygulama yeniden başlatılacak — devam edilsin mi?';
    } else if (busy) {
      note.textContent = 'Şu an meşgulsünüz. Uygun olduğunuzda uygulayabilirsiniz.';
    } else {
      note.textContent = 'Uygulanması birkaç saniye sürer; kaldığınız sayfaya geri dönersiniz.';
    }

    var row = document.createElement('div');
    row.setAttribute('style', 'display:flex;gap:8px;justify-content:flex-end;');

    var later = document.createElement('button');
    later.type = 'button';
    later.textContent = 'Sonra';
    later.setAttribute('style', 'cursor:pointer;padding:7px 12px;border-radius:8px;border:1px solid rgba(255,255,255,.18);background:transparent;color:inherit;font:inherit;');
    later.onclick = function () {
      dismiss(pendingVersion);
      remove();
    };

    var now = document.createElement('button');
    now.type = 'button';
    now.setAttribute('style', 'cursor:pointer;padding:7px 12px;border-radius:8px;border:0;background:#5443D2;color:#fff;font:inherit;font-weight:600;');

    if (manualMode) {
      // KALICI kurulum hatasi: tekrar denemek ise yaramaz -> sistem tarayicisinda
      // indirme sayfasini ac (Rust dogru <app>-<surum>-<platform> baglantisini kurar).
      now.textContent = 'İndirme sayfasını aç';
      now.onclick = function () {
        invoke('bogahost_open_download', {}).catch(function () {});
      };
    } else {
      now.textContent = confirmArmed ? 'Yine de uygula' : 'Şimdi uygula';
      now.onclick = function () {
        // MESGULKEN tek tikla yeniden baslatilmaz: once ne olacagi soylenir.
        if (isBusy() && !confirmArmed) {
          confirmArmed = true;
          render();
          return;
        }
        now.disabled = true;
        now.textContent = 'Uygulanıyor…';
        invoke('bogahost_apply_update', {}).catch(function () {
          // Uygulanamadi (imzasiz macOS / installer hatasi): "Daha sonra yeniden
          // deneyin" YANILTICIDIR (kalici). Manuel indirmeye gec.
          manualMode = true;
          render();
        });
      };
    }

    row.appendChild(later);
    row.appendChild(now);

    box.textContent = '';
    box.appendChild(title);
    box.appendChild(note);
    box.appendChild(row);
  }

  // Rust tarafi cagirir: guncelleme indirildi/kuruldu, onay bekleniyor.
  window.__bogahostUpdateReady = function (version, force) {
    pendingVersion = String(version || '');
    if (!pendingVersion) { remove(); return; }
    manualMode = false;
    if (force === true) {
      // Elle denetim: "Sonra" karari yok sayilir.
      try { window.sessionStorage.removeItem(dismissKey(pendingVersion)); } catch (e) {}
      confirmArmed = false;
    }
    render();
  };

  // Rust tarafi cagirir: otomatik kurulum KALICI basarisiz (imzasiz macOS) —
  // serit "İndirme sayfasını aç" (manuel indirme) moduna gecer.
  window.__bogahostUpdateManual = function (version) {
    pendingVersion = String(version || '');
    if (!pendingVersion) { remove(); return; }
    manualMode = true;
    confirmArmed = false;
    // "Sonra" demis olsa da manuel indirme onemlidir: karari sifirla.
    try { window.sessionStorage.removeItem(dismissKey(pendingVersion)); } catch (e) {}
    render();
  };

  // Gezinmeden sonra Rust seridi yeniden cizdirir; yine de sayfa kendi
  // basina da sorabilsin (SPA yeniden cizimleri icin).
  try {
    invoke('bogahost_update_state', {}).then(function (v) {
      if (v) { window.__bogahostUpdateReady(v, false); }
    }).catch(function () {});
  } catch (e) {}

  // Ilk durum bildirimi (mesgul degiliz demek de bilgidir).
  try { report(); } catch (e) {}
})();
"#;

/// Yukleme katmanini kaldiran betik (Rust tarafindan cagrilir).
const HIDE_OVERLAY_SCRIPT: &str = r#"
try { window.__bogahostLoadingHide && window.__bogahostLoadingHide(); } catch (e) {}
"#;

/// Acilis + uygulama gecisi yukleme katmani.
///
/// KOK NEDEN / TASARIM: onceki iki yaklasim da calismadi —
///  1) Eski sayfaya `eval` ile DOM katmani eklemek: `navigate` belgeyi yikinca
///     katman da yok oluyordu (ve `evaluate_script` asenkron oldugu icin cogu
///     zaman hic calismiyordu bile).
///  2) Ayri `splash` PENCERESI: her gecisde sifirdan webview kuruluyor, ana
///     pencerenin `PageLoadEvent::Finished` olayi cogu zaman pencere BOYANMADAN
///     once gelip pencereyi yok ediyordu.
///
/// Bu betik `initialization_script` ile enjekte edilir: HER ust duzey
/// gezinmede, HEDEF sayfada, sayfanin kendi icerigi/betikleri calismadan ONCE
/// (document-start) calisir. Katman eski sayfaya degil YENI sayfaya cizildigi
/// icin navigasyondan etkilenmez.
///
/// `__BOGAHOST_APPS_JSON__` yer tutucusu `loading_overlay_script()` icinde
/// doldurulur.
const LOADING_OVERLAY_JS: &str = r#"
(function () {
  // `initialization_script` Windows'ta (WebView2) ALT CERCEVELERE de enjekte
  // edilir. Katman YALNIZCA en ust cercevede cizilmelidir.
  try {
    if (window.top !== window.self) { return; }
  } catch (eTop) { return; }

  if (window.__BOGAHOST_LOADING__) { return; }
  window.__BOGAHOST_LOADING__ = true;

  // { hostname: { t: tam ad, s: kisa ad, a: vurgu rengi, g: gecis metni } }
  var APPS = __BOGAHOST_APPS_JSON__;

  var BAR_ID = '__bogahost_loading_bar__';
  var FULL_ID = '__bogahost_loading_full__';

  var BRAND = '#5443D2';
  var DARK_BG = '#0e1015';
  var DARK_FG = '#eef0f6';
  var DARK_MUTED = '#8b93a4';
  var LIGHT_BG = '#f6f7fb';
  var LIGHT_FG = '#171a22';
  var LIGHT_MUTED = '#6b7280';

  // Hiz once: giris 180ms, cikis 150ms.
  var BAR_DELAY = 140;
  var FADE_IN = 180;
  var FADE_OUT = 150;
  var HARD_CAP = 15000;
  var EASE_IN = 'cubic-bezier(.22,.61,.36,1)';
  var EASE_OUT = 'cubic-bezier(.4,0,.2,1)';

  function media(q) {
    try { return !!(window.matchMedia && window.matchMedia(q).matches); } catch (e) { return false; }
  }

  var reduce = media('(prefers-reduced-motion: reduce)');
  var light = media('(prefers-color-scheme: light)');

  var BG = light ? LIGHT_BG : DARK_BG;
  var FG = light ? LIGHT_FG : DARK_FG;
  var MUTED = light ? LIGHT_MUTED : DARK_MUTED;

  var barEl = null;
  var fullEl = null;
  var statusEl = null;
  var statusText = 'Bağlanılıyor…';
  var pinned = false;
  var barTimer = null;
  var capTimer = null;
  var bodyTimer = null;
  var tries = 0;
  var done = false;
  var anims = [];

  function info() {
    var h = '';
    try { h = String(location.hostname || '').toLowerCase(); } catch (e) {}
    return APPS[h] || null;
  }

  function accent() {
    var i = info();
    return (i && i.a) ? i.a : BRAND;
  }

  function shortName() {
    var i = info();
    if (i && i.s) { return i.s; }
    if (window.__BOGAHOST_APP_TITLE__) {
      return String(window.__BOGAHOST_APP_TITLE__).replace('Bogahost ', '');
    }
    return 'Bogahost';
  }

  function root() {
    return document.documentElement || document.body || null;
  }

  function play(el, frames, opts) {
    if (reduce) { return null; }
    try {
      if (el && typeof el.animate === 'function') {
        var a = el.animate(frames, opts);
        anims.push(a);
        return a;
      }
    } catch (e) {}
    return null;
  }

  function stopAnims() {
    for (var i = 0; i < anims.length; i++) {
      try { anims[i].cancel(); } catch (e) {}
    }
    anims = [];
  }

  function drop(el) {
    try {
      if (el && el.parentNode) { el.parentNode.removeChild(el); }
    } catch (e) {}
  }

  function join(parts) {
    return parts.join(';') + ';';
  }

  // ---- Durum metni: GERCEK asamalara bagli ----------------------------------

  function setStatus(text) {
    if (pinned) { return; }
    statusText = text;
    try {
      if (statusEl) { statusEl.textContent = text; }
    } catch (e) {}
  }

  // ---- Ince ust cubuk: siradan sayfa gezinmeleri ----------------------------

  function buildBar() {
    var wrap = document.createElement('div');
    wrap.id = BAR_ID;
    wrap.style.cssText = join([
      'position:fixed', 'top:0', 'left:0', 'right:0', 'height:3px',
      'width:100%', 'margin:0', 'padding:0',
      'z-index:2147483646', 'pointer-events:none', 'overflow:hidden',
      'background:rgba(84,67,210,.16)',
      'opacity:0',
      reduce ? 'transition:none' : ('transition:opacity ' + FADE_IN + 'ms ' + EASE_IN)
    ]);

    var fill = document.createElement('div');
    if (reduce) {
      fill.style.cssText = join([
        'position:absolute', 'top:0', 'left:0', 'height:100%', 'width:100%',
        'opacity:.6', 'background:' + accent()
      ]);
    } else {
      fill.style.cssText = join([
        'position:absolute', 'top:0', 'left:0', 'height:100%', 'width:35%',
        'will-change:transform',
        'background:linear-gradient(90deg,rgba(84,67,210,0),' + BRAND + ',' + accent() + ',rgba(84,67,210,0))'
      ]);
      play(fill, [
        { transform: 'translateX(-100%)' },
        { transform: 'translateX(300%)' }
      ], { duration: 1050, iterations: Infinity, easing: 'linear' });
    }
    wrap.appendChild(fill);
    return wrap;
  }

  function showBar() {
    if (done || barEl || fullEl) { return; }
    var r = root();
    if (!r) {
      tries = tries + 1;
      if (tries < 60) { setTimeout(showBar, 16); }
      return;
    }
    var el = buildBar();
    try { r.appendChild(el); } catch (e) { return; }
    barEl = el;
    raise(el);
  }

  function raise(el) {
    if (reduce) {
      try { el.style.opacity = '1'; } catch (e) {}
      return;
    }
    setTimeout(function () {
      try { el.style.opacity = '1'; } catch (e) {}
    }, 16);
  }

  // ---- Tam ekran acilis / gecis katmani -------------------------------------

  function buildFull() {
    var acc = accent();

    var wrap = document.createElement('div');
    wrap.id = FULL_ID;
    wrap.style.cssText = join([
      'position:fixed', 'top:0', 'left:0', 'right:0', 'bottom:0',
      'width:100%', 'height:100%', 'margin:0', 'padding:0',
      'z-index:2147483647',
      'display:flex', 'align-items:center', 'justify-content:center',
      'background:' + BG,
      'color:' + FG,
      'font:400 15px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif',
      '-webkit-user-select:none', 'user-select:none',
      'opacity:0',
      reduce ? 'transition:none' : ('transition:opacity ' + FADE_IN + 'ms ' + EASE_IN)
    ]);

    // Yumusak marka parlamasi (arka plan) — transform/opacity disi bir sey animasyon YOK.
    var glow = document.createElement('div');
    glow.style.cssText = join([
      'position:absolute', 'top:50%', 'left:50%',
      'width:620px', 'height:620px', 'margin:-310px 0 0 -310px',
      'border-radius:50%', 'pointer-events:none',
      'opacity:' + (light ? '.10' : '.16'),
      'background:radial-gradient(circle,' + acc + ' 0%,rgba(0,0,0,0) 62%)'
    ]);
    wrap.appendChild(glow);

    var box = document.createElement('div');
    box.style.cssText = join([
      'position:relative', 'text-align:center',
      'padding:32px 40px', 'max-width:90%'
    ]);

    // 1) Belirgin uygulama isareti
    var mark = document.createElement('div');
    mark.style.cssText = join([
      'width:82px', 'height:82px', 'margin:0 auto 22px',
      'border-radius:22px',
      'background:linear-gradient(145deg,' + BRAND + ',' + acc + ')',
      'box-shadow:0 0 0 1px rgba(255,255,255,.07) inset,0 18px 44px rgba(84,67,210,.34)',
      'display:flex', 'align-items:center', 'justify-content:center',
      'will-change:transform,opacity'
    ]);
    var letter = document.createElement('span');
    letter.style.cssText = join([
      'font-size:38px', 'font-weight:700', 'color:#fff', 'line-height:1',
      'letter-spacing:-.5px'
    ]);
    letter.textContent = 'B';
    mark.appendChild(letter);

    // 2) Uygulama adi
    var name = document.createElement('div');
    name.style.cssText = join([
      'font-size:26px', 'font-weight:650', 'letter-spacing:-.3px', 'line-height:1.2'
    ]);
    name.textContent = shortName();

    // 3) Marka imzasi
    var brand = document.createElement('div');
    brand.style.cssText = join([
      'margin-top:6px', 'font-size:13px', 'letter-spacing:.9px',
      'text-transform:lowercase', 'color:' + MUTED
    ]);
    brand.textContent = 'bogahost.com';

    // 4) Durum metni
    var status = document.createElement('div');
    status.style.cssText = join([
      'margin-top:24px', 'font-size:13px', 'color:' + MUTED, 'min-height:18px'
    ]);
    status.textContent = statusText;
    statusEl = status;

    // 5) Ince ilerleme cubugu
    var track = document.createElement('div');
    track.style.cssText = join([
      'position:relative', 'width:210px', 'height:3px', 'margin:14px auto 0',
      'border-radius:99px', 'overflow:hidden',
      'background:' + (light ? 'rgba(23,26,34,.10)' : 'rgba(238,240,246,.12)')
    ]);
    var fill = document.createElement('div');
    if (reduce) {
      fill.style.cssText = join([
        'position:absolute', 'top:0', 'left:0', 'height:100%', 'width:100%',
        'border-radius:99px', 'opacity:.55', 'background:' + acc
      ]);
    } else {
      fill.style.cssText = join([
        'position:absolute', 'top:0', 'left:0', 'height:100%', 'width:42%',
        'border-radius:99px', 'will-change:transform',
        'background:linear-gradient(90deg,rgba(84,67,210,0),' + BRAND + ',' + acc + ',rgba(84,67,210,0))'
      ]);
    }
    track.appendChild(fill);

    // 6) Surum etiketi
    var ver = document.createElement('div');
    ver.style.cssText = join([
      'position:absolute', 'left:0', 'right:0', 'bottom:18px',
      'text-align:center', 'pointer-events:none',
      'font-size:11px', 'letter-spacing:.4px',
      'color:' + (light ? 'rgba(23,26,34,.38)' : 'rgba(238,240,246,.32)')
    ]);
    var v = '';
    try { v = window.__BOGAHOST_NATIVE_VERSION__ ? String(window.__BOGAHOST_NATIVE_VERSION__) : ''; } catch (e) {}
    ver.textContent = v ? ('v' + v) : '';

    box.appendChild(mark);
    box.appendChild(name);
    box.appendChild(brand);
    box.appendChild(status);
    box.appendChild(track);
    wrap.appendChild(box);
    wrap.appendChild(ver);

    if (!reduce) {
      play(fill, [
        { transform: 'translateX(-100%)' },
        { transform: 'translateX(240%)' }
      ], { duration: 1100, iterations: Infinity, easing: 'linear' });

      play(mark, [
        { transform: 'scale(.86)', opacity: 0 },
        { transform: 'scale(1)', opacity: 1 }
      ], { duration: 260, easing: EASE_IN, fill: 'both' });

      play(box, [
        { transform: 'translateY(8px)', opacity: 0 },
        { transform: 'translateY(0)', opacity: 1 }
      ], { duration: 240, delay: 40, easing: EASE_IN, fill: 'both' });
    }

    return wrap;
  }

  // mode: 'boot' | 'switch'
  function showFull(mode) {
    if (done) { return; }
    if (mode === 'switch') {
      var i = info();
      if (i && i.g) {
        statusText = i.g;
        pinned = true;
        try { if (statusEl) { statusEl.textContent = statusText; } } catch (e) {}
      }
    }
    if (fullEl) { return; }
    var r = root();
    if (!r) {
      tries = tries + 1;
      if (tries < 60) { setTimeout(function () { showFull(mode); }, 16); }
      return;
    }
    var el = buildFull();
    try { r.appendChild(el); } catch (e) { return; }
    fullEl = el;
    try { clearTimeout(barTimer); } catch (e) {}
    if (barEl) { drop(barEl); barEl = null; }
    raise(el);
  }

  // ---- Kaldirma: sayfa hazir olur olmaz, capraz gecisle --------------------

  function fadeOut(el) {
    if (!el) { return; }
    if (reduce) { drop(el); return; }
    try {
      el.style.transition = 'opacity ' + FADE_OUT + 'ms ' + EASE_OUT;
      el.style.opacity = '0';
    } catch (e) {
      drop(el);
      return;
    }
    setTimeout(function () { drop(el); }, FADE_OUT + 60);
  }

  function hide() {
    if (done) { return; }
    done = true;
    try { clearTimeout(barTimer); } catch (e) {}
    try { clearTimeout(capTimer); } catch (e) {}
    try { clearTimeout(bodyTimer); } catch (e) {}
    stopAnims();
    var b = barEl;
    var f = fullEl;
    barEl = null;
    fullEl = null;
    statusEl = null;
    fadeOut(b);
    fadeOut(f);
  }

  window.__bogahostLoadingFull = showFull;
  window.__bogahostLoadingHide = hide;

  // Rust cagirir (acilis otomatik guncellemesi): katman gorunurken durum
  // metnini sabitler ("Güncelleniyor…"). Katman zaten kapandiysa zararsizca
  // yok sayilir (guncelleme yeniden baslatmayla bitecegi icin kisa surelidir).
  window.__bogahostLoadingStatus = function (t) {
    try {
      if (done) { return; }
      pinned = true;
      statusText = String(t == null ? statusText : t);
      showFull('boot');
      if (statusEl) { statusEl.textContent = statusText; }
    } catch (e) {}
  };

  // ---- Uygulama GECISI tespiti: DETERMINISTIK, eval yarisindan BAGIMSIZ -------
  //
  // KOK NEDEN (v1.9.x'e kadar splash'in gorunmemesi): tam ekran gecis katmani
  // yalnizca Rust->JS `eval` (`wake_overlay`) ile uyandiriliyordu; bu eval
  // navigasyondan sonra hedef belgeye YETISEMEYIP cogu zaman bosa dusuyordu ve
  // panel-ici "Uygulamalar" menusu gecislerinde HIC cagrilmiyordu (o linkler
  // ana pencerede degil onizleme penceresinde aciliyordu). Sonuc: gecislerde
  // ekranda yalnizca ince cubuk ya da hicbir sey gorunuyordu.
  //
  // Cozum: hangi uygulamadan gelindigini SAYFANIN KENDISI belirler. Paylasilan
  // `.bogahost.com` cerezi son ziyaret edilen kardes uygulamayi tutar; hedef
  // sayfa document-start'ta onu okur. Onceki uygulama su ankinden FARKLIYSA (ya
  // da cerez bossa = ilk acilis) bu bir GECIS/ACILISTIR -> tam ekran splash
  // HEMEN cizilir. Ayni uygulama icindeki gezinmelerde yalnizca ince ust cubuk
  // gorunur. Bu yol menu-cubugu VE panel-ici link gecislerinin IKISINDE de,
  // hicbir zamanlama yarisi olmadan calisir. (`wake_overlay` artik yalnizca
  // zararsiz bir pekistirmedir; `showFull` idempotenttir.)
  var COOKIE = '__bgh_last_app';
  function readCookie(name) {
    try {
      var m = String(document.cookie || '').match(new RegExp('(?:^|; )' + name + '=([^;]*)'));
      return m ? decodeURIComponent(m[1]) : '';
    } catch (e) { return ''; }
  }
  function writeCookie(name, val) {
    try {
      var host = String(location.hostname || '').toLowerCase();
      // Kardes alt alan adlarinin PAYLASMASI icin kok alan adina yazilir.
      var dom = host.indexOf('bogahost.com') >= 0 ? '; domain=.bogahost.com' : '';
      document.cookie = name + '=' + encodeURIComponent(val) + '; path=/' + dom + '; max-age=1800; samesite=lax';
    } catch (e) {}
  }
  var curHost = '';
  try { curHost = String(location.hostname || '').toLowerCase(); } catch (e) {}
  var isApp = !!(APPS && APPS[curHost]);
  var autoMode = null;
  if (isApp) {
    var prevApp = readCookie(COOKIE);
    if (prevApp !== curHost) { autoMode = prevApp ? 'switch' : 'boot'; }
    // Bir sonraki sicrama icin "son uygulama"yi guncelle.
    writeCookie(COOKIE, curHost);
  }

  if (autoMode) {
    // Gecis/acilis: tam ekran splash HEMEN (ince cubuk atlanir).
    showFull(autoMode);
  } else {
    // Ayni uygulama ici gezinme: yalnizca ince ust cubuk.
    barTimer = setTimeout(showBar, BAR_DELAY);
  }
  capTimer = setTimeout(hide, HARD_CAP);

  // Asama 2: govde gelmeye basladi -> "Yükleniyor…"
  function watchBody() {
    if (done) { return; }
    if (document.body) { setStatus('Yükleniyor…'); return; }
    bodyTimer = setTimeout(watchBody, 32);
  }
  watchBody();

  // Asama 3: DOM hazir -> "Hazırlanıyor…"
  function onReady() {
    setStatus('Hazırlanıyor…');
    setTimeout(hide, 350);
  }

  function onLoad() { setTimeout(hide, 40); }

  try {
    if (document.readyState === 'complete') {
      onLoad();
    } else {
      window.addEventListener('load', onLoad);
      if (document.readyState === 'interactive') {
        onReady();
      } else {
        document.addEventListener('DOMContentLoaded', onReady);
      }
    }
  } catch (e) {
    setTimeout(hide, 3000);
  }
})();
"#;

/// `OVERLAY_APPS` tablosunu katmanin bekledigi JS nesnesine cevirir.
fn overlay_apps_json() -> String {
    let mut out = String::from("{");
    for (i, (host, short, accent, going)) in OVERLAY_APPS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        // `{:?}` tirnaklari/kacislari kendisi ekler — gecerli JS dize sabiti uretir.
        out.push_str(&format!(
            "{:?}:{{t:{:?},s:{:?},a:{:?},g:{:?}}}",
            host,
            format!("Bogahost {short}"),
            short,
            accent,
            going
        ));
    }
    out.push('}');
    out
}

/// Katman betigini uygulama tablosuyla birlikte uretir.
fn loading_overlay_script() -> String {
    LOADING_OVERLAY_JS.replace("__BOGAHOST_APPS_JSON__", &overlay_apps_json())
}

/// Onizleme (popup) penceresine EK olarak enjekte edilir.
///
/// Pencerede zaten baslik cubugu + kapat dugmesi vardir; buna ek olarak
/// ESC tusu ve sag ustte belirgin bir "Kapat" dugmesi sunulur — kullanici
/// acilan PDF/CSV/gorsel icinde ASLA kilitli kalmasin.
const POPUP_INIT_SCRIPT: &str = r#"
(function () {
  if (window.__BOGAHOST_POPUP__) { return; }
  window.__BOGAHOST_POPUP__ = true;

  function closeSelf() {
    try {
      var t = window.__TAURI__;
      if (t && t.core && typeof t.core.invoke === 'function') { t.core.invoke('bogahost_close_window', {}); return; }
      if (t && typeof t.invoke === 'function') { t.invoke('bogahost_close_window', {}); return; }
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
        window.__TAURI_INTERNALS__.invoke('bogahost_close_window', {});
        return;
      }
    } catch (e) {}
    try { window.close(); } catch (e2) {}
  }

  document.addEventListener('keydown', function (ev) {
    if (ev.key === 'Escape' || ev.keyCode === 27) { closeSelf(); }
  }, true);

  function printSelf() {
    try {
      var t = window.__TAURI__;
      if (t && t.core && typeof t.core.invoke === 'function') { t.core.invoke('bogahost_print', {}); return; }
      if (t && typeof t.invoke === 'function') { t.invoke('bogahost_print', {}); return; }
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
        window.__TAURI_INTERNALS__.invoke('bogahost_print', {});
        return;
      }
    } catch (e) {}
    try { (window.__bogahostNativePrint || window.print).call(window); } catch (e2) {}
  }

  function styleButton(b, right) {
    b.setAttribute('style', 'position:fixed;top:12px;right:' + right + 'px;z-index:2147483647;background:#5443D2;color:#fff;border:0;border-radius:8px;padding:8px 14px;font:13px -apple-system,"Segoe UI",Roboto,Arial,sans-serif;cursor:pointer;box-shadow:0 4px 14px rgba(0,0,0,.35);opacity:.92;');
  }

  function addCloseButton() {
    try {
      if (document.getElementById('bogahost-popup-close')) { return; }
      var host = document.body || document.documentElement;
      if (!host) { return; }

      var p = document.createElement('button');
      p.id = 'bogahost-popup-print';
      p.type = 'button';
      p.textContent = 'Yazdır';
      styleButton(p, 122);
      p.onclick = printSelf;
      host.appendChild(p);

      var b = document.createElement('button');
      b.id = 'bogahost-popup-close';
      b.type = 'button';
      b.textContent = 'Kapat (ESC)';
      styleButton(b, 14);
      b.onclick = closeSelf;
      host.appendChild(b);
    } catch (e) {}
  }

  // PDF/gorsel gibi icerikte `document.body` olmayabilir; birkac kez denenir.
  var tries = 0;
  var timer = setInterval(function () {
    tries++;
    addCloseButton();
    if (tries >= 8 || document.getElementById('bogahost-popup-close')) { clearInterval(timer); }
  }, 400);
  addCloseButton();
})();
"#;

/// Uygulama gecisinden SONRA calisir: hedef uygulama erisim engeli (403/401)
/// donuyorsa "Bu uygulamaya geçiş izniniz yok" ekranini gosterir.
/// Kopru henuz hazir degilse kisa bir gecikmeyle yeniden dener.
const ACCESS_CHECK_SCRIPT: &str = r#"
(function () {
  try {
    var tries = 0;
    var timer = setInterval(function () {
      tries++;
      if (typeof window.__bogahostAccessCheck === 'function') {
        clearInterval(timer);
        window.__bogahostAccessCheck('switch');
      } else if (tries >= 10) {
        clearInterval(timer);
      }
    }, 200);
  } catch (e) {}
})();
"#;

// ---------------------------------------------------------------------------
// Indirmeler
// ---------------------------------------------------------------------------

/// Sayfa tarafindan uretilen (blob/data URL) dosyalari diske yazan kopru.
/// Yalnizca kullanicinin **Indirilenler** klasorune yazar.
#[tauri::command]
fn bogahost_save_file(
    app: AppHandle,
    name: Option<String>,
    b64: String,
    open_after: Option<bool>,
) -> Result<String, String> {
    let bytes = decode_base64(&b64).ok_or_else(|| "veri çözülemedi".to_string())?;
    if bytes.is_empty() {
        return Err("boş dosya".to_string());
    }
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("dosya çok büyük".to_string());
    }

    let file_name = sanitize_file_name(name.as_deref().unwrap_or(""));
    let dir = downloads_dir(&app);
    if std::fs::create_dir_all(&dir).is_err() {
        return Err("İndirilenler klasörü oluşturulamadı".to_string());
    }
    let target = unique_path(&dir, &file_name);
    std::fs::write(&target, &bytes).map_err(|e| e.to_string())?;

    remember_download(&app, &target);
    notify_download_saved(&app, &target);

    if open_after.unwrap_or(false) {
        // WebView'de gosterilemeyen turler (PDF vb.) sistem uygulamasinda acilir.
        let _ = app
            .shell()
            .open(target.to_string_lossy().to_string(), None);
    }

    Ok(target.to_string_lossy().to_string())
}

/// Sayfadaki indirme bilgi mesajinin "Klasörde göster" dugmesi.
/// Son indirilen dosyayi dosya yoneticisinde SECILI acar; kayit yoksa
/// dogrudan Indirilenler klasorunu acar.
#[tauri::command]
fn bogahost_reveal_download(app: AppHandle) -> Result<(), String> {
    match last_download(&app) {
        Some(p) => reveal_in_file_manager(&app, &p),
        None => {
            let dir = downloads_dir(&app);
            let _ = app.shell().open(dir.to_string_lossy().to_string(), None);
        }
    }
    Ok(())
}

/// YAVAS/BUYUK ic indirme uclarini (Parasut e-belge PDF gibi, dis API arkasi)
/// panel oturumuyla RUST tarafinda indirir.
///
/// KOK NEDEN: WKWebView'de `fetch()`->`blob()` yolu yavas/buyuk yanitlarda
/// GUVENILMEZ — Parasut dis API yavas olunca baglanti kopuyor ve `fetch`
/// REDDEDIYOR ("Dosya indirilemedi..."). Bu yol `reqwest` ile UZUN timeout
/// kullanir ve yaniti DOGRUDAN diske streaming yazar (bellege blob toplamaz).
///
/// CLOUDFLARE UYUMU: `cf_clearance` cerezi UA + IP'ye baglidir. Istek ayni
/// makineden (kullanicinin IP'si) gider; UA da webview'in KENDI
/// `navigator.userAgent`'idir (JS'ten `ua` ile gelir) — boylece cerez gecerli
/// kalir. Cerezler `Webview::cookies_for_url` ile alinir (httpOnly session +
/// cf_clearance dahil; bkz. tauri 2.11 `cookies()` belgesi).
///
/// Donus:
///  * `Ok(path)`            -> diske yazildi.
///  * `Ok("__NAVIGATE__")`  -> yanit HTML sayfasi; cagiran taraf normal gezinsin
///                             (yalnizca `allow_navigate = true` iken).
///  * `Err(code)`           -> `"status:404"` | `"status:500"` | `"timeout"`
///                             | `"network"` | `"empty"` | `"io"`
///                             (JS anlasilir mesaja cevirir).
#[tauri::command]
async fn bogahost_fetch_download(
    app: AppHandle,
    window: tauri::WebviewWindow<Wry>,
    url: String,
    name: Option<String>,
    ua: Option<String>,
    cookie: Option<String>,
    open_after: Option<bool>,
    allow_navigate: Option<bool>,
) -> Result<String, String> {
    let parsed = Url::parse(&url).map_err(|_| "network".to_string())?;
    // GUVENLIK: yalnizca kendi alan adimizdan indirilir (harici adres bu yoldan gecmez).
    if !is_internal_url(&parsed) {
        return Err("network".to_string());
    }

    // Webview oturum cerezleri (httpOnly session + cf_clearance dahil).
    // Rust tarafi bos donerse (nadir), JS'ten gelen `document.cookie` yedegi.
    let mut cookie_header = cookie_header_for(&window, &parsed);
    if cookie_header.is_empty() {
        if let Some(c) = cookie {
            if !c.trim().is_empty() {
                cookie_header = c;
            }
        }
    }

    let ua = ua.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
        format!("BogahostNative/{} ({})", env!("CARGO_PKG_VERSION"), APP_KEY)
    });

    let client = reqwest::Client::builder()
        .connect_timeout(NETWORK_TIMEOUT)
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent(ua)
        .build()
        .map_err(|_| "network".to_string())?;

    let mut req = client.get(parsed.clone());
    if !cookie_header.is_empty() {
        req = req.header(reqwest::header::COOKIE, cookie_header);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return Err(if e.is_timeout() { "timeout" } else { "network" }.to_string());
        }
    };

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("status:{}", status.as_u16()));
    }

    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let cd = resp
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // HTML sayfasi (attachment DEGIL) ise indirme; cagiran taraf gezinsin.
    let is_attachment = cd.to_ascii_lowercase().contains("attachment");
    let is_html = (ct.contains("text/html") || ct.contains("xhtml")) && !is_attachment;
    if is_html && allow_navigate.unwrap_or(false) {
        return Ok("__NAVIGATE__".to_string());
    }

    // Dosya adi: once Content-Disposition, sonra JS'ten gelen ad, sonra URL.
    let base_name = disposition_filename(&cd)
        .or(name)
        .unwrap_or_else(|| file_name_from_url(&parsed));
    let file_name = ensure_extension(sanitize_file_name(&base_name), &ct);

    let dir = downloads_dir(&app);
    if std::fs::create_dir_all(&dir).is_err() {
        return Err("io".to_string());
    }
    let target = unique_path(&dir, &file_name);

    // Streaming: yaniti parca parca diske yaz (bellege blob toplamadan).
    let mut file = std::fs::File::create(&target).map_err(|_| "io".to_string())?;
    let mut resp = resp;
    let mut total: u64 = 0;
    loop {
        match resp.chunk().await {
            Ok(Some(bytes)) => {
                use std::io::Write;
                if file.write_all(bytes.as_ref()).is_err() {
                    drop(file);
                    let _ = std::fs::remove_file(&target);
                    return Err("io".to_string());
                }
                total += bytes.len() as u64;
            }
            Ok(None) => break,
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(&target);
                return Err(if e.is_timeout() { "timeout" } else { "network" }.to_string());
            }
        }
    }
    drop(file);

    if total == 0 {
        let _ = std::fs::remove_file(&target);
        return Err("empty".to_string());
    }

    remember_download(&app, &target);
    notify_download_saved(&app, &target);
    if open_after.unwrap_or(false) {
        let _ = app.shell().open(target.to_string_lossy().to_string(), None);
    }
    Ok(target.to_string_lossy().to_string())
}

/// Webview cerez deposundan verilen URL icin `Cookie:` basligi olusturur
/// (httpOnly session + cf_clearance dahil — bkz. `Webview::cookies_for_url`).
fn cookie_header_for(window: &tauri::WebviewWindow<Wry>, url: &Url) -> String {
    match window.cookies_for_url(url.clone()) {
        Ok(cookies) => cookies
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; "),
        Err(_) => String::new(),
    }
}

/// `Content-Disposition` basligindaki dosya adini cozer (RFC5987 `filename*`
/// oncelikli). Bulunamazsa `None`.
fn disposition_filename(cd: &str) -> Option<String> {
    let lower = cd.to_ascii_lowercase();
    // filename*=UTF-8''ad%20.pdf
    if let Some(idx) = lower.find("filename*=") {
        let rest = &cd[idx + "filename*=".len()..];
        let val = rest.split(';').next().unwrap_or("").trim().trim_matches('"');
        let name = val.rsplit("''").next().unwrap_or(val);
        let cleaned = sanitize_file_name(&percent_decode(name));
        if !cleaned.is_empty() && cleaned != "indirilen-dosya" {
            return Some(cleaned);
        }
    }
    if let Some(idx) = lower.find("filename=") {
        let rest = &cd[idx + "filename=".len()..];
        let val = rest.split(';').next().unwrap_or("").trim().trim_matches('"');
        let cleaned = sanitize_file_name(val);
        if !cleaned.is_empty() && cleaned != "indirilen-dosya" {
            return Some(cleaned);
        }
    }
    None
}

/// Dosya adinda uzanti yoksa `Content-Type`'a gore ekler.
fn ensure_extension(name: String, content_type: &str) -> String {
    if name.contains('.') {
        return name;
    }
    let ext = if content_type.contains("pdf") {
        ".pdf"
    } else if content_type.contains("csv") {
        ".csv"
    } else if content_type.contains("spreadsheet") || content_type.contains("excel") {
        ".xlsx"
    } else if content_type.contains("zip") {
        ".zip"
    } else if content_type.contains("json") {
        ".json"
    } else if content_type.contains("png") {
        ".png"
    } else if content_type.contains("jpeg") {
        ".jpg"
    } else {
        ""
    };
    format!("{name}{ext}")
}

/// Kullanicinin Indirilenler klasoru (bulunamazsa ev dizini / gecici klasor).
fn downloads_dir(app: &AppHandle) -> PathBuf {
    if let Ok(dir) = app.path().download_dir() {
        return dir;
    }
    if let Ok(home) = app.path().home_dir() {
        return home.join("Downloads");
    }
    std::env::temp_dir()
}

/// Dosya adini guvenli hale getirir (dizin ayraclari ve kontrol karakterleri atilir).
fn sanitize_file_name(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('.').trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push('-'),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    let out = out.trim().to_string();
    let out = if out.chars().count() > 120 {
        out.chars().take(120).collect::<String>()
    } else {
        out
    };
    if out.is_empty() {
        "indirilen-dosya".to_string()
    } else {
        out
    }
}

/// URL'den makul bir dosya adi cikarir.
fn file_name_from_url(url: &Url) -> String {
    let candidate = url
        .path_segments()
        .and_then(|mut s| s.next_back())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let decoded = percent_decode(&candidate);
    sanitize_file_name(&decoded)
}

/// `%20` gibi kacislari cozer (harici bagimlilik olmadan).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Ayni adda dosya varsa "ad-1.pdf", "ad-2.pdf" ... uretir.
fn unique_path(dir: &Path, file_name: &str) -> PathBuf {
    let as_path = Path::new(file_name);
    let stem = as_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("indirilen-dosya")
        .to_string();
    let ext = as_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();

    let mut candidate = dir.join(format!("{stem}{ext}"));
    let mut counter = 1;
    while candidate.exists() && counter < 1000 {
        candidate = dir.join(format!("{stem}-{counter}{ext}"));
        counter += 1;
    }
    candidate
}

fn remember_download(app: &AppHandle, path: &Path) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut slot) = state.last_download.lock() {
            *slot = Some(path.to_path_buf());
        }
    }
}

fn last_download(app: &AppHandle) -> Option<PathBuf> {
    app.try_state::<AppState>()
        .and_then(|s| s.last_download.lock().ok().and_then(|p| p.clone()))
}

/// Indirme bildirimi — dosyanin TAM KONUMU gosterilir.
/// (Kullanici "nereye indirdi?" diye aramasin: hem bildirimde hem sayfada yazar.)
fn notify_download_saved(app: &AppHandle, path: &Path) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("dosya")
        .to_string();
    let folder = path
        .parent()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();

    notify(
        app,
        &format!("İndirildi: {name}"),
        &format!(
            "Konum: {folder}\nTepsi menüsü ▸ \"Son indirilen dosyayı göster\" ile klasörde açabilirsiniz."
        ),
    );

    // Sayfa uzerinde de gorunur geri bildirim + tek tikla "Klasörde göster".
    // Kopru hazir degilse duz metin mesajina duseriz.
    if let Some(w) = app.get_webview_window("main") {
        let js = format!(
            "try {{ if (window.__bogahostDownloadDone) {{ window.__bogahostDownloadDone({name:?}, {folder:?}); }} else if (window.__bogahostToast) {{ window.__bogahostToast(\"İndirildi: \" + {name:?}); }} }} catch (e) {{}}"
        );
        let _ = w.eval(js);
    }
}

/// Bagimlilik eklemeden base64 cozucu (standart ve URL-guvenli alfabe).
fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len() / 4 * 3 + 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for ch in input.chars() {
        let value = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            '=' => break,
            c if c.is_whitespace() => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Menuler
// ---------------------------------------------------------------------------

/// Kisayolu olan menu ogesi. Kisayol dizesi ayristirilamazsa oge KISAYOLSUZ
/// olusturulur — menu kurulumu asla basarisiz olmaz (uygulama acilmama riski yok).
fn menu_item(
    app: &AppHandle,
    id: &str,
    label: &str,
    accelerator: Option<&str>,
) -> tauri::Result<MenuItem<Wry>> {
    match MenuItem::with_id(app, id, label, true, accelerator) {
        Ok(item) => Ok(item),
        Err(_) => MenuItem::with_id(app, id, label, true, None::<&str>),
    }
}

/// "Görünüm" alt menusu: Yenile, Geri/İleri, yakinlastirma, tam ekran.
fn build_view_submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let reload = menu_item(app, "view-reload", "Yenile", Some("CmdOrCtrl+R"))?;
    // Yazdirma: panellerdeki rapor/fatura ciktilari icin (bkz. `trigger_print`).
    let print = menu_item(app, "view-print", "Yazdır…", Some("CmdOrCtrl+P"))?;
    let back = menu_item(app, "view-back", "Geri", Some("CmdOrCtrl+BracketLeft"))?;
    let forward = menu_item(app, "view-forward", "İleri", Some("CmdOrCtrl+BracketRight"))?;
    // KURTARMA YOLU: pencere bir sekilde panelden koptuysa (PDF/gorsel/hata
    // sayfasi) kullanici TEK tikla panele donebilsin.
    let home = menu_item(app, "view-home", "Panele dön", Some("CmdOrCtrl+Shift+H"))?;
    let zoom_in = menu_item(app, "view-zoom-in", "Yakınlaştır", Some("CmdOrCtrl+Equal"))?;
    let zoom_out = menu_item(app, "view-zoom-out", "Uzaklaştır", Some("CmdOrCtrl+Minus"))?;
    let zoom_reset = menu_item(app, "view-zoom-reset", "Gerçek Boyut", Some("CmdOrCtrl+0"))?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    // Tam ekran ogesi macOS'a ozgudur; Windows'ta tepsi menusune eklenmez.
    #[cfg(target_os = "macos")]
    let sep3 = PredefinedMenuItem::separator(app)?;
    #[cfg(target_os = "macos")]
    let fullscreen = PredefinedMenuItem::fullscreen(app, Some("Tam Ekran"))?;

    #[allow(unused_mut)]
    let mut items: Vec<&dyn IsMenuItem<Wry>> = vec![
        &reload,
        &print,
        &sep1,
        &back,
        &forward,
        &home,
        &sep2,
        &zoom_in,
        &zoom_out,
        &zoom_reset,
    ];
    #[cfg(target_os = "macos")]
    {
        items.push(&sep3);
        items.push(&fullscreen);
    }

    let submenu = Submenu::with_items(app, "Görünüm", true, &items)?;
    drop(items);
    Ok(submenu)
}

/// "Uygulamalar" alt menusunu ve icindeki isaretlenebilir ogeleri uretir.
/// Aktif uygulama isaretli + pasif (gri) gosterilir.
fn build_apps_submenu(
    app: &AppHandle,
) -> tauri::Result<(Submenu<Wry>, Vec<(String, CheckMenuItem<Wry>)>)> {
    let mut items: Vec<(String, CheckMenuItem<Wry>)> = Vec::with_capacity(APPS.len());

    for entry in APPS.iter() {
        let key = entry.0;
        let label = entry.1;
        let is_current = key == APP_KEY;
        let item = CheckMenuItem::with_id(
            app,
            format!("{}{}", APP_MENU_PREFIX, key),
            label,
            !is_current, // aktif olan tiklanamaz
            is_current,  // ve isaretli
            None::<&str>,
        )?;
        items.push((key.to_string(), item));
    }

    let refs: Vec<&dyn IsMenuItem<Wry>> = items
        .iter()
        .map(|(_, item)| item as &dyn IsMenuItem<Wry>)
        .collect();
    let submenu = Submenu::with_items(app, "Uygulamalar", true, &refs)?;
    drop(refs);

    Ok((submenu, items))
}

/// macOS menu cubugu. Standart Uygulama / Düzen / Pencere menuleri ACIKCA
/// kuruluyor ki kopyala-yapistir (Cmd+C/V/X/A), Cmd+Q, Cmd+W, Cmd+M, Cmd+H
/// her zaman calissin.
#[cfg(target_os = "macos")]
fn build_menu_bar(app: &AppHandle) -> tauri::Result<(Menu<Wry>, Vec<(String, CheckMenuItem<Wry>)>)> {
    // --- Uygulama menusu ---
    let about_label = format!("{APP_TITLE} Hakkında");
    let hide_label = format!("{APP_TITLE} Gizle");
    let about = PredefinedMenuItem::about(app, Some(about_label.as_str()), None)?;
    // Surum, menu cubugunda da HER ZAMAN gorunur (pasif bilgi ogesi) +
    // guncelleme dugmeli kendi "Hakkında" diyalogumuz.
    let version_label = format!("Sürüm v{}", env!("CARGO_PKG_VERSION"));
    let version_i = MenuItem::with_id(
        app,
        "version-info",
        version_label.as_str(),
        false,
        None::<&str>,
    )?;
    let about_dlg = MenuItem::with_id(
        app,
        "about",
        "Hakkında ve Güncelleme…",
        true,
        None::<&str>,
    )?;
    let services = PredefinedMenuItem::services(app, Some("Hizmetler"))?;
    let hide = PredefinedMenuItem::hide(app, Some(hide_label.as_str()))?;
    let hide_others = PredefinedMenuItem::hide_others(app, Some("Diğerlerini Gizle"))?;
    let show_all = PredefinedMenuItem::show_all(app, Some("Tümünü Göster"))?;
    let quit = PredefinedMenuItem::quit(app, Some("Çıkış"))?;
    let a1 = PredefinedMenuItem::separator(app)?;
    let a2 = PredefinedMenuItem::separator(app)?;
    let a3 = PredefinedMenuItem::separator(app)?;
    let app_items: Vec<&dyn IsMenuItem<Wry>> = vec![
        &version_i,
        &about,
        &about_dlg,
        &a1,
        &services,
        &a2,
        &hide,
        &hide_others,
        &show_all,
        &a3,
        &quit,
    ];
    let app_sub = Submenu::with_items(app, APP_TITLE, true, &app_items)?;
    drop(app_items);

    // --- Düzen menusu (kopyala/yapistir/kes/tumunu sec) ---
    let undo = PredefinedMenuItem::undo(app, Some("Geri Al"))?;
    let redo = PredefinedMenuItem::redo(app, Some("Yinele"))?;
    let cut = PredefinedMenuItem::cut(app, Some("Kes"))?;
    let copy = PredefinedMenuItem::copy(app, Some("Kopyala"))?;
    let paste = PredefinedMenuItem::paste(app, Some("Yapıştır"))?;
    let select_all = PredefinedMenuItem::select_all(app, Some("Tümünü Seç"))?;
    let e1 = PredefinedMenuItem::separator(app)?;
    let edit_items: Vec<&dyn IsMenuItem<Wry>> = vec![
        &undo,
        &redo,
        &e1,
        &cut,
        &copy,
        &paste,
        &select_all,
    ];
    let edit_sub = Submenu::with_items(app, "Düzen", true, &edit_items)?;
    drop(edit_items);

    // --- Görünüm menusu ---
    let view_sub = build_view_submenu(app)?;

    // --- Uygulamalar menusu ---
    let (apps_sub, apps_items) = build_apps_submenu(app)?;

    // --- Pencere menusu ---
    let minimize = PredefinedMenuItem::minimize(app, Some("Simge Durumuna Küçült"))?;
    let maximize = PredefinedMenuItem::maximize(app, Some("Büyüt / Küçült"))?;
    let close = PredefinedMenuItem::close_window(app, Some("Pencereyi Kapat"))?;
    let w1 = PredefinedMenuItem::separator(app)?;
    let window_items: Vec<&dyn IsMenuItem<Wry>> = vec![&minimize, &maximize, &w1, &close];
    let window_sub = Submenu::with_items(app, "Pencere", true, &window_items)?;
    drop(window_items);

    let root_items: Vec<&dyn IsMenuItem<Wry>> = vec![
        &app_sub,
        &edit_sub,
        &view_sub,
        &apps_sub,
        &window_sub,
    ];
    let menu = Menu::with_items(app, &root_items)?;
    drop(root_items);

    Ok((menu, apps_items))
}

/// Tepsi ve menu cubugu olaylarinin ortak isleyicisi.
/// UZUN SUREN IS YAPILMAZ — her sey ya aninda ya da arka planda calisir.
fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }
        "hide" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
        }
        "autostart-toggle" => toggle_autostart(app),
        "quit" => {
            save_window_state_from_handle(app, true);
            app.exit(0)
        }
        "view-reload" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval("location.reload()");
            }
        }
        "view-back" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval("history.back()");
            }
        }
        "view-forward" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval("history.forward()");
            }
        }
        // Bu uygulamanin panel adresine geri don (kurtarma yolu).
        "view-home" => {
            let target = APPS
                .iter()
                .find(|e| e.0 == APP_KEY)
                .map(|e| e.2)
                .unwrap_or("https://bogahost.com/");
            if let (Some(w), Ok(url)) = (app.get_webview_window("main"), Url::parse(target)) {
                let _ = w.navigate(url);
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }
        // Menuden yazdirma ANA pencereyi (paneli) yazdirir.
        // Onizleme penceresindeki PDF icin o pencerenin kendi "Yazdır" dugmesi
        // kullanilir (bkz. `POPUP_INIT_SCRIPT` -> `bogahost_print`); boylece
        // hangi pencerenin yazdirilacagi belirsiz kalmaz.
        "view-print" => {
            if let Some(w) = app.get_webview_window("main") {
                trigger_print(&w);
            }
        }
        "view-zoom-in" => apply_zoom(app, 0.1),
        "view-zoom-out" => apply_zoom(app, -0.1),
        "view-zoom-reset" => set_zoom(app, 1.0),
        "downloads-folder" => {
            let dir = downloads_dir(app);
            let _ = app.shell().open(dir.to_string_lossy().to_string(), None);
        }
        "downloads-last" => {
            // Dosyayi klasorde SECILI gosterir (Finder / Explorer).
            match last_download(app) {
                Some(p) => reveal_in_file_manager(app, &p),
                None => {
                    let dir = downloads_dir(app);
                    let _ = app.shell().open(dir.to_string_lossy().to_string(), None);
                }
            }
        }
        "notify-status" => {
            // Windows'ta durum guvenilir okunamaz (yukaridaki nota bakin):
            // tiklama HER ZAMAN ayarlari acar.
            let granted = if cfg!(target_os = "windows") {
                false
            } else {
                notification_granted(app)
            };
            if granted {
                notify(
                    app,
                    APP_TITLE,
                    "Bildirimler açık. Panel bildirimleri masaüstünde gösteriliyor.",
                );
            } else {
                open_notification_settings(app);
            }
            let h = app.clone();
            std::thread::spawn(move || refresh_notification_menu(&h));
        }
        // Bildirime tiklama olayi olmadigi icin hedef adres BURADAN acilir.
        "notify-last-open" => {
            let target = app
                .try_state::<AppState>()
                .and_then(|st| st.last_notify_url.lock().ok().and_then(|u| u.clone()));
            match target {
                Some(u) => open_in_main_window(app, &u),
                None => page_toast(app, "Henüz açılacak bir bildirim yok."),
            }
        }
        // Pasif bilgi ogesi: tiklanamaz, yine de emniyet icin yutulur.
        "version-info" => {}
        "about" => show_about_dialog(app),
        // ELLE denetim (tepsi + macOS menu cubugu + "Hakkında" diyalogu).
        // Zaten indirilmis bir guncelleme varsa ag'a CIKMAZ: pencereyi one alip
        // seridi tekrar gosterir — kullanici "Sonra" dedikten sonra fikir
        // degistirdiginde basvuracagi yol budur.
        "check-update" => {
            if let Some(version) = pending_version() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
                show_update_banner(app, &version, true);
                return;
            }
            // Ayni olay hem tepsi hem menu cubugundan gelebilir; tek seferde tek denetim.
            if UPDATE_CHECK_RUNNING.swap(true, Ordering::SeqCst) {
                return;
            }
            let h = app.clone();
            tauri::async_runtime::spawn(async move {
                run_update_flow(h, true).await;
                UPDATE_CHECK_RUNNING.store(false, Ordering::SeqCst);
            });
        }
        "open-downloads" => {
            let mut url = DOWNLOAD_PAGE_URL.to_string();
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(slot) = state.download_url.lock() {
                    url = slot.clone();
                }
            }
            let _ = app.shell().open(url, None);
        }
        other => {
            if let Some(key) = other.strip_prefix(APP_MENU_PREFIX) {
                switch_app(app, key);
            }
        }
    }
}

/// "Hakkında" diyalogu — tepsiden ve (macOS) menu cubugundan acilir.
/// Uygulama adi + surum + "Powered by Bogahost" gosterir; "Güncellemeleri
/// denetle" dugmesi tepsi menusundeki denetimin AYNISINI calistirir.
fn show_about_dialog(app: &AppHandle) {
    let message = format!(
        "{APP_TITLE}\nSürüm v{}\n\nPowered by Bogahost\nhttps://bogahost.com",
        env!("CARGO_PKG_VERSION")
    );
    let handle = app.clone();
    app.dialog()
        .message(message)
        .title(format!("{APP_TITLE} Hakkında"))
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Güncellemeleri denetle".to_string(),
            "Kapat".to_string(),
        ))
        .show(move |check| {
            if !check {
                return;
            }
            // Ayni tekillik korumasi: ust uste denetim baslatilmaz.
            if UPDATE_CHECK_RUNNING.swap(true, Ordering::SeqCst) {
                return;
            }
            tauri::async_runtime::spawn(async move {
                run_update_flow(handle, true).await;
                UPDATE_CHECK_RUNNING.store(false, Ordering::SeqCst);
            });
        });
}

fn apply_zoom(app: &AppHandle, delta: f64) {
    let current = app
        .try_state::<AppState>()
        .and_then(|s| s.zoom.lock().ok().map(|z| *z))
        .unwrap_or(1.0);
    set_zoom(app, current + delta);
}

fn set_zoom(app: &AppHandle, factor: f64) {
    let clamped = factor.clamp(0.5, 3.0);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_zoom(clamped);
    }
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut slot) = state.zoom.lock() {
            *slot = clamped;
        }
    }
}

// ---------------------------------------------------------------------------
// Uygulamalar arasi gecis
// ---------------------------------------------------------------------------

/// Mevcut pencereyi secilen uygulamanin canli URL'sine yonlendirir.
/// Yeni pencere ACMAZ — ayni kabukta gezinir.
fn switch_app(app: &AppHandle, key: &str) {
    let Some(entry) = APPS.iter().find(|e| e.0 == key) else {
        return;
    };
    let label = entry.1;
    let target = entry.2;

    let state = app.try_state::<AppState>();

    // Zaten o uygulamadaysak hicbir sey yapma (menu olayinin cift tetiklenmesi
    // ve gereksiz sayfa yenilemesi icin koruma).
    if let Some(s) = state.as_ref() {
        if let Ok(cur) = s.current.lock() {
            if cur.as_str() == key {
                return;
            }
        }
    }

    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(url) = Url::parse(target) else {
        return;
    };

    // KATMANI BURADA CIZMEYE CALISMA. `eval` + `navigate` ayni olay dongusune
    // kuyruklanir ve `evaluate_script` asenkrondur; navigasyon belgeyi katman
    // boyanmadan yikar (v1.5.1'in KOK NEDENI). Katman hedef sayfada
    // document-start'ta kurulur; asagida yalnizca "gecis" bilgisi iletilir.

    // Gecis sonrasi ILK sayfa yuklemesinde erisim engeli (403/401) denetlensin.
    PENDING_ACCESS_CHECK.store(true, Ordering::SeqCst);

    if window.navigate(url).is_err() {
        PENDING_ACCESS_CHECK.store(false, Ordering::SeqCst);
        hide_overlay(app);
        return;
    }

    // Hedef sayfadaki katmani "uygulama gecisi" kipine al ("Finans'a geçiliyor…").
    wake_overlay(app, "switch");

    let _ = window.set_title(&format!("Bogahost {}", label));
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();

    // EMNIYET AGI: hedef sayfa hic yuklenmezse (ag hatasi / sunucu yanit vermiyor)
    // "Yükleniyor…" katmani ekranda KALICI olarak kalir ve kullanici bunu
    // "gecis calismiyor / uygulama dondu" olarak gorur. En gec bu sure sonunda
    // katman kaldirilir ve durum kullaniciya yazili olarak bildirilir.
    {
        let h = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(SWITCH_LOADING_TIMEOUT);
            hide_overlay(&h);
        });
    }

    // Menu isaretlerini guncelle.
    if let Some(s) = state.as_ref() {
        if let Ok(mut cur) = s.current.lock() {
            *cur = key.to_string();
        }
        if let Ok(items) = s.items.lock() {
            for (item_key, item) in items.iter() {
                let active = item_key.as_str() == key;
                let _ = item.set_checked(active);
                let _ = item.set_enabled(!active);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pencere durumu (boyut / konum)
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct WindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn window_state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join(WINDOW_STATE_FILE))
}

fn restore_window_state(window: &tauri::WebviewWindow<Wry>) {
    let Some(path) = window_state_path(window.app_handle()) else {
        return;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(state) = serde_json::from_str::<WindowState>(&raw) else {
        return;
    };
    if state.width >= 400 && state.height >= 300 {
        let _ = window.set_size(PhysicalSize::new(state.width, state.height));
    }
    // Ekran disina dusmus konumlari yok say.
    if state.x > -20_000 && state.y > -20_000 && state.x < 20_000 && state.y < 20_000 {
        let _ = window.set_position(PhysicalPosition::new(state.x, state.y));
    }
}

/// Pencere durumunu diske yazar. `force = false` ise en fazla 2 saniyede bir yazar
/// (surukleme/boyutlandirma sirasinda diske bogulmamak icin).
fn save_window_state(window: &tauri::Window<Wry>, force: bool) {
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    persist_window_state(window.app_handle(), pos, size, force);
}

/// Ayni islemin tepsi/menu tarafindan (yalnizca `AppHandle` varken) cagrilan hali.
fn save_window_state_from_handle(app: &AppHandle, force: bool) {
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    let (Ok(pos), Ok(size)) = (w.outer_position(), w.inner_size()) else {
        return;
    };
    persist_window_state(app, pos, size, force);
}

fn persist_window_state(
    app: &AppHandle,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    force: bool,
) {
    if !force {
        if let Ok(mut last) = LAST_STATE_SAVE.lock() {
            let now = Instant::now();
            if let Some(prev) = *last {
                if now.duration_since(prev) < Duration::from_secs(2) {
                    return;
                }
            }
            *last = Some(now);
        }
    }

    let Some(path) = window_state_path(app) else {
        return;
    };
    let state = WindowState {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    };
    let Ok(json) = serde_json::to_string(&state) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, json);
}

// ---------------------------------------------------------------------------
// Bildirim izni
// ---------------------------------------------------------------------------

fn notification_granted(app: &AppHandle) -> bool {
    matches!(
        app.notification().permission_state(),
        Ok(PermissionState::Granted)
    )
}

/// Bildirim iznini acikca ister. macOS'ta sistem onay penceresi bu cagri ile
/// (ve ilk bildirimle) cikar. Arka plan is parcaciginda calisir — arayuzu bloklamaz.
/// Izin verildikten sonra YALNIZCA ILK SEFER bir test bildirimi gosterir.
fn ensure_notification_permission(app: &AppHandle) {
    let notification = app.notification();

    let mut granted = matches!(notification.permission_state(), Ok(PermissionState::Granted));

    // ILK ACILIS: sistem izin penceresini habersiz onune koymak yerine once
    // NEDEN gerektigini anlat. YALNIZCA BIR KEZ sorulur — kullanici "Hayır"
    // derse bir daha ustelenmez; tepsideki "Bildirimler: kapalı (ayarları aç)"
    // ogesi kalici ama rahatsiz etmeyen yol olarak durur.
    //
    // `blocking_show` ARKA PLAN is parcaciginda cagrilir (bkz. `setup`);
    // ana thread'de cagrilsaydi kilitlenirdi.
    if !granted {
        if mark_once(app, "notify-ask") {
            let wants = app
                .dialog()
                .message(concat!(
                    "Bildirimleri açmak ister misiniz?\n\n",
                    "Yeni görev, mesaj ve uyarılar siz panelde değilken de ",
                    "anında masaüstünde gösterilir."
                ))
                .title(APP_TITLE)
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::YesNo)
                .blocking_show();
            if !wants {
                return;
            }
        }
        granted = matches!(notification.request_permission(), Ok(PermissionState::Granted));
    }

    if granted && mark_once(app, "notify-intro") {
        // Gosterim ana thread'e kuyruklanir (bkz. `notify`).
        notify(
            app,
            APP_TITLE,
            "Bildirimler açıldı. Panel bildirimleri artık masaüstünde gösterilecek.",
        );
    }

    // Izin REDDEDILDIYSE sessiz kalma: sayfada aciklama + "Bildirim Ayarlarını
    // Aç" dugmesi goster.
    // Yalnizca ILK reddedilmede aciklama kutusu gosterilir; her acilista
    // tekrarlamak "usteleme" olurdu.
    if !granted && mark_once(app, "notify-denied-box") {
        show_notification_denied_box(app);
    }
}

/// Bildirim izni yoksa sayfada aciklayici kutu + ayar dugmesi gosterir.
///
/// Windows'ta durum GUVENILIR okunamadigi icin (bkz. `refresh_notification_menu`
/// — plugin orada daima `Granted` doner) bu kutu HIC cikarilmaz: yanlis alarm
/// vermektense hic vermemek yeglenir.
#[cfg(target_os = "windows")]
fn show_notification_denied_box(_app: &AppHandle) {}

#[cfg(not(target_os = "windows"))]
fn show_notification_denied_box(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(
            "try { window.__bogahostNotifyDeniedBox && window.__bogahostNotifyDeniedBox(); } catch (e) {}",
        );
    }
}

/// Tepsi menusundeki "Bildirimler: ..." ogesinin etiketini gunceller.
///
/// DURUSTLUK NOTU: `tauri-plugin-notification`, Windows masaustunde bir izin
/// kavrami OLMADIGI icin `permission_state()` cagrisindan HER ZAMAN `Granted`
/// dondurur. Bunu "Bildirimler: açık" diye gostermek YANILTICIDIR — kullanici
/// Windows Ayarlar'dan bildirimleri kapatmis olabilir ve uygulama bunu goremez.
/// Bu yuzden Windows'ta durum IDDIA EDILMEZ, ayara YONLENDIRILIR.
fn refresh_notification_menu(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    let label = {
        let _ = notification_granted(app);
        "Bildirimler: Windows ayarlarından yönetilir"
    };

    #[cfg(not(target_os = "windows"))]
    let label = if notification_granted(app) {
        "Bildirimler: açık"
    } else {
        "Bildirimler: kapalı (ayarları aç)"
    };
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(items) = state.notify_items.lock() {
            for item in items.iter() {
                let _ = item.set_text(label);
            }
        }
    }
}

/// Sistem bildirim ayarlarini acar.
fn open_notification_settings(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let url = "x-apple.systempreferences:com.apple.preference.notifications";
    #[cfg(target_os = "windows")]
    let url = "ms-settings:notifications";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let url = "https://bogahost.com/";

    let _ = app.shell().open(url.to_string(), None);
}

/// Verilen bayrak daha once isaretlenmediyse isaretler ve `true` doner.
/// (Tek seferlik islemler icin; uygulama veri klasorunde kucuk bir dosya tutar.)
fn mark_once(app: &AppHandle, flag: &str) -> bool {
    let Ok(dir) = app.path().app_config_dir() else {
        return false;
    };
    let marker = dir.join(format!("{}.flag", flag));
    if marker.exists() {
        return false;
    }
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }
    let _ = std::fs::write(&marker, b"1");
    true
}

// ---------------------------------------------------------------------------
// Otomatik baslatma (oturum acilisi) — "arka planda calisir kal"
// ---------------------------------------------------------------------------

/// Tepsideki "Bilgisayar açılınca başlat" ogesini isletim sistemindeki GERCEK
/// duruma gore isaretler (varsayimda bulunmaz).
fn refresh_autostart_menu(app: &AppHandle) {
    let enabled = app.autolaunch().is_enabled().unwrap_or(false);
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(items) = state.autostart_items.lock() {
            for item in items.iter() {
                let _ = item.set_checked(enabled);
            }
        }
    }
}

/// Otomatik baslatmayi acar/kapatir.
///
/// Hata durumunda menu GERCEK duruma geri alinir — kullanici yanlis bir
/// isaret gormesin.
fn toggle_autostart(app: &AppHandle) {
    let manager = app.autolaunch();
    let enabled = manager.is_enabled().unwrap_or(false);
    let result = if enabled {
        manager.disable()
    } else {
        manager.enable()
    };
    if let Err(e) = result {
        eprintln!("[{}][autostart] degistirilemedi: {}", APP_KEY, e);
    }
    refresh_autostart_menu(app);
}

/// Cikista (Cmd+Q / tepsi "Cikis") YALNIZCA BIR KEZ bilgilendirir:
/// uygulama kapaliyken bildirim gelmez, tepside birakilirsa gelmeye devam eder.
///
/// Cikis ENGELLENMEZ; yalnizca ilk seferde diyalog kapanana kadar beklenir.
/// Diyalog acilamazsa ya da kullanici yanit vermezse emniyet suresi sonunda
/// uygulama yine de kapanir — "kapanmayan uygulama" DURUMU OLUSMAZ.
fn show_quit_notice(app: &AppHandle) {
    let handle = app.clone();
    app.dialog()
        .message(
            concat!(
                "Uygulama tamamen kapanıyor. Kapalıyken yeni bildirim ALINMAZ.\n\n",
                "Bildirim almaya devam etmek için pencereyi kapatıp uygulamayı ",
                "tepside açık bırakabilirsiniz."
            ),
        )
        .title(APP_TITLE)
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::Ok)
        .show(move |_| {
            handle.exit(0);
        });

    // EMNIYET: diyalog hic yanitlanmazsa da kapan.
    let h = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        h.exit(0);
    });
}

// ---------------------------------------------------------------------------
// Guncelleme — birincil yol: tam otomatik (indir + kur + yeniden baslat)
// ---------------------------------------------------------------------------

/// Guncelleme hatalari kullaniciyi RAHATSIZ ETMEZ; yalnizca stderr'e yazilir.
fn log_update(message: &str) {
    eprintln!("[{}][updater] {}", APP_KEY, message);
}

/// `try_auto_update` sonucu.
enum UpdateOutcome {
    /// Updater akisi calisti (guncelleme yok / diyalog acildi).
    Handled,
    /// Updater kullanilamadi — eski manifest denetimine dusulmeli.
    Unavailable(String),
}

/// Sayfa su an "mesgul" mu? (gorusme / ekran paylasimi / doldurulmus form)
fn page_busy() -> bool {
    PAGE_BUSY.load(Ordering::SeqCst)
}

/// Kurulmayi/uygulanmayi bekleyen surum (varsa).
fn pending_version() -> Option<String> {
    PENDING_UPDATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.version.clone()))
}

/// Guncelleme akisinin girisi.
/// Once `tauri-plugin-updater` denenir; kullanilamazsa eski manifest denetimi yapilir.
/// `verbose = true` (tepsiden elle denetim) ise sonuc ne olursa olsun bildirim gosterilir.
async fn run_update_flow(app: AppHandle, verbose: bool) {
    // Guncelleme ZATEN indirildi/kuruldu ve kullanici onayini bekliyor:
    // sunucuyu bir daha yormanin anlami yok, yalnizca seridi tazele.
    if let Some(version) = pending_version() {
        show_update_banner(&app, &version, verbose);
        if verbose {
            notify(
                &app,
                "Güncelleme hazır",
                &format!("Sürüm {version} kuruldu. Uygulamadaki şeritten \"Şimdi uygula\" deyin."),
            );
        }
        return;
    }

    match try_auto_update(&app, verbose).await {
        UpdateOutcome::Handled => {}
        UpdateOutcome::Unavailable(reason) => {
            log_update(&format!(
                "otomatik guncelleme kullanilamadi ({reason}) — manifest denetimine dusuluyor"
            ));
            check_update_legacy(app, verbose).await;
        }
    }
}

/// `tauri-plugin-updater` ile sunucudaki imzali manifesti denetler.
/// Kisa zaman asimi: ag yavassa uygulama BEKLEMEZ.
async fn try_auto_update(app: &AppHandle, verbose: bool) -> UpdateOutcome {
    if !UPDATER_READY.load(Ordering::SeqCst) {
        return UpdateOutcome::Unavailable("eklenti yuklu degil".to_string());
    }

    let updater = match app.updater_builder().timeout(NETWORK_TIMEOUT).build() {
        Ok(u) => u,
        Err(e) => return UpdateOutcome::Unavailable(e.to_string()),
    };

    match updater.check().await {
        Ok(Some(update)) => {
            // KULLANICIYI BOLME: diyalog YOK. Indirme/kurulum sessizce arka
            // planda yapilir; hazir olunca sayfa icinde serit cikar.
            // `auto_apply = false`: bu CALISIRKEN periyodik yoldur, kendiliginden
            // YENIDEN BASLATMAZ (asil otomatik yol acilistadir).
            stage_update(app.clone(), update, false).await;
            UpdateOutcome::Handled
        }
        Ok(None) => {
            if verbose {
                notify(
                    app,
                    "Sürüm denetimi",
                    &format!(
                        "En güncel sürümü kullanıyorsunuz ({}).",
                        env!("CARGO_PKG_VERSION")
                    ),
                );
            }
            UpdateOutcome::Handled
        }
        Err(e) => UpdateOutcome::Unavailable(e.to_string()),
    }
}

/// ACILIS OTOMATIK GUNCELLEMESI (ASIL yol) — kullanici etkilesime girmeden,
/// sessizce denetler; guncelleme varsa ONAY SORMADAN indirir, kurar ve yeniden
/// baslatir. `stage_update(..., auto_apply = true)` bu isi yapar.
///
/// BLOKLAMAZ: `check` kisa timeout'ludur. Guncelleme yoksa / ag yoksa hicbir sey
/// yapmaz ve uygulama normal calismaya devam eder.
async fn startup_auto_update(app: AppHandle) {
    if !UPDATER_READY.load(Ordering::SeqCst) {
        return;
    }
    let updater = match app.updater_builder().timeout(NETWORK_TIMEOUT).build() {
        Ok(u) => u,
        Err(e) => {
            log_update(&format!("acilis guncelleme: updater kurulamadi ({e})"));
            return;
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            log_update(&format!(
                "acilis: {} bulundu — otomatik indirilip kurulacak",
                update.version
            ));
            // Splash zaten aciliyorsa durumu bildir (zararsiz; katman yoksa no-op).
            set_overlay_status(&app, "Güncelleniyor…");
            stage_update(app, update, true).await;
        }
        // Guncelleme yok / ag hatasi: sessiz, normal acilis.
        _ => {}
    }
}

/// Guncellemeyi indirir ve kurar.
///
/// `auto_apply = true` (ACILIS yolu): kurulum basariliysa KENDILIGINDEN yeniden
/// baslatir (kullaniciya sormaz) — bu, "kapatip acinca guncellensin" davranisidir
/// ve imzasiz macOS'ta yalnizca ACILIS penceresinde calisir.
///
/// `auto_apply = false` (CALISIRKEN periyodik yol): yeniden BASLATMAZ; hazir
/// olunca serit cikar, kullanici "Şimdi uygula" der.
///
/// Platform farki kasitlidir (`tauri-plugin-updater`in gercek davranisi):
///  * **Windows:** `Update::install` installer'i calistirip SURECI OLDURUR; bu
///    yuzden calisirken YALNIZCA `download()` yapilir, kurulum kullanici onayinda
///    (`apply_update`) calisir. Acilis otomatik yolu Windows'ta UAC/installer'i
///    kullaniciya sormadan tetiklemez — orada da serit yolu kullanilir.
///  * **macOS/Linux:** `install` uygulama paketini surec calisirken degistirir.
///    Basarisizsa (imzasiz macOS: bundle degistirilemez) bu KALICI bir hatadir —
///    serit "İndirme sayfasını aç" (manuel indirme) moduna gecer.
async fn stage_update(app: AppHandle, update: tauri_plugin_updater::Update, auto_apply: bool) {
    // Es zamanli iki denetim ayni surumu iki kez indirmesin.
    if UPDATE_INSTALLING.swap(true, Ordering::SeqCst) {
        return;
    }
    let version = update.version.clone();
    log_update(&format!("{version} arka planda indiriliyor"));

    // 1) INDIR (her platformda ayni). Basarisizsa AG hatasidir -> sessiz,
    //    bir sonraki periyodik denetimde tekrar denenir.
    let bytes = match update.download(|_chunk, _total| {}, || {}).await {
        Ok(b) => b,
        Err(e) => {
            UPDATE_INSTALLING.store(false, Ordering::SeqCst);
            log_update(&format!("indirme basarisiz (ag): {e}"));
            return;
        }
    };

    // 2) KUR / SUNU.
    #[cfg(target_os = "windows")]
    {
        // Windows: install SURECI OLDURUR; kurulum kullanici onayinda yapilir.
        // (Acilis otomatik yolunda dahi Windows'ta installer'i kullaniciya
        // sormadan calistirmayiz — paketi bellekte tutup serit gosteririz.)
        let _ = auto_apply;
        UPDATE_INSTALLING.store(false, Ordering::SeqCst);
        UPDATE_MANUAL.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = PENDING_UPDATE.lock() {
            *slot = Some(PendingUpdate {
                version: version.clone(),
                installer: Some((update, bytes)),
            });
        }
        log_update(&format!("{version} indirildi — kullanici onayi bekleniyor"));
        show_update_banner(&app, &version, true);
        notify(
            &app,
            "Güncelleme hazır",
            &format!("Sürüm {version} indirildi. Uygun olduğunuzda \"Şimdi uygula\" deyin."),
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        if auto_apply {
            RESTART_IN_PROGRESS.store(true, Ordering::SeqCst);
        }
        match update.install(&bytes) {
            Ok(()) => {
                log_update(&format!("{version} kuruldu"));
                UPDATE_MANUAL.store(false, Ordering::SeqCst);
                if auto_apply {
                    // ACILIS yolu: kuruldu -> HEMEN yeniden baslat (DONMEZ).
                    log_update(&format!("acilis: {version} kuruldu — yeniden baslatiliyor"));
                    UPDATE_INSTALLING.store(false, Ordering::SeqCst);
                    app.restart();
                }
                // CALISIRKEN periyodik yol: paket kuruldu, yeniden baslatma
                // kullanici onayinda (apply_update yalnizca restart eder).
                UPDATE_INSTALLING.store(false, Ordering::SeqCst);
                if let Ok(mut slot) = PENDING_UPDATE.lock() {
                    *slot = Some(PendingUpdate {
                        version: version.clone(),
                        installer: None,
                    });
                }
                log_update(&format!("{version} hazir — kullanici onayi bekleniyor"));
                show_update_banner(&app, &version, true);
                notify(
                    &app,
                    "Güncelleme hazır",
                    &format!(
                        "Sürüm {version} kuruldu. Uygun olduğunuzda \"Şimdi uygula\" deyin."
                    ),
                );
            }
            Err(e) => {
                // KALICI hata (imzasiz macOS: calisan uygulama kendi bundle'ini
                // degistiremez). Tekrar denemek ise yaramaz -> MANUEL indirme.
                if auto_apply {
                    RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
                }
                UPDATE_INSTALLING.store(false, Ordering::SeqCst);
                UPDATE_MANUAL.store(true, Ordering::SeqCst);
                log_update(&format!(
                    "kurulum basarisiz (kalici): {e} — manuel indirme yoluna gecildi"
                ));
                // pending_version() Some kalsin ki serit yeniden cizilebilsin.
                if let Ok(mut slot) = PENDING_UPDATE.lock() {
                    *slot = Some(PendingUpdate {
                        version: version.clone(),
                        installer: None,
                    });
                }
                show_update_banner(&app, &version, true);
                notify(
                    &app,
                    "Güncelleme mevcut",
                    &format!(
                        "Sürüm {version} otomatik kurulamadı. Şeritten \"İndirme sayfasını aç\" ile indirin."
                    ),
                );
            }
        }
    }
}

/// Sayfa icindeki guncelleme seridini cizdirir (bkz. `UPDATE_UI_JS`).
/// Serit KENDILIGINDEN hicbir sey yapmaz; yalnizca dugme sunar.
/// `force = true` ise kullanicinin "Sonra" karari YOK SAYILIR (elle denetim).
///
/// `UPDATE_MANUAL` set ise (imzasiz macOS: otomatik kurulum kalici basarisiz)
/// serit "Şimdi uygula" yerine "İndirme sayfasını aç" (manuel indirme) gosterir.
fn show_update_banner(app: &AppHandle, version: &str, force: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let js = if UPDATE_MANUAL.load(Ordering::SeqCst) {
            format!(
                "try {{ window.__bogahostUpdateManual && window.__bogahostUpdateManual({version:?}); }} catch (e) {{}}"
            )
        } else {
            format!(
                "try {{ window.__bogahostUpdateReady && window.__bogahostUpdateReady({version:?}, {force}); }} catch (e) {{}}"
            )
        };
        let _ = w.eval(js);
    }
}

/// Acilis/guncelleme sirasinda yukleme katmaninin durum metnini gunceller
/// (katman gorunur degilse zararsizca yok sayilir).
fn set_overlay_status(app: &AppHandle, text: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(format!(
            "try {{ window.__bogahostLoadingStatus && window.__bogahostLoadingStatus({text:?}); }} catch (e) {{}}"
        ));
    }
}

/// Otomatik guncelleme uygulanamadiginda (imzasiz macOS ya da Windows installer
/// hatasi) YENI surumun kurulum dosyasini sistem tarayicisinda acar.
/// Surum: bekleyen (yeni) surum; yoksa kurulu surum.
#[tauri::command]
fn bogahost_open_download(app: AppHandle) -> Result<(), String> {
    let version = pending_version().unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let file = if cfg!(target_os = "windows") {
        format!("{APP_KEY}-{version}-windows-x86_64-setup.exe")
    } else {
        format!("{APP_KEY}-{version}-macos.dmg")
    };
    let url = format!("{DOWNLOAD_BASE_URL}/{file}");
    app.shell().open(url, None).map_err(|e| e.to_string())
}

/// Sayfanin bildirdigi "mesgulum" bayragi.
///
/// Sayfa gorusme/ekran paylasimi/doldurulmus form durumunda `true` gonderir.
/// Bayrak YALNIZCA otomatik indirmeyi erteler; yeniden baslatma zaten her zaman
/// kullanici tikladiginda olur.
#[tauri::command]
fn bogahost_set_busy(busy: bool) {
    PAGE_BUSY.store(busy, Ordering::SeqCst);
}

/// Sayfa, seridi yeniden cizmek icin bekleyen guncellemeyi sorabilir.
/// Bekleyen yoksa bos dize doner.
#[tauri::command]
fn bogahost_update_state() -> String {
    pending_version().unwrap_or_default()
}

/// Kullanici serittteki "Şimdi uygula" dugmesine basti.
///
/// Bu, yeniden baslatmayi tetikleyen TEK yoldur — periyodik denetim, indirme
/// veya kurulum ASLA kendiliginden yeniden baslatmaz.
#[tauri::command]
fn bogahost_apply_update(app: AppHandle) -> Result<(), String> {
    let pending = PENDING_UPDATE.lock().ok().and_then(|mut g| g.take());
    let Some(pending) = pending else {
        return Err("hazır güncelleme yok".to_string());
    };

    // SIRA ONEMLI: surec birazdan olecek, once diske yazilir.
    save_resume_url(&app);
    save_window_state_from_handle(&app, true);
    RESTART_IN_PROGRESS.store(true, Ordering::SeqCst);
    log_update(&format!("{} uygulaniyor (kullanici onayi)", pending.version));

    // Windows: kurulum burada baslar ve installer sureci sonlandirir.
    let PendingUpdate { version, installer } = pending;
    if let Some((update, bytes)) = installer {
        if let Err(e) = update.install(&bytes) {
            // Basarisizlikta paketi GERI KOY: tekrar indirmeye gerek yok ve
            // serit "hazır güncelleme yok" diye bosa dusmez.
            RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
            if let Ok(mut slot) = PENDING_UPDATE.lock() {
                *slot = Some(PendingUpdate {
                    version,
                    installer: Some((update, bytes)),
                });
            }
            log_update(&format!("kurulum basarisiz: {e}"));
            return Err("Güncelleme kurulamadı.".to_string());
        }
        return Ok(());
    }

    // macOS/Linux: paket zaten kuruldu, yalnizca yeniden baslat.
    // `restart()` geri DONMEZ.
    app.restart()
}

// ---------------------------------------------------------------------------
// Yeniden baslatmada BAGLAMI KORUMA — "kaldigi yerden devam"
// ---------------------------------------------------------------------------
//
// Surec olecegi icin bellek ise yaramaz: acik olan sayfa adresi DISKE yazilir
// (uygulama yapilandirma klasoru, `RESUME_FILE`). Yeni surec `build_main_window`
// icinde dosyayi okur, SILER ve oradan acilir.
//
// Dosya YALNIZCA guncelleme uygulanirken yazilir; normal cikista yazilmaz —
// boylece gunlerdir kapali duran uygulama eski bir sayfayla acilmaz.

/// Ana pencerede su an acik olan adresi bellekte tutar.
fn remember_page_url(url: &Url) {
    // Yalnizca KENDI alan adimizdaki gercek panel sayfalari saklanir.
    // Belge adresleri (PDF/CSV) `guard_document_navigation` tarafindan zaten
    // geri alinir; onlari kurtarma hedefi yapmak anlamsiz olurdu.
    if !is_internal_url(url) || looks_like_document_url(url) {
        return;
    }
    if let Ok(mut slot) = CURRENT_PAGE_URL.lock() {
        *slot = Some(url.to_string());
    }
}

fn resume_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(RESUME_FILE))
}

/// Su anki sayfa adresini diske yazar (yeniden baslatmadan HEMEN once).
fn save_resume_url(app: &AppHandle) {
    let Some(url) = CURRENT_PAGE_URL.lock().ok().and_then(|g| g.clone()) else {
        return;
    };
    let Some(path) = resume_path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let payload = serde_json::json!({ "url": url, "ts": unix_now() });
    if let Err(e) = std::fs::write(&path, payload.to_string()) {
        log_update(&format!("sayfa adresi saklanamadi: {e}"));
    }
}

/// Saklanmis adresi okur ve dosyayi SILER (tek seferlik).
/// Adres bayatsa ya da dis bir alan adiysa `None` doner.
fn take_resume_url(app: &AppHandle) -> Option<String> {
    let path = resume_path(app)?;
    let raw = std::fs::read_to_string(&path).ok();
    // Okuma basarisiz olsa bile dosya TEMIZLENIR: bozuk bir dosya her acilista
    // ayni hataya yol acmasin.
    let _ = std::fs::remove_file(&path);

    let value: serde_json::Value = serde_json::from_str(&raw?).ok()?;
    let ts = value.get("ts")?.as_u64()?;
    if unix_now().saturating_sub(ts) > RESUME_MAX_AGE_SECS {
        log_update("saklanan sayfa adresi bayat — panel anasayfasi acilacak");
        return None;
    }
    let url = value.get("url")?.as_str()?.to_string();
    // GUVENLIK: yalnizca kendi alan adimiz geri yuklenir.
    if !is_internal_url(&Url::parse(&url).ok()?) {
        return None;
    }
    Some(url)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
// ---------------------------------------------------------------------------
// Guncelleme — yedek yol: eski manifest denetimi ("yeni surum var mi")
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct VersionManifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

/// YEDEK YOL. `VERSION_MANIFEST_URL` adresindeki JSON'u okur ve yerel surumle
/// karsilastirir; yalnizca BILDIRIM gosterir (indirme/kurulum yapmaz).
/// Yalnizca `tauri-plugin-updater` kullanilamadiginda cagrilir.
/// `verbose = true` ise sonuc ne olursa olsun bildirim gosterir (menuden manuel denetim).
async fn check_update_legacy(app: AppHandle, verbose: bool) {
    let current = env!("CARGO_PKG_VERSION");

    let client = match reqwest::Client::builder()
        .timeout(NETWORK_TIMEOUT)
        .connect_timeout(NETWORK_TIMEOUT)
        .user_agent(format!("BogahostNative/{} ({})", current, APP_KEY))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let response = match client.get(VERSION_MANIFEST_URL).send().await {
        Ok(r) => r,
        Err(_) => {
            if verbose {
                notify(&app, "Sürüm denetimi", "Sürüm bilgisine ulaşılamadı.");
            }
            return;
        }
    };

    if !response.status().is_success() {
        if verbose {
            notify(&app, "Sürüm denetimi", "Sürüm bilgisine ulaşılamadı.");
        }
        return;
    }

    let body = match response.text().await {
        Ok(t) => t,
        Err(_) => return,
    };

    let manifest: VersionManifest = match serde_json::from_str(&body) {
        Ok(m) => m,
        Err(_) => {
            if verbose {
                notify(&app, "Sürüm denetimi", "Sürüm bilgisi okunamadı.");
            }
            return;
        }
    };

    if is_newer(&manifest.version, current) {
        if let Some(url) = manifest.url.as_ref() {
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut slot) = state.download_url.lock() {
                    *slot = url.clone();
                }
            }
        }

        let notes = manifest.notes.clone().unwrap_or_default();
        let body = if notes.is_empty() {
            format!(
                "Yeni sürüm {} yayınlandı (yüklü: {}). Tepsi menüsünden \"İndirme sayfasını aç\".",
                manifest.version, current
            )
        } else {
            format!(
                "Yeni sürüm {} yayınlandı (yüklü: {}). {}",
                manifest.version, current, notes
            )
        };
        notify(&app, "Güncelleme mevcut", &body);
    } else if verbose {
        notify(
            &app,
            "Sürüm denetimi",
            &format!("En güncel sürümü kullanıyorsunuz ({}).", current),
        );
    }
}

/// Native bildirim gosterir.
/// macOS bildirim API'si ANA THREAD'den cagrilmalidir; bu yuzden gosterim
/// `run_on_main_thread` ile kuyruga alinir (cagiran thread BLOKLANMAZ).
fn notify(app: &AppHandle, title: &str, body: &str) {
    let handle = app.clone();
    let title = title.to_string();
    let body = body.to_string();
    let _ = app.run_on_main_thread(move || {
        let _ = handle
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
    });
}

/// "1.2.0" tarzi surumleri sayisal parcalara ayirir ("v" oneki ve ekler tolere edilir).
fn parse_version(value: &str) -> Vec<u64> {
    value
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(4)
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn is_newer(remote: &str, local: &str) -> bool {
    let remote_parts = parse_version(remote);
    let local_parts = parse_version(local);
    if remote_parts.is_empty() {
        return false;
    }
    let len = remote_parts.len().max(local_parts.len());
    for i in 0..len {
        let r = remote_parts.get(i).copied().unwrap_or(0);
        let l = local_parts.get(i).copied().unwrap_or(0);
        if r != l {
            return r > l;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{decode_base64, is_newer, sanitize_file_name};

    #[test]
    fn surum_karsilastirma() {
        assert!(is_newer("1.3.0", "1.2.0"));
        assert!(is_newer("v1.2.1", "1.2.0"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(!is_newer("1.1.9", "1.2.0"));
        assert!(!is_newer("bozuk", "1.2.0"));
    }

    #[test]
    fn dosya_adi_temizleme() {
        assert_eq!(sanitize_file_name("rapor.pdf"), "rapor.pdf");
        assert_eq!(sanitize_file_name("../../etc/passwd"), "-..-etc-passwd");
        assert_eq!(sanitize_file_name("   "), "indirilen-dosya");
    }

    #[test]
    fn base64_cozme() {
        assert_eq!(decode_base64("Qm9nYQ==").unwrap(), b"Boga".to_vec());
        assert_eq!(decode_base64("").unwrap(), Vec::<u8>::new());
        assert!(decode_base64("!!!").is_none());
    }
}
