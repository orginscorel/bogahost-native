// Bogahost Chat — Tauri v2 masaustu kabugu.
//
// Bu dosya 4 uygulamada (finans/dcim/chat/task) AYNIDIR; yalnizca asagidaki
// APP_KEY / APP_TITLE sabitleri farklidir. Degistirirken hepsini birlikte guncelleyin.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Url, WindowEvent, Wry,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::{NotificationExt, PermissionState};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_updater::UpdaterExt;

const APP_KEY: &str = "chat";
const APP_TITLE: &str = "Bogahost Chat";

/// Uygulamalar arasi gecis tablosu — `apps.config.json` ile BIREBIR ayni olmali.
/// (key, menu etiketi, canli URL)
const APPS: [(&str, &str, &str); 4] = [
    ("finans", "Finans", "https://finans.bogahost.com/admin"),
    ("dcim", "DCIM", "https://dcim.bogahost.com/admin"),
    ("chat", "Chat", "https://chat.bogahost.com/admin"),
    ("task", "Görevler", "https://task.bogahost.com/admin"),
];

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

/// Es zamanli/cift surum denetimini engeller.
static UPDATE_CHECK_RUNNING: AtomicBool = AtomicBool::new(false);

/// `tauri-plugin-updater` calisma aninda basariyla yuklendi mi?
/// Yuklenmediyse `app.updater()` cagrilmaz (yonetilmeyen state -> panic olurdu).
static UPDATER_READY: AtomicBool = AtomicBool::new(false);

/// Ayni anda birden fazla "Simdi kurulsun mu?" diyalogu acilmasini engeller.
static UPDATE_PROMPT_OPEN: AtomicBool = AtomicBool::new(false);

/// Tepsi + macOS menu cubugundaki "Uygulamalar" ogeleri ve aktif uygulama.
struct SwitchState {
    items: Mutex<Vec<(String, CheckMenuItem<Wry>)>>,
    current: Mutex<String>,
    download_url: Mutex<String>,
}

pub fn run() {
    tauri::Builder::default()
        // Harici linkleri sistem tarayicisinda acmak + genel shell erisimi.
        .plugin(tauri_plugin_shell::init())
        // Native masaustu bildirimleri.
        .plugin(tauri_plugin_notification::init())
        // Guncelleme onay diyalogu.
        .plugin(tauri_plugin_dialog::init())
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

            // Menulerde kullanilan "Uygulamalar" ogelerinin tamami (tepsi + menu cubugu).
            let mut switch_items: Vec<(String, CheckMenuItem<Wry>)> = Vec::new();

            // ----- Sistem tepsisi (tray) menusu -----
            let show_i = MenuItem::with_id(app, "show", "Goster / Show", true, None::<&str>)?;
            let hide_i = MenuItem::with_id(app, "hide", "Gizle / Hide", true, None::<&str>)?;
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
            let quit_i = MenuItem::with_id(app, "quit", "Cikis / Quit", true, None::<&str>)?;

            let sep_a = PredefinedMenuItem::separator(app)?;
            let sep_b = PredefinedMenuItem::separator(app)?;
            let sep_c = PredefinedMenuItem::separator(app)?;

            let (tray_apps_sub, tray_apps_items) = build_apps_submenu(&handle)?;
            switch_items.extend(tray_apps_items);

            let tray_items: Vec<&dyn IsMenuItem<Wry>> = vec![
                &show_i,
                &hide_i,
                &sep_a,
                &tray_apps_sub,
                &sep_b,
                &upd_i,
                &dl_i,
                &sep_c,
                &quit_i,
            ];
            let menu = Menu::with_items(app, &tray_items)?;

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
            // Varsayilan menuyu (Uygulama/Duzen/Pencere) KORUYUP sonuna
            // "Uygulamalar" alt menusunu ekliyoruz. Windows'ta pencere icinde
            // menu cubugu istemiyoruz; orada gecis tepsi menusunden yapilir.
            #[cfg(target_os = "macos")]
            {
                let app_menu = Menu::default(&handle)?;
                let (mac_apps_sub, mac_apps_items) = build_apps_submenu(&handle)?;
                switch_items.extend(mac_apps_items);
                app_menu.append(&mac_apps_sub)?;
                let _ = app.set_menu(app_menu)?;
            }

            app.manage(SwitchState {
                items: Mutex::new(switch_items),
                current: Mutex::new(APP_KEY.to_string()),
                download_url: Mutex::new(DOWNLOAD_PAGE_URL.to_string()),
            });

            // Menu cubugu (ve varsa pencere menusu) olaylari.
            app.on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()));

            // ----- Bildirim izni -----
            // macOS/Windows sistem onay penceresi ancak ACIKCA istenince cikar.
            {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    ensure_notification_permission(&h);
                });
            }

            // ----- Acilista sessiz guncelleme denetimi -----
            // Guncelleme varsa onay diyalogu cikar; kullanici "Daha sonra" derse
            // kalici bir "atla" kaydi TUTULMAZ — bir sonraki acilista tekrar sorulur.
            {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    run_update_flow(h, false).await;
                });
            }

            Ok(())
        })
        // Pencere kapatilinca uygulamayi kapatma, tepsiye gizle (masaustu app hissi).
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("Bogahost Tauri uygulamasi calistirilirken hata");
}

// ---------------------------------------------------------------------------
// Uygulamalar arasi gecis
// ---------------------------------------------------------------------------

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

/// Tepsi ve menu cubugu olaylarinin ortak isleyicisi.
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
        "quit" => app.exit(0),
        "check-update" => {
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
            if let Some(state) = app.try_state::<SwitchState>() {
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

/// Mevcut pencereyi secilen uygulamanin canli URL'sine yonlendirir.
/// Yeni pencere ACMAZ — ayni kabukta gezinir.
fn switch_app(app: &AppHandle, key: &str) {
    let Some(entry) = APPS.iter().find(|e| e.0 == key) else {
        return;
    };
    let label = entry.1;
    let target = entry.2;

    let state = app.try_state::<SwitchState>();

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
    if window.navigate(url).is_err() {
        return;
    }

    let _ = window.set_title(&format!("Bogahost {}", label));
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();

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
// Bildirim izni
// ---------------------------------------------------------------------------

/// Bildirim iznini acikca ister. macOS'ta sistem onay penceresi bu cagri ile cikar.
/// Izin verildikten sonra YALNIZCA ILK SEFER bir test bildirimi gosterir.
fn ensure_notification_permission(app: &AppHandle) {
    let notification = app.notification();

    let mut granted = matches!(notification.permission_state(), Ok(PermissionState::Granted));
    if !granted {
        granted = matches!(notification.request_permission(), Ok(PermissionState::Granted));
    }

    if granted && mark_once(app, "notify-intro") {
        let _ = notification
            .builder()
            .title(APP_TITLE)
            .body("Bildirimler açıldı. Panel bildirimleri artık masaüstünde gösterilecek.")
            .show();
    }
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

/// Guncelleme akisinin girisi.
/// Once `tauri-plugin-updater` denenir; kullanilamazsa eski manifest denetimi yapilir.
/// `verbose = true` (tepsiden elle denetim) ise sonuc ne olursa olsun bildirim gosterilir.
async fn run_update_flow(app: AppHandle, verbose: bool) {
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
async fn try_auto_update(app: &AppHandle, verbose: bool) -> UpdateOutcome {
    if !UPDATER_READY.load(Ordering::SeqCst) {
        return UpdateOutcome::Unavailable("eklenti yuklu degil".to_string());
    }

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => return UpdateOutcome::Unavailable(e.to_string()),
    };

    match updater.check().await {
        Ok(Some(update)) => {
            prompt_and_install(app.clone(), update);
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

/// Onay diyalogunu gosterir; kullanici kabul ederse indirme+kurulumu baslatir.
/// Diyalog BLOKLAMAZ (callback'li `show`) — arayuz donmaz.
fn prompt_and_install(app: AppHandle, update: tauri_plugin_updater::Update) {
    // Acilis denetimi ile tepsiden elle denetim ust uste binerse tek diyalog.
    if UPDATE_PROMPT_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut message = format!(
        "Yeni sürüm {} hazır (yüklü: {}). Şimdi kurulsun mu?",
        update.version,
        env!("CARGO_PKG_VERSION")
    );
    if let Some(notes) = update.body.as_ref() {
        if !notes.trim().is_empty() {
            message.push_str("\n\n");
            message.push_str(notes.trim());
        }
    }

    let handle = app.clone();
    app.dialog()
        .message(message)
        .title("Güncelleme mevcut")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Şimdi kur".to_string(),
            "Daha sonra".to_string(),
        ))
        .show(move |accepted| {
            UPDATE_PROMPT_OPEN.store(false, Ordering::SeqCst);
            if !accepted {
                // Reddedildi: kalici kayit TUTULMAZ, bir sonraki acilista tekrar sorulur.
                log_update("kullanici guncellemeyi erteledi");
                return;
            }
            tauri::async_runtime::spawn(async move {
                install_update(handle, update).await;
            });
        });
}

/// Indirir, kurar ve uygulamayi yeniden baslatir. Hata olursa yalnizca bildirir.
async fn install_update(app: AppHandle, update: tauri_plugin_updater::Update) {
    let version = update.version.clone();
    notify(
        &app,
        "Güncelleme indiriliyor",
        &format!("Sürüm {version} indiriliyor. Bittiğinde uygulama yeniden başlatılacak."),
    );

    match update.download_and_install(|_chunk, _total| {}, || {}).await {
        Ok(()) => {
            notify(
                &app,
                "Güncelleme kuruldu",
                &format!("Sürüm {version} kuruldu. Uygulama yeniden başlatılıyor."),
            );
            // Windows'ta installer sureci uygulamayi zaten sonlandirir;
            // macOS'ta yeniden baslatma burada yapilir.
            app.restart();
        }
        Err(e) => {
            log_update(&format!("kurulum basarisiz: {e}"));
            notify(
                &app,
                "Güncelleme başarısız",
                "Güncelleme kurulamadı. Tepsi menüsünden \"İndirme sayfasını aç\" ile elle kurabilirsiniz.",
            );
        }
    }
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
        .timeout(std::time::Duration::from_secs(12))
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
            if let Some(state) = app.try_state::<SwitchState>() {
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

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
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
    use super::is_newer;

    #[test]
    fn surum_karsilastirma() {
        assert!(is_newer("1.2.0", "1.1.0"));
        assert!(is_newer("v1.1.1", "1.1.0"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("1.0.9", "1.1.0"));
        assert!(!is_newer("bozuk", "1.1.0"));
    }
}
