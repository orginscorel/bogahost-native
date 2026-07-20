// Bogahost Chat — Tauri v2 masaustu kabugu.
//
// Bu dosya 4 uygulamada (finans/dcim/chat/task) AYNIDIR; yalnizca asagidaki
// APP_KEY / APP_TITLE sabitleri farklidir. Degistirirken hepsini birlikte guncelleyin.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::{DownloadEvent, PageLoadEvent, WebviewWindowBuilder},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Url, WebviewUrl, WindowEvent, Wry,
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

/// Ag islemleri icin ust sinir — acilista ASLA uzun sure beklenmez.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(7);

/// Acilis surum denetimi bu kadar gecikmeyle baslar (sayfa yuklenmesiyle yarismasin).
const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(5);

/// Sayfa yuklenmese bile pencere en gec bu sure sonunda gosterilir.
const WINDOW_REVEAL_FALLBACK: Duration = Duration::from_secs(8);

/// Pencere konumu/boyutu bu dosyada saklanir (uygulama yapilandirma klasoru).
const WINDOW_STATE_FILE: &str = "window-state.json";

/// Es zamanli/cift surum denetimini engeller.
static UPDATE_CHECK_RUNNING: AtomicBool = AtomicBool::new(false);

/// `tauri-plugin-updater` calisma aninda basariyla yuklendi mi?
/// Yuklenmediyse `app.updater()` cagrilmaz (yonetilmeyen state -> panic olurdu).
static UPDATER_READY: AtomicBool = AtomicBool::new(false);

/// Ayni anda birden fazla "Simdi kurulsun mu?" diyalogu acilmasini engeller.
static UPDATE_PROMPT_OPEN: AtomicBool = AtomicBool::new(false);

/// Pencere bir kez gosterildi mi? (beyaz ekran yerine "yuklenince goster")
static WINDOW_REVEALED: AtomicBool = AtomicBool::new(false);

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
}

pub fn run() {
    tauri::Builder::default()
        // Harici linkleri sistem tarayicisinda acmak + genel shell erisimi.
        .plugin(tauri_plugin_shell::init())
        // Native masaustu bildirimleri.
        .plugin(tauri_plugin_notification::init())
        // Guncelleme onay diyalogu.
        .plugin(tauri_plugin_dialog::init())
        // Sayfadan (blob/data URL) gelen indirmeleri diske yazan kopru.
        .invoke_handler(tauri::generate_handler![
            bogahost_save_file,
            bogahost_open_external
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

            // ----- Ana pencere -----
            // Pencere tauri.conf.json'da DEGIL burada olusturuluyor; cunku
            // `on_download` / `on_navigation` / `on_page_load` yalnizca
            // WebviewWindowBuilder uzerinden baglanabilir (indirme destegi bunlara bagli).
            let window = build_main_window(&handle)?;

            // Menulerde kullanilan isaretlenebilir "Uygulamalar" ogelerinin tamami.
            let mut switch_items: Vec<(String, CheckMenuItem<Wry>)> = Vec::new();
            let mut notify_items: Vec<MenuItem<Wry>> = Vec::new();

            // ----- Sistem tepsisi (tray) menusu -----
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
            let sep_d = PredefinedMenuItem::separator(app)?;

            let (tray_apps_sub, tray_apps_items) = build_apps_submenu(&handle)?;
            switch_items.extend(tray_apps_items);
            // Windows'ta menu cubugu yok; Yenile/yakinlastirma tepsiden erisilebilsin.
            let tray_view_sub = build_view_submenu(&handle)?;

            let tray_items: Vec<&dyn IsMenuItem<Wry>> = vec![
                &show_i,
                &hide_i,
                &sep_a,
                &tray_apps_sub,
                &tray_view_sub,
                &sep_b,
                &dlfolder_i,
                &dllast_i,
                &notify_i,
                &sep_c,
                &upd_i,
                &dl_i,
                &sep_d,
                &quit_i,
            ];
            let menu = Menu::with_items(app, &tray_items)?;
            drop(tray_items);
            notify_items.push(notify_i);

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
                });
            }

            // ----- Acilista sessiz guncelleme denetimi -----
            // Guncelleme varsa onay diyalogu cikar; kullanici "Daha sonra" derse
            // kalici bir "atla" kaydi TUTULMAZ — bir sonraki acilista tekrar sorulur.
            // Gecikmeli baslar ki acilis/ilk sayfa yuklemesi yavaslamasin.
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(UPDATE_CHECK_DELAY);
                    tauri::async_runtime::spawn(async move {
                        run_update_flow(h, false).await;
                    });
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
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
        })
        .run(tauri::generate_context!())
        .expect("Bogahost Tauri uygulamasi calistirilirken hata");
}

// ---------------------------------------------------------------------------
// Ana pencere: indirme + gezinme + yukleniyor gostergesi
// ---------------------------------------------------------------------------

/// Ana pencereyi olusturur. Pencere BASLANGICTA GIZLIDIR; ilk sayfa yuklenince
/// (veya `WINDOW_REVEAL_FALLBACK` dolunca) gosterilir — boylece acilista
/// beyaz/donuk bir kare gorunmez.
fn build_main_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow<Wry>> {
    let start = APPS
        .iter()
        .find(|e| e.0 == APP_KEY)
        .map(|e| e.2)
        .unwrap_or("https://bogahost.com/");
    let url = Url::parse(start).expect("baslangic URL'i gecerli olmali");
    let nav_handle = app.clone();

    WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title(APP_TITLE)
        .inner_size(1280.0, 860.0)
        .min_inner_size(960.0, 640.0)
        .resizable(true)
        .center()
        .visible(false)
        .theme(Some(tauri::Theme::Dark))
        .zoom_hotkeys_enabled(true)
        .initialization_script(INIT_SCRIPT)
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
                "http" | "https" | "tauri" | "file" | "about" | "data" | "blob" | "asset"
                | "ipc" => true,
                _ => {
                    let _ = nav_handle.shell().open(url.to_string(), None);
                    false
                }
            }
        })
        .on_page_load(|window, payload| {
            if matches!(payload.event(), PageLoadEvent::Finished) {
                reveal_window(window.app_handle());
                // Uygulama gecisinde gosterilen "Yükleniyor" katmanini kaldir.
                let _ = window.eval(HIDE_LOADING_SCRIPT);
            }
        })
        .on_download(|webview, event| {
            let app = webview.app_handle().clone();
            match event {
                // Indirme baslamadan once hedefi belirle: Indirilenler klasoru,
                // ad cakismasinda "-1", "-2" ...
                DownloadEvent::Requested { url, destination } => {
                    let suggested = destination
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| file_name_from_url(&url));
                    let target = unique_path(&downloads_dir(&app), &sanitize_file_name(&suggested));
                    remember_download(&app, &target);
                    *destination = target;
                    true
                }
                DownloadEvent::Finished { url, path, success } => {
                    if success {
                        // macOS'ta `path` None olabilir; o zaman istekte kaydettigimiz yolu kullaniriz.
                        let saved = path.or_else(|| last_download(&app));
                        match saved {
                            Some(p) => {
                                remember_download(&app, &p);
                                notify_download_saved(&app, &p);
                            }
                            None => notify(
                                &app,
                                "İndirildi",
                                "Dosya İndirilenler klasörüne kaydedildi.",
                            ),
                        }
                    } else {
                        notify(
                            &app,
                            "İndirme başarısız",
                            &format!("Dosya indirilemedi: {}", file_name_from_url(&url)),
                        );
                    }
                    true
                }
                _ => true,
            }
        })
        .build()
}

/// Pencereyi (bir kez) gorunur yapar.
fn reveal_window(app: &AppHandle) {
    if WINDOW_REVEALED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
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
// Sayfa tarafi kopru (initialization script)
// ---------------------------------------------------------------------------

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

  function openExternal(href) {
    return invoke('bogahost_open_external', { url: href });
  }

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

    var target = (a.getAttribute('target') || '').toLowerCase();
    if (isHttp && target && target !== '_self' && target !== '_top' && target !== '_parent') {
      // Yeni sekme WebView'de acilmaz -> sessizce yutulmasin.
      ev.preventDefault();
      try { location.href = abs.href; } catch (e4) {}
    }
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
    try { location.href = abs.href; } catch (e3) {}
    return null;
  };
})();
"#;

/// Uygulama gecisinde gosterilen basit "Yükleniyor" katmani.
const SHOW_LOADING_SCRIPT: &str = r#"
(function () {
  try {
    if (document.getElementById('bogahost-native-loading')) { return; }
    var d = document.createElement('div');
    d.id = 'bogahost-native-loading';
    d.setAttribute('style', 'position:fixed;inset:0;z-index:2147483647;display:flex;align-items:center;justify-content:center;background:#0e1015;color:#e6e8ee;font:15px -apple-system,"Segoe UI",Roboto,Arial,sans-serif;');
    d.textContent = 'Yükleniyor…';
    (document.body || document.documentElement).appendChild(d);
  } catch (e) {}
})();
"#;

const HIDE_LOADING_SCRIPT: &str = r#"
(function () {
  try {
    var d = document.getElementById('bogahost-native-loading');
    if (d && d.parentNode) { d.parentNode.removeChild(d); }
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

fn notify_download_saved(app: &AppHandle, path: &Path) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("dosya")
        .to_string();
    notify(
        app,
        &format!("İndirildi: {name}"),
        "İndirilenler klasörüne kaydedildi. Tepsi menüsünden \"Son indirilen dosyayı göster\".",
    );
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
    let back = menu_item(app, "view-back", "Geri", Some("CmdOrCtrl+BracketLeft"))?;
    let forward = menu_item(app, "view-forward", "İleri", Some("CmdOrCtrl+BracketRight"))?;
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
        &sep1,
        &back,
        &forward,
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
    let services = PredefinedMenuItem::services(app, Some("Hizmetler"))?;
    let hide = PredefinedMenuItem::hide(app, Some(hide_label.as_str()))?;
    let hide_others = PredefinedMenuItem::hide_others(app, Some("Diğerlerini Gizle"))?;
    let show_all = PredefinedMenuItem::show_all(app, Some("Tümünü Göster"))?;
    let quit = PredefinedMenuItem::quit(app, Some("Çıkış"))?;
    let a1 = PredefinedMenuItem::separator(app)?;
    let a2 = PredefinedMenuItem::separator(app)?;
    let a3 = PredefinedMenuItem::separator(app)?;
    let app_items: Vec<&dyn IsMenuItem<Wry>> = vec![
        &about,
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
        "view-zoom-in" => apply_zoom(app, 0.1),
        "view-zoom-out" => apply_zoom(app, -0.1),
        "view-zoom-reset" => set_zoom(app, 1.0),
        "downloads-folder" => {
            let dir = downloads_dir(app);
            let _ = app.shell().open(dir.to_string_lossy().to_string(), None);
        }
        "downloads-last" => {
            // macOS/Windows: dosyanin bulundugu klasoru ac.
            let target = last_download(app)
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| downloads_dir(app));
            let _ = app.shell().open(target.to_string_lossy().to_string(), None);
        }
        "notify-status" => {
            let granted = notification_granted(app);
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

    // Once "Yükleniyor" katmani — gecis sirasinda donma hissi olmasin.
    let _ = window.eval(SHOW_LOADING_SCRIPT);

    if window.navigate(url).is_err() {
        let _ = window.eval(HIDE_LOADING_SCRIPT);
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
    if !granted {
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
}

/// Tepsi menusundeki "Bildirimler: ..." ogesinin etiketini gunceller.
fn refresh_notification_menu(app: &AppHandle) {
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
