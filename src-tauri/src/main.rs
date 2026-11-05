#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod library;
mod model;
mod playback;
mod state;
mod steam;
mod windows;

use state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let state = AppState::load(app_data.clone())?;
            #[cfg(windows)]
            let discovered_roots = windows::steam_paths::discover_wallpaper_engine_roots();
            #[cfg(not(windows))]
            let discovered_roots: Vec<String> = Vec::new();
            {
                let mut data = state.data.lock().map_err(|_| "state lock poisoned")?;
                library::add_roots(&mut data, &discovered_roots)?;
                library::scan_library(&mut data, &app_data.join("thumbnails"))?;
            }
            app.manage(state);
            {
                let state = app.state::<AppState>();
                let roots = state.data.lock().map_err(|_| "state lock poisoned")?.library_roots.clone();
                for root in roots {
                    let _ = app.asset_protocol_scope().allow_directory(std::path::PathBuf::from(root), true);
                }
                let _ = app.asset_protocol_scope().allow_directory(app_data.join("thumbnails"), true);
                state.save()?;
            }

            let open_item = MenuItem::with_id(app, "open", "Открыть OpenWallpaper", true, None::<&str>)?;
            let pause_item = MenuItem::with_id(app, "pause", "Пауза / продолжить", true, None::<&str>)?;
            let exit_item = MenuItem::with_id(app, "exit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &pause_item, &exit_item])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("OpenWallpaper")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "pause" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            let current = state.paused.lock().map(|v| *v).unwrap_or(false);
                            let state = app.state::<AppState>();
                            let _ = commands::pause_wallpapers(app.clone(), state, !current);
                        }
                    }
                    "exit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() { tray = tray.icon(icon.clone()); }
            tray.build(app)?;

            windows::system_events::spawn(app.handle().clone());
            let background = std::env::args().any(|arg| arg == "--background");
            let start_minimized = app.state::<AppState>().data.lock().map(|d| d.settings.start_minimized).unwrap_or(false);
            if let Some(window) = app.get_webview_window("main") {
                if background || start_minimized { let _ = window.hide(); }
                else { let _ = window.show(); }
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let assignments = app_handle.state::<AppState>().data.lock()
                    .map(|data| data.settings.monitor_wallpapers.clone()).unwrap_or_default();
                for (monitor_id, wallpaper_id) in assignments {
                    let state = app_handle.state::<AppState>();
                    if let Err(error) = commands::apply_wallpaper(app_handle.clone(), state, wallpaper_id.clone(), monitor_id.clone()).await {
                        let _ = app_handle.emit("ow:restore-error", serde_json::json!({
                            "wallpaperId": wallpaper_id,
                            "monitorId": monitor_id,
                            "message": error,
                        }));
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_library,
            commands::get_settings,
            commands::get_collections,
            commands::get_monitors,
            commands::add_library_folders,
            commands::rescan_library,
            commands::add_web_wallpaper,
            commands::toggle_favorite,
            commands::create_collection,
            commands::add_to_collection,
            commands::update_settings,
            commands::apply_wallpaper,
            commands::pause_wallpapers,
            commands::search_workshop,
            commands::workshop_item_details,
            commands::open_workshop_item,
            commands::open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("OpenWallpaper failed to start");
}

