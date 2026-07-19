use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

const APP_TITLE: &str = "Bogahost Görevler";

pub fn run() {
    tauri::Builder::default()
        // Harici linkleri sistem tarayicisinda acmak + genel shell erisimi.
        .plugin(tauri_plugin_shell::init())
        // Native masaustu bildirimleri.
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // ----- Sistem tepsisi (tray) menusu -----
            let show_i = MenuItem::with_id(app, "show", "Goster / Show", true, None::<&str>)?;
            let hide_i = MenuItem::with_id(app, "hide", "Gizle / Hide", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Cikis / Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &hide_i, &quit_i])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(
                    app.default_window_icon()
                        .expect("default window icon (icons/icon.png) mevcut olmali")
                        .clone(),
                )
                .tooltip(APP_TITLE)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
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
                    _ => {}
                })
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
