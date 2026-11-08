use crate::{model::PersistedData, playback::MpvPlayer};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{atomic::AtomicBool, Mutex},
};

pub struct AppState {
    pub data: Mutex<PersistedData>,
    pub playback: Mutex<HashMap<String, MpvPlayer>>,
    /// Active desktop surface label per monitor. Labels are staged uniquely so a new surface
    /// can be prepared before the currently visible wallpaper is replaced.
    pub active_surfaces: Mutex<HashMap<String, String>>,
    pub data_file: PathBuf,
    pub app_data_dir: PathBuf,
    pub paused: Mutex<bool>,
    pub manual_paused: Mutex<bool>,
    pub library_scan_lock: Mutex<()>,
    /// True when the initial library scan has either completed or failed.
    pub library_scan_complete: AtomicBool,
    /// Startup and restore failures retained until the UI explicitly acknowledges them.
    pub startup_errors: Mutex<Vec<String>>,
    save_lock: Mutex<()>,
}

impl AppState {
    pub fn load(app_data_dir: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&app_data_dir)?;
        let data_file = app_data_dir.join("openwallpaper.json");
        let (data, startup_errors) = if data_file.exists() {
            let raw = fs::read_to_string(&data_file)?;
            match serde_json::from_str::<PersistedData>(&raw) {
                Ok(data) => (data, Vec::new()),
                Err(error) => {
                    // Preserve the original file instead of silently discarding user data.
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_secs();
                    let backup = app_data_dir.join(format!("openwallpaper.corrupt-{timestamp}.json"));
                    fs::copy(&data_file, &backup)?;
                    (
                        PersistedData::default(),
                        vec![format!("Файл настроек повреждён. Копия сохранена: {}. Причина: {error}", backup.display())],
                    )
                }
            }
        } else {
            (PersistedData::default(), Vec::new())
        };
        Ok(Self {
            data: Mutex::new(data),
            playback: Mutex::new(HashMap::new()),
            active_surfaces: Mutex::new(HashMap::new()),
            data_file,
            app_data_dir,
            paused: Mutex::new(false),
            manual_paused: Mutex::new(false),
            library_scan_lock: Mutex::new(()),
            library_scan_complete: AtomicBool::new(false),
            startup_errors: Mutex::new(startup_errors),
            save_lock: Mutex::new(()),
        })
    }

    /// Writes a consistent snapshot and atomically replaces the previous settings file.
    /// The previous JSON remains intact if serialization or writing fails.
    pub fn save(&self) -> anyhow::Result<()> {
        let _save_guard = self.save_lock.lock()
            .map_err(|_| anyhow::anyhow!("save lock poisoned"))?;
        let snapshot = self.data.lock()
            .map_err(|_| anyhow::anyhow!("state lock poisoned"))?
            .clone();
        let serialized = serde_json::to_vec_pretty(&snapshot)?;
        let temporary = self.data_file.with_extension("json.tmp");

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&serialized)?;
        file.sync_all()?;
        drop(file);

        replace_file(&temporary, &self.data_file)?;
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &std::path::Path, destination: &std::path::Path) -> anyhow::Result<()> {
    fs::rename(source, destination)?;
    Ok(())
}

#[cfg(windows)]
fn replace_file(source: &std::path::Path, destination: &std::path::Path) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing_name: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::AppState;

    #[test]
    fn settings_file_can_be_saved_and_loaded_again() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::load(temp.path().to_path_buf()).unwrap();
        {
            let mut data = state.data.lock().unwrap();
            data.settings.fps_limit = 45;
        }
        state.save().unwrap();
        {
            let mut data = state.data.lock().unwrap();
            data.settings.fps_limit = 60;
        }
        state.save().unwrap();
        let loaded = AppState::load(temp.path().to_path_buf()).unwrap();
        let data = loaded.data.lock().unwrap();
        assert_eq!(data.settings.fps_limit, 60);
        assert!(loaded.data_file.is_file());
        assert!(!loaded.data_file.with_extension("json.tmp").exists());
    }
    #[test]
    fn corrupt_settings_are_backed_up_and_reported() {
        let temp = tempfile::tempdir().unwrap();
        let data_file = temp.path().join("openwallpaper.json");
        std::fs::write(&data_file, "{ this is not valid json").unwrap();

        let loaded = AppState::load(temp.path().to_path_buf()).unwrap();

        assert_eq!(std::fs::read_to_string(&data_file).unwrap(), "{ this is not valid json");
        let backups: Vec<_> = std::fs::read_dir(temp.path()).unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.file_name().unwrap().to_string_lossy().starts_with("openwallpaper.corrupt-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(!loaded.startup_errors.lock().unwrap().is_empty());
        assert_eq!(loaded.data.lock().unwrap().schema_version, 1);
    }
}
