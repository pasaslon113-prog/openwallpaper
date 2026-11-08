#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod builtin;
mod commands;
mod library;
mod model;
mod playback;
mod state;
mod steam;
mod windows;

use state::AppState;
use std::sync::atomic::Ordering;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};


fn report_restore_error(app: &tauri::AppHandle, payload: serde_json::Value) {
    let message = payload.get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Неизвестная ошибка восстановления обоев")
        .to_string();
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut errors) = state.startup_errors.lock() {
            errors.push(message);
        }
    }
    let _ = app.emit("ow:restore-error", payload);
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let path = std::env::temp_dir().join("openwallpaper-startup-error.log");
        let _ = std::fs::write(&path, format!("{info}\n"));
    }));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let builtin_wallpapers = builtin::install(&app_data)?;
            let state = AppState::load(app_data.clone())?;
            #[cfg(windows)]
            let discovered_roots = windows::steam_paths::discover_wallpaper_engine_roots();
            #[cfg(not(windows))]
            let discovered_roots: Vec<String> = Vec::new();
            {
                let mut data = state.data.lock().map_err(|_| "state lock poisoned")?;
                library::add_roots(&mut data, &discovered_roots)?;
                library::add_roots(&mut data, &[builtin_wallpapers.to_string_lossy().into_owned()])?;
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
            std::thread::spawn(move || {
                let state = app_handle.state::<AppState>();
                let Ok(_scan_guard) = state.library_scan_lock.blocking_lock() else {
                    state.library_scan_complete.store(true, Ordering::Release);
                    report_restore_error(&app_handle, serde_json::json!({
                        "message": "Не удалось получить блокировку сканирования библиотеки",
                    }));
                    let _ = app_handle.emit("ow:library-ready", ());
                    return;
                };
                let mut snapshot = match state.data.lock() {
                    Ok(data) => data.clone(),
                    Err(_) => {
                        state.library_scan_complete.store(true, Ordering::Release);
                        report_restore_error(&app_handle, serde_json::json!({
                            "message": "Не удалось прочитать состояние библиотеки",
                        }));
                        let _ = app_handle.emit("ow:library-ready", ());
                        return;
                    }
                };
                if let Err(error) = library::scan_library(&mut snapshot, &state.app_data_dir.join("thumbnails")) {
                    state.library_scan_complete.store(true, Ordering::Release);
                    report_restore_error(&app_handle, serde_json::json!({
                        "message": format!("Не удалось просканировать библиотеку: {error}"),
                    }));
                    let _ = app_handle.emit("ow:library-ready", ());
                    return;
                }
                {
                    let Ok(mut data) = state.data.lock() else { return; };
                    let favorites: std::collections::HashMap<String, bool> = data.wallpapers.iter()
                        .map(|item| (item.id.clone(), item.favorite)).collect();
                    for item in &mut snapshot.wallpapers {
                        if let Some(favorite) = favorites.get(&item.id) { item.favorite = *favorite; }
                    }
                    data.wallpapers = snapshot.wallpapers;
                }
                for root in snapshot.library_roots {
                    let _ = app_handle.asset_protocol_scope().allow_directory(std::path::PathBuf::from(root), true);
                }
                let _ = app_handle.asset_protocol_scope().allow_directory(state.app_data_dir.join("thumbnails"), true);
                if let Err(error) = state.save() {
                    report_restore_error(&app_handle, serde_json::json!({
                        "message": format!("Не удалось сохранить индекс библиотеки: {error}"),
                    }));
                }
                state.library_scan_complete.store(true, Ordering::Release);
                let _ = app_handle.emit("ow:library-ready", ());
                drop(_scan_guard);

                let mut assignments = state.data.lock()
                    .map(|data| data.settings.monitor_wallpapers.clone()).unwrap_or_default();
                if assignments.is_empty() {
                    let default_id = state.data.lock().ok()
                        .and_then(|data| builtin::default_wallpaper_id(&data.wallpapers));
                    let primary = commands::get_monitors(app_handle.clone()).ok()
                        .and_then(|monitors| monitors.into_iter().find(|monitor| monitor.primary));
                    if let (Some(wallpaper_id), Some(monitor)) = (default_id, primary) {
                        if let Ok(mut data) = state.data.lock() {
                            data.settings.monitor_wallpapers.insert(monitor.id, wallpaper_id);
                            assignments = data.settings.monitor_wallpapers.clone();
                        }
                        if let Err(error) = state.save() {
                            report_restore_error(&app_handle, serde_json::json!({
                                "message": format!("Не удалось сохранить стартовые обои: {error}"),
                            }));
                        }
                    }
                }
                tauri::async_runtime::spawn(async move {
                    for (monitor_id, wallpaper_id) in assignments {
                        let state = app_handle.state::<AppState>();
                        if let Err(error) = commands::apply_wallpaper(app_handle.clone(), state, wallpaper_id.clone(), monitor_id.clone()).await {
                            report_restore_error(&app_handle, serde_json::json!({
                                "wallpaperId": wallpaper_id,
                                "monitorId": monitor_id,
                                "message": error,
                            }));
                        }
                    }
                });
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
            commands::is_library_scan_complete,
            commands::get_pause_state,
            commands::take_startup_errors,
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
        .unwrap_or_else(|error| {
            let path = std::env::temp_dir().join("openwallpaper-startup-error.log");
            let _ = std::fs::write(&path, format!("OpenWallpaper failed to start: {error:#}\n"));
            panic!("OpenWallpaper failed to start: {error:#}");
        });
}

