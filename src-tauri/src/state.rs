use crate::{model::PersistedData, playback::MpvPlayer};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};

pub struct AppState {
    pub data: Mutex<PersistedData>,
    pub playback: Mutex<HashMap<String, MpvPlayer>>,
    pub data_file: PathBuf,
    pub app_data_dir: PathBuf,
    pub paused: Mutex<bool>,
    pub manual_paused: Mutex<bool>,
    pub library_scan_lock: Mutex<()>,
    save_lock: Mutex<()>,
}

impl AppState {
    pub fn load(app_data_dir: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&app_data_dir)?;
        let data_file = app_data_dir.join("openwallpaper.json");
        let data = if data_file.exists() {
            let raw = fs::read_to_string(&data_file)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            PersistedData::default()
        };
        Ok(Self {
            data: Mutex::new(data),
            playback: Mutex::new(HashMap::new()),
            data_file,
            app_data_dir,
            paused: Mutex::new(false),
            manual_paused: Mutex::new(false),
            library_scan_lock: Mutex::new(()),
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
}
