use crate::{model::AppSettings, state::AppState};
use std::{thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

pub fn spawn(app: AppHandle) {
    let initial_paused = if let Some(state) = app.try_state::<AppState>() {
        let settings = state.data.lock().map(|data| data.settings.clone()).unwrap_or_default();
        let manual_pause = state.manual_paused.lock().map(|value| *value).unwrap_or(false);
        let paused = manual_pause || pause_reason(&settings).is_some();
        if let Ok(mut current) = state.paused.lock() { *current = paused; }
        paused
    } else { false };
    thread::spawn(move || {
        let mut last_pause = Some(initial_paused);
        let mut last_monitors: Option<Vec<crate::model::MonitorInfo>> = None;
        loop {
            thread::sleep(Duration::from_secs(4));
            let Some(state) = app.try_state::<AppState>() else { continue; };
            if let Ok(current_monitors) = crate::commands::monitors_for_app(&app) {
                let monitors_changed = last_monitors.as_ref().is_some_and(|previous| previous != &current_monitors);
                let explorer_restarted = app.webview_windows().values().any(|window| {
                    if !window.label().starts_with("wallpaper-") { return false; }
                    #[cfg(windows)]
                    { window.hwnd().map(|hwnd| !crate::windows::desktop::has_live_workerw_parent(hwnd.0)).unwrap_or(true) }
                    #[cfg(not(windows))]
                    { false }
                });
                if monitors_changed || explorer_restarted {
                    let previous = last_monitors.clone().unwrap_or_default();
                    let app_for_reconcile = app.clone();
                    let next = current_monitors.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::commands::reconcile_monitor_wallpapers(app_for_reconcile, previous, next).await;
                    });
                }
                last_monitors = Some(current_monitors);
            }
            let settings = match state.data.lock() {
                Ok(data) => data.settings.clone(),
                Err(_) => continue,
            };
            let manual_pause = state.manual_paused.lock().map(|v| *v).unwrap_or(false);
            let automatic_reason = pause_reason(&settings);
            let should_pause = manual_pause || automatic_reason.is_some();
            let reason = if manual_pause { Some("Ручная пауза".to_string()) } else { automatic_reason };
            if last_pause != Some(should_pause) {
                last_pause = Some(should_pause);
                if let Ok(mut paused) = state.paused.lock() { *paused = should_pause; }
                if let Ok(mut players) = state.playback.lock() {
                    for player in players.values_mut() { let _ = player.set_paused(should_pause); }
                }
                for window in app.webview_windows().values() {
                    if window.label().starts_with("wallpaper-") { let _ = window.emit("ow:pause", should_pause); }
                }
                let _ = app.emit("ow:pause-state", serde_json::json!({"paused": should_pause, "reason": reason}));
            }
        }
    });
}

pub fn pause_reason(settings: &AppSettings) -> Option<String> {
    #[cfg(windows)]
    {
        if settings.pause_in_games {
            if let Some(process) = foreground_process_name() {
                if settings.game_processes.iter().any(|name| name.eq_ignore_ascii_case(&process)) {
                    return Some(format!("Игра: {process}"));
                }
            }
        }
        if settings.pause_on_lock && is_lock_screen_foreground() {
            return Some("Экран заблокирован".into());
        }
        if settings.pause_on_battery && running_on_battery() {
            return Some("Работа от батареи".into());
        }
    }
    #[cfg(not(windows))]
    let _ = settings;
    None
}

#[cfg(windows)]
fn foreground_process_name() -> Option<String> {
    use std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" { fn GetForegroundWindow() -> *mut c_void; fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32; }
    #[link(name = "kernel32")]
    extern "system" { fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void; fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, name: *mut u16, size: *mut u32) -> i32; fn CloseHandle(handle: *mut c_void) -> i32; }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() { return None; }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        if pid == 0 { return None; }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() { return None; }
        let mut buffer = vec![0u16; 32768];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size);
        let _ = CloseHandle(process);
        if result == 0 { return None; }
        let full = String::from_utf16_lossy(&buffer[..size as usize]);
        full.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
    }
}

#[cfg(windows)]
fn is_lock_screen_foreground() -> bool {
    matches!(foreground_process_name().as_deref(), Some("lockapp.exe") | Some("logonui.exe"))
}

#[cfg(windows)]
fn running_on_battery() -> bool {
    #[repr(C)]
    struct PowerStatus { ac_line_status: u8, battery_flag: u8, battery_percent: u8, system_status_flag: u8, battery_life_time: u32, battery_full_life_time: u32 }
    #[link(name = "kernel32")]
    extern "system" { fn GetSystemPowerStatus(status: *mut PowerStatus) -> i32; }
    unsafe {
        let mut status = PowerStatus { ac_line_status: 255, battery_flag: 128, battery_percent: 255, system_status_flag: 0, battery_life_time: u32::MAX, battery_full_life_time: u32::MAX };
        GetSystemPowerStatus(&mut status) != 0 && status.ac_line_status == 0 && status.battery_flag != 128
    }
}

