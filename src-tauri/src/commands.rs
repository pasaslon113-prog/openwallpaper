use crate::{
    library,
    model::{AppSettings, MonitorInfo, RuntimeWallpaper, SceneManifest, WallpaperItem, WallpaperKind, WorkshopItem},
    playback::MpvPlayer,
    state::AppState,
    steam,
    windows::desktop,
};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
pub fn get_library(state: tauri::State<'_, AppState>, query: Option<String>, kind: Option<String>, favorites_only: Option<bool>) -> Result<Vec<WallpaperItem>, String> {
    let data = state.data.lock().map_err(|e| e.to_string())?;
    let query = query.unwrap_or_default().to_lowercase();
    let kind = kind.unwrap_or_default();
    let favorites_only = favorites_only.unwrap_or(false);
    Ok(data.wallpapers.iter().filter(|item| {
        let matches_query = query.is_empty()
            || item.title.to_lowercase().contains(&query)
            || item.tags.iter().any(|tag| tag.to_lowercase().contains(&query));
        let matches_kind = kind.is_empty() || kind == "all" || format!("{:?}", item.kind).eq_ignore_ascii_case(&kind);
        matches_query && matches_kind && (!favorites_only || item.favorite)
    }).cloned().collect())
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.data.lock().map_err(|e| e.to_string())?.settings.clone())
}

#[tauri::command]
pub fn get_collections(state: tauri::State<'_, AppState>) -> Result<Vec<crate::model::Collection>, String> {
    Ok(state.data.lock().map_err(|e| e.to_string())?.collections.clone())
}

#[tauri::command]
pub fn get_monitors(app: AppHandle) -> Result<Vec<MonitorInfo>, String> {
    monitors_for_app(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_library_folders(app: AppHandle, state: tauri::State<'_, AppState>, paths: Vec<String>) -> Result<usize, String> {
    let _scan_guard = state.library_scan_lock.lock().map_err(|e| e.to_string())?;
    let mut snapshot = {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        library::add_roots(&mut data, &paths).map_err(|e| e.to_string())?;
        data.clone()
    };
    let cache_dir = state.app_data_dir.join("thumbnails");
    library::scan_library(&mut snapshot, &cache_dir).map_err(|e| e.to_string())?;
    {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        let favorites: std::collections::HashMap<String, bool> = data.wallpapers.iter()
            .map(|item| (item.id.clone(), item.favorite)).collect();
        for item in &mut snapshot.wallpapers {
            if let Some(favorite) = favorites.get(&item.id) { item.favorite = *favorite; }
        }
        data.library_roots = snapshot.library_roots;
        data.wallpapers = snapshot.wallpapers;
    }
    for path in paths {
        let _ = app.asset_protocol_scope().allow_directory(PathBuf::from(path), true);
    }
    let _ = app.asset_protocol_scope().allow_directory(cache_dir, true);
    state.save().map_err(|e| e.to_string())?;
    Ok(state.data.lock().map_err(|e| e.to_string())?.wallpapers.len())
}

#[tauri::command]
pub fn rescan_library(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let _scan_guard = state.library_scan_lock.lock().map_err(|e| e.to_string())?;
    let mut snapshot = state.data.lock().map_err(|e| e.to_string())?.clone();
    let cache_dir = state.app_data_dir.join("thumbnails");
    let count = library::scan_library(&mut snapshot, &cache_dir).map_err(|e| e.to_string())?;
    {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        let favorites: std::collections::HashMap<String, bool> = data.wallpapers.iter()
            .map(|item| (item.id.clone(), item.favorite)).collect();
        for item in &mut snapshot.wallpapers {
            if let Some(favorite) = favorites.get(&item.id) { item.favorite = *favorite; }
        }
        data.wallpapers = snapshot.wallpapers;
    }
    for root in snapshot.library_roots {
        let _ = app.asset_protocol_scope().allow_directory(PathBuf::from(root), true);
    }
    let _ = app.asset_protocol_scope().allow_directory(cache_dir, true);
    state.save().map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
pub fn add_web_wallpaper(state: tauri::State<'_, AppState>, url: String, title: String) -> Result<WallpaperItem, String> {
    let parsed = url::Url::parse(&url).map_err(|_| "Некорректный URL".to_string())?;
    if !matches!(parsed.scheme(), "https" | "http") { return Err("Разрешены только HTTP и HTTPS".into()); }
    let safe_title = title.trim();
    if safe_title.is_empty() { return Err("Укажите название веб-обоев".into()); }
    let id = blake3::hash(url.as_bytes()).to_hex()[..20].to_string();
    let item = WallpaperItem {
        id,
        title: safe_title.to_string(),
        kind: WallpaperKind::Web,
        path: url.clone(),
        preview_data_uri: None,
        preview_path: None,
        source: "Веб-обои".into(),
        favorite: false,
        tags: vec!["web".into()],
        description: Some(url),
        file_count: 1,
    };
    let mut data = state.data.lock().map_err(|e| e.to_string())?;
    data.wallpapers.retain(|existing| existing.id != item.id);
    data.wallpapers.push(item.clone());
    drop(data);
    state.save().map_err(|e| e.to_string())?;
    Ok(item)
}

#[tauri::command]
pub fn toggle_favorite(state: tauri::State<'_, AppState>, wallpaper_id: String) -> Result<bool, String> {
    let value = {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        let item = data.wallpapers.iter_mut().find(|item| item.id == wallpaper_id).ok_or("Обои не найдены")?;
        item.favorite = !item.favorite;
        item.favorite
    };
    state.save().map_err(|e| e.to_string())?;
    Ok(value)
}

#[tauri::command]
pub fn create_collection(state: tauri::State<'_, AppState>, name: String) -> Result<crate::model::Collection, String> {
    let name = name.trim();
    if name.is_empty() { return Err("Название подборки не может быть пустым".into()); }
    let id = format!("collection-{}", blake3::hash(format!("{}-{:?}", name, std::time::SystemTime::now()).as_bytes()).to_hex()[..12].to_string());
    let collection = crate::model::Collection { id, name: name.to_string(), wallpaper_ids: Vec::new() };
    state.data.lock().map_err(|e| e.to_string())?.collections.push(collection.clone());
    state.save().map_err(|e| e.to_string())?;
    Ok(collection)
}

#[tauri::command]
pub fn add_to_collection(state: tauri::State<'_, AppState>, collection_id: String, wallpaper_id: String) -> Result<(), String> {
    let mut data = state.data.lock().map_err(|e| e.to_string())?;
    if !data.wallpapers.iter().any(|w| w.id == wallpaper_id) { return Err("Обои не найдены".into()); }
    let collection = data.collections.iter_mut().find(|c| c.id == collection_id).ok_or("Подборка не найдена")?;
    if !collection.wallpaper_ids.contains(&wallpaper_id) { collection.wallpaper_ids.push(wallpaper_id); }
    drop(data);
    state.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, state: tauri::State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    let settings = settings.normalized();

    #[cfg(windows)]
    set_autostart(settings.autostart).map_err(|e| e.to_string())?;

    let (previous, assignments) = {
        let mut data = state.data.lock().map_err(|e| e.to_string())?;
        let previous = data.settings.clone();
        data.settings = settings.clone();
        (previous, settings.monitor_wallpapers.clone())
    };

    if let Ok(mut players) = state.playback.lock() {
        for player in players.values_mut() {
            player.set_volume(settings.volume, settings.muted).map_err(|e| e.to_string())?;
        }
    }

    let _ = app.emit("ow:settings-updated", &settings);
    let _ = app.emit("ow:audio-settings", serde_json::json!({
        "volume": settings.volume,
        "muted": settings.muted
    }));
    state.save().map_err(|e| e.to_string())?;

    // Recreate expensive desktop surfaces only when playback timing actually changed.
    let playback_timing_changed = playback_timing_changed(&previous, &settings);
    if playback_timing_changed {
        let active_monitors: std::collections::HashSet<String> = monitors_for_app(&app)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|monitor| monitor.id)
            .collect();
        for (monitor_id, wallpaper_id) in assignments {
            if !active_monitors.contains(&monitor_id) { continue; }
            let state_for_apply = app.state::<AppState>();
            apply_wallpaper(app.clone(), state_for_apply, wallpaper_id, monitor_id).await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn apply_wallpaper(app: AppHandle, state: tauri::State<'_, AppState>, wallpaper_id: String, monitor_id: String) -> Result<String, String> {
    let (item, settings) = {
        let data = state.data.lock().map_err(|e| e.to_string())?;
        let item = data.wallpapers.iter().find(|item| item.id == wallpaper_id).cloned().ok_or("Обои не найдены")?;
        (item, data.settings.clone())
    };
    let paused = state.paused.lock().map(|value| *value).unwrap_or(false);
    let monitors = monitors_for_app(&app).map_err(|e| e.to_string())?;
    let selected: Vec<MonitorInfo> = if monitor_id == "all" {
        monitors
    } else {
        monitors.into_iter().filter(|m| m.id == monitor_id).collect()
    };
    if selected.is_empty() { return Err("Выбранный монитор не найден".into()); }

    let mut created = Vec::new();
    for monitor in selected {
        let runtime = build_runtime_wallpaper(&item, &settings, &monitor.id, paused).map_err(|e| e.to_string())?;
        let label = wallpaper_window_label(&monitor.id);
        if let Some(old) = app.get_webview_window(&label) {
            if let Ok(mut players) = state.playback.lock() { players.remove(&label); }
            let _ = old.destroy();
        }
        let init = format!("if (window === window.top) {{ window.__OPEN_WALLPAPER__ = {}; }}", serde_json::to_string(&runtime).map_err(|e| e.to_string())?);
        let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("wallpaper.html".into()))
            .title("OpenWallpaper Surface")
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .visible(false)
            .initialization_script(&init)
            .build()
            .map_err(|e| format!("Не удалось создать поверхность рабочего стола: {e}"))?;
        window.set_position(PhysicalPosition::new(monitor.x, monitor.y)).map_err(|e| e.to_string())?;
        window.set_size(PhysicalSize::new(monitor.width, monitor.height)).map_err(|e| e.to_string())?;

        #[cfg(windows)]
        {
            let hwnd = window.hwnd().map_err(|e| e.to_string())?;
            desktop::attach_to_desktop(hwnd.0, monitor.x, monitor.y, monitor.width, monitor.height).map_err(|e| e.to_string())?;
            if runtime.kind != WallpaperKind::Web && runtime.kind != WallpaperKind::Scene {
                let mut player = MpvPlayer::start(hwnd.0 as isize, &runtime, &state.app_data_dir).map_err(|e| e.to_string())?;
                if paused { let _ = player.set_paused(true); }
                state.playback.lock().map_err(|e| e.to_string())?.insert(label.clone(), player);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = desktop::attach_to_desktop((), monitor.x, monitor.y, monitor.width, monitor.height);
        }
        window.show().map_err(|e| e.to_string())?;
        created.push(monitor.name);
        {
            let mut data = state.data.lock().map_err(|e| e.to_string())?;
            data.settings.monitor_wallpapers.insert(monitor.id.clone(), wallpaper_id.clone());
        }
    }
    state.save().map_err(|e| e.to_string())?;
    Ok(format!("Обои применены: {}", created.join(", ")))
}

#[tauri::command]
pub fn pause_wallpapers(app: AppHandle, state: tauri::State<'_, AppState>, paused: bool) -> Result<(), String> {
    if let Ok(mut manual) = state.manual_paused.lock() { *manual = paused; }
    let settings = state.data.lock().map_err(|e| e.to_string())?.settings.clone();
    let automatic_reason = crate::windows::system_events::pause_reason(&settings);
    let effective_pause = paused || automatic_reason.is_some();
    if let Ok(mut value) = state.paused.lock() { *value = effective_pause; }
    if let Ok(mut players) = state.playback.lock() {
        for player in players.values_mut() { let _ = player.set_paused(effective_pause); }
    }
    for window in app.webview_windows().values() {
        if window.label().starts_with("wallpaper-") { let _ = window.emit("ow:pause", effective_pause); }
    }
    let reason = if paused { "manual" } else { automatic_reason.as_deref().unwrap_or("manual") };
    let _ = app.emit("ow:pause-state", serde_json::json!({"paused": effective_pause, "reason": reason}));
    Ok(())
}

#[tauri::command]
pub async fn search_workshop(state: tauri::State<'_, AppState>, query: String, page: Option<u32>) -> Result<Vec<WorkshopItem>, String> {
    let roots = state.data.lock().map_err(|e| e.to_string())?.library_roots.clone();
    steam::search_workshop(&query, page.unwrap_or(1), &roots).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn workshop_item_details(published_file_id: String) -> Result<serde_json::Value, String> {
    steam::get_published_file_details(&published_file_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_workshop_item(published_file_id: String) -> Result<(), String> {
    steam::open_workshop_item(&published_file_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|_| "Некорректный URL".to_string())?;
    if !matches!(parsed.scheme(), "https" | "http") { return Err("Разрешены только HTTP и HTTPS".into()); }
    open::that(parsed.as_str()).map_err(|e| e.to_string())
}

fn playback_timing_changed(previous: &AppSettings, next: &AppSettings) -> bool {
    previous.fps_limit != next.fps_limit
        || previous.monitor_fps != next.monitor_fps
        || previous.slideshow_seconds != next.slideshow_seconds
}

fn build_runtime_wallpaper(item: &WallpaperItem, settings: &AppSettings, monitor_id: &str, paused: bool) -> anyhow::Result<RuntimeWallpaper> {
    let fps_limit = settings.monitor_fps.get(monitor_id).copied().unwrap_or(settings.fps_limit).clamp(10, 120);
    let mut path = item.path.clone();
    let mut url = None;
    let mut scene: Option<SceneManifest> = None;
    if item.kind == WallpaperKind::Web {
        if item.path.starts_with("https://") || item.path.starts_with("http://") {
            url = Some(item.path.clone());
        } else {
            let p = PathBuf::from(&item.path);
            path = if p.is_dir() { p.join("index.html").to_string_lossy().to_string() } else { item.path.clone() };
        }
    }
    if item.kind == WallpaperKind::Scene {
        let folder = PathBuf::from(&item.path);
        let raw = std::fs::read_to_string(folder.join("scene.json"))?;
        let mut manifest: SceneManifest = serde_json::from_str(&raw)?;
        for layer in &mut manifest.layers {
            if !matches!(layer.layer_type.as_str(), "image" | "video" | "web") {
                anyhow::bail!("Неподдерживаемый тип слоя сцены: {}", layer.layer_type);
            }
            if layer.source.trim().is_empty() { anyhow::bail!("У слоя сцены отсутствует source"); }
            if layer.layer_type == "web" && (layer.source.starts_with("https://") || layer.source.starts_with("http://")) {
                continue;
            }
            let resolved = library::safe_scene_layer_path(&folder, &layer.source)?;
            layer.source = resolved.to_string_lossy().to_string();
        }
        scene = Some(manifest);
    }
    Ok(RuntimeWallpaper {
        id: item.id.clone(),
        title: item.title.clone(),
        kind: item.kind.clone(),
        path,
        url,
        scene,
        fps_limit,
        paused,
        volume: settings.volume,
        muted: settings.muted,
        slideshow_seconds: settings.slideshow_seconds,
    })
}

pub(crate) fn monitors_for_app(app: &AppHandle) -> anyhow::Result<Vec<MonitorInfo>> {
    let window = app.get_webview_window("main").ok_or_else(|| anyhow::anyhow!("Главное окно приложения недоступно"))?;
    let monitors = window.available_monitors()?;
    let primary = window.primary_monitor()?.map(|m| *m.position());
    Ok(monitors.into_iter().enumerate().map(|(index, monitor)| {
        let pos = monitor.position();
        let size = monitor.size();
        let id = format!("{}:{}:{}:{}", pos.x, pos.y, size.width, size.height);
        let name = monitor.name().cloned().unwrap_or_else(|| format!("Монитор {}", index + 1));
        MonitorInfo {
            id,
            name,
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            scale_factor: monitor.scale_factor(),
            primary: primary.map(|p| p == pos).unwrap_or(index == 0),
        }
    }).collect())
}

pub(crate) async fn reconcile_monitor_wallpapers(app: AppHandle, previous: Vec<MonitorInfo>, current: Vec<MonitorInfo>) {
    if current.is_empty() { return; }
    let state = app.state::<AppState>();
    let old_assignments = state.data.lock()
        .map(|data| data.settings.monitor_wallpapers.clone()).unwrap_or_default();
    if old_assignments.is_empty() { return; }

    let assignments = remap_monitor_assignments(&previous, &current, &old_assignments);

    for window in app.webview_windows().values() {
        if window.label().starts_with("wallpaper-") { let _ = window.destroy(); }
    }
    if let Ok(mut players) = state.playback.lock() { players.clear(); }
    if let Ok(mut data) = state.data.lock() { data.settings.monitor_wallpapers = assignments.clone(); }
    let _ = state.save();

    for (monitor_id, wallpaper_id) in assignments {
        let state = app.state::<AppState>();
        if let Err(error) = apply_wallpaper(app.clone(), state, wallpaper_id.clone(), monitor_id.clone()).await {
            let _ = app.emit("ow:restore-error", serde_json::json!({
                "wallpaperId": wallpaper_id,
                "monitorId": monitor_id,
                "message": error,
            }));
        }
    }
}

fn monitor_distance_squared(a: &MonitorInfo, b: &MonitorInfo) -> i64 {
    let ax = i64::from(a.x) + i64::from(a.width / 2);
    let ay = i64::from(a.y) + i64::from(a.height / 2);
    let bx = i64::from(b.x) + i64::from(b.width / 2);
    let by = i64::from(b.y) + i64::from(b.height / 2);
    (ax - bx).pow(2) + (ay - by).pow(2)
}

fn remap_monitor_assignments(
    previous: &[MonitorInfo],
    current: &[MonitorInfo],
    old_assignments: &std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, String> {
    let fallback = old_assignments.values().next().cloned();
    current.iter().filter_map(|monitor| {
        let wallpaper = if let Some(existing) = old_assignments.get(&monitor.id) {
            Some(existing.clone())
        } else {
            previous.iter().min_by_key(|old| monitor_distance_squared(old, monitor))
                .and_then(|old| old_assignments.get(&old.id)).cloned().or_else(|| fallback.clone())
        }?;
        Some((monitor.id.clone(), wallpaper))
    }).collect()
}

fn wallpaper_window_label(monitor_id: &str) -> String {
    format!("wallpaper-{}", blake3::hash(monitor_id.as_bytes()).to_hex()[..10].to_string())
}

#[cfg(windows)]
fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;
    if enabled {
        let executable = std::env::current_exe()?;
        let value = format!("\"{}\" --background", executable.display());
        key.set_value("OpenWallpaper", &value)?;
    } else {
        let _ = key.delete_value("OpenWallpaper");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_window_label_is_safe_and_stable() {
        let id = "-1920:0:1920:1080";
        assert_eq!(wallpaper_window_label(id), wallpaper_window_label(id));
        assert!(wallpaper_window_label(id).starts_with("wallpaper-"));
    }

    #[test]
    fn changing_only_audio_settings_does_not_recreate_wallpaper_windows() {
        let previous = AppSettings::default();
        let mut next = previous.clone();
        next.volume = 55;
        next.muted = false;
        assert!(!playback_timing_changed(&previous, &next));
    }

    #[test]
    fn changing_fps_or_slideshow_interval_restarts_playback() {
        let previous = AppSettings::default();
        let mut next = previous.clone();
        next.fps_limit = 60;
        assert!(playback_timing_changed(&previous, &next));
        next = previous.clone();
        next.slideshow_seconds = 12;
        assert!(playback_timing_changed(&previous, &next));
    }

    #[test]
    fn monitor_matching_prefers_nearest_center_after_resolution_or_position_change() {
        let old = MonitorInfo { id: "old".into(), name: "A".into(), x: 0, y: 0, width: 1920, height: 1080, scale_factor: 1.0, primary: true };
        let resized = MonitorInfo { id: "new".into(), name: "A".into(), x: 0, y: 0, width: 2560, height: 1440, scale_factor: 1.0, primary: true };
        let distant = MonitorInfo { id: "far".into(), name: "B".into(), x: 4000, y: 0, width: 1920, height: 1080, scale_factor: 1.0, primary: false };
        assert!(monitor_distance_squared(&old, &resized) < monitor_distance_squared(&distant, &resized));
    }

    #[test]
    fn monitor_assignments_survive_resolution_change_and_new_monitor_connection() {
        let old = MonitorInfo { id: "old".into(), name: "A".into(), x: 0, y: 0, width: 1920, height: 1080, scale_factor: 1.0, primary: true };
        let resized = MonitorInfo { id: "new".into(), name: "A".into(), x: 0, y: 0, width: 2560, height: 1440, scale_factor: 1.0, primary: true };
        let added = MonitorInfo { id: "added".into(), name: "B".into(), x: 2560, y: 0, width: 1920, height: 1080, scale_factor: 1.0, primary: false };
        let old_assignments = std::collections::HashMap::from([("old".into(), "wallpaper-1".into())]);
        let mapped = remap_monitor_assignments(&[old], &[resized, added], &old_assignments);
        assert_eq!(mapped.get("new").map(String::as_str), Some("wallpaper-1"));
        assert_eq!(mapped.get("added").map(String::as_str), Some("wallpaper-1"));
    }
}

