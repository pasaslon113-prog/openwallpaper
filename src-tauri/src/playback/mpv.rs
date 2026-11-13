use crate::model::RuntimeWallpaper;
use libloading::Library;
use std::{ffi::{c_char, c_int, c_void, CString}, path::{Path, PathBuf}};

#[cfg(windows)]
type MpvCreate = unsafe extern "C" fn() -> *mut c_void;
#[cfg(windows)]
type MpvSetOptionString = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
#[cfg(windows)]
type MpvInitialize = unsafe extern "C" fn(*mut c_void) -> c_int;
#[cfg(windows)]
type MpvCommand = unsafe extern "C" fn(*mut c_void, *const *const c_char) -> c_int;
#[cfg(windows)]
type MpvDestroy = unsafe extern "C" fn(*mut c_void);

#[cfg(windows)]
pub struct MpvPlayer {
    _library: Library,
    handle: *mut c_void,
    command: MpvCommand,
    destroy: MpvDestroy,
}

#[cfg(windows)]
unsafe impl Send for MpvPlayer {}

#[cfg(windows)]
impl MpvPlayer {
    pub fn start(hwnd: isize, wallpaper: &RuntimeWallpaper, app_data: &Path, mpv_runtime_dir: &Path) -> anyhow::Result<Self> {
        // Prepare fallible filesystem/string work before allocating the native mpv handle.
        let source = if wallpaper.kind == crate::model::WallpaperKind::Slideshow {
            create_slideshow_playlist(&wallpaper.path, app_data, wallpaper.slideshow_seconds)?
        } else {
            PathBuf::from(&wallpaper.path)
        };
        let source_text = source.to_string_lossy().to_string();
        let args = [CString::new("loadfile")?, CString::new(source_text)?, CString::new("replace")?];
        let pointers: Vec<*const c_char> = args.iter().map(|value| value.as_ptr())
            .chain(std::iter::once(std::ptr::null())).collect();

        let library = load_mpv_library(mpv_runtime_dir)?;
        unsafe {
            let create: MpvCreate = *library.get(b"mpv_create\0")?;
            let set_option: MpvSetOptionString = *library.get(b"mpv_set_option_string\0")?;
            let initialize: MpvInitialize = *library.get(b"mpv_initialize\0")?;
            let command: MpvCommand = *library.get(b"mpv_command\0")?;
            let destroy: MpvDestroy = *library.get(b"mpv_destroy\0")?;

            let handle = create();
            if handle.is_null() { anyhow::bail!("libmpv не смог создать экземпляр"); }

            // Keep option CStrings alive through initialization. Do not use mpv's
            // low-quality `fast` profile: it trades visual quality for speed.
            let mut options: Vec<(CString, CString)> = vec![
                (CString::new("wid")?, CString::new(hwnd.to_string())?),
                (CString::new("hwdec")?, CString::new("auto-safe")?),
                (CString::new("vo")?, CString::new("gpu")?),
                (CString::new("gpu-api")?, CString::new("d3d11")?),
                (CString::new("osd-level")?, CString::new("0")?),
                (CString::new("terminal")?, CString::new("no")?),
                (CString::new("config")?, CString::new("no")?),
                (CString::new("keep-open")?, CString::new("yes")?),
                (CString::new("loop-playlist")?, CString::new("inf")?),
                (CString::new("loop-file")?, CString::new(if wallpaper.kind == crate::model::WallpaperKind::Slideshow { "no" } else { "inf" })?),
                (CString::new("image-display-duration")?, CString::new(wallpaper.slideshow_seconds.to_string())?),
                (CString::new("volume")?, CString::new(wallpaper.volume.to_string())?),
                (CString::new("mute")?, CString::new(if wallpaper.muted { "yes" } else { "no" })?),
                (CString::new("video-sync")?, CString::new("display-resample")?),
                (CString::new("framedrop")?, CString::new("vo")?),
            ];
            // The FPS filter is useful for animated/video sources, but needlessly
            // running it for a still image adds work without changing what is shown.
            if matches!(wallpaper.kind, crate::model::WallpaperKind::Video | crate::model::WallpaperKind::Gif) {
                options.push((CString::new("vf")?, CString::new(format!("fps=fps={}", wallpaper.fps_limit.clamp(10, 120)))?));
            }
            for (name, value) in &options {
                let result = set_option(handle, name.as_ptr(), value.as_ptr());
                if result < 0 {
                    destroy(handle);
                    anyhow::bail!("libmpv отклонил параметр {} (код {result})", name.to_string_lossy());
                }
            }

            let initialized = initialize(handle);
            if initialized < 0 {
                destroy(handle);
                anyhow::bail!("libmpv: ошибка инициализации (код {initialized}). Проверьте mpv-2.dll и видеодрайвер.");
            }
            let loaded = command(handle, pointers.as_ptr());
            if loaded < 0 {
                destroy(handle);
                anyhow::bail!("libmpv не смог открыть выбранный файл (код {loaded})");
            }
            Ok(Self { _library: library, handle, command, destroy })
        }
    }

    pub fn set_paused(&mut self, paused: bool) -> anyhow::Result<()> {
        self.run_command(&["set", "pause", if paused { "yes" } else { "no" }])
    }

    pub fn set_volume(&mut self, volume: u8, muted: bool) -> anyhow::Result<()> {
        let volume_text = volume.to_string();
        self.run_command(&["set", "volume", volume_text.as_str()])?;
        self.run_command(&["set", "mute", if muted { "yes" } else { "no" }])
    }

    fn run_command(&mut self, args: &[&str]) -> anyhow::Result<()> {
        let strings: Vec<CString> = args.iter().map(|s| CString::new(*s)).collect::<Result<_, _>>()?;
        let pointers: Vec<*const c_char> = strings.iter().map(|s| s.as_ptr()).chain(std::iter::once(std::ptr::null())).collect();
        let result = unsafe { (self.command)(self.handle, pointers.as_ptr()) };
        if result < 0 { anyhow::bail!("Команда libmpv завершилась с кодом {result}"); }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for MpvPlayer {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.handle); }
    }
}

#[cfg(not(windows))]
pub struct MpvPlayer;

#[cfg(not(windows))]
impl MpvPlayer {
    pub fn start(_hwnd: isize, _wallpaper: &RuntimeWallpaper, _app_data: &Path, _mpv_runtime_dir: &Path) -> anyhow::Result<Self> {
        anyhow::bail!("Воспроизведение libmpv реализовано для Windows 10/11");
    }
    pub fn set_paused(&mut self, _paused: bool) -> anyhow::Result<()> { Ok(()) }
    pub fn set_volume(&mut self, _volume: u8, _muted: bool) -> anyhow::Result<()> { Ok(()) }
}

#[cfg(windows)]
fn load_mpv_library(mpv_runtime_dir: &Path) -> anyhow::Result<Library> {
    let mut candidates = Vec::new();
    if let Ok(path) = std::env::var("OPENWALLPAPER_MPV") { candidates.push(PathBuf::from(path)); }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() { candidates.push(parent.join("mpv-2.dll")); }
    }
    candidates.push(mpv_runtime_dir.join("mpv-2.dll"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("resources").join("mpv-runtime").join("mpv-2.dll"));
        }
    }
    candidates.push(PathBuf::from("mpv-2.dll"));
    let mut errors = Vec::new();
    for candidate in candidates {
        match unsafe { Library::new(&candidate) } {
            Ok(lib) => return Ok(lib),
            Err(error) => errors.push(format!("{}: {}", candidate.display(), error)),
        }
    }
    anyhow::bail!("Не найдена встроенная библиотека mpv-2.dll. Переустановите OpenWallpaper или проверьте целостность папки resources/mpv-runtime. {}", errors.join("; "))
}


#[cfg(windows)]
fn create_slideshow_playlist(folder: &str, app_data: &Path, seconds: u32) -> anyhow::Result<PathBuf> {
    let directory = PathBuf::from(folder);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && crate::library::classify_path(p) == Some(crate::model::WallpaperKind::Image))
        .collect();
    files.sort();
    if files.is_empty() { anyhow::bail!("В папке слайд-шоу не найдены изображения"); }
    std::fs::create_dir_all(app_data)?;
    let playlist = app_data.join(format!("slideshow-{}.m3u", crate::library::stable_id(&directory)));
    let mut content = String::from("#EXTM3U\n");
    for file in files {
        let display_name = file.file_name().unwrap_or_default().to_string_lossy();
        let full_path = file.to_string_lossy();
        if display_name.contains('\r') || display_name.contains('\n') || full_path.contains('\r') || full_path.contains('\n') {
            continue;
        }
        content.push_str(&format!("#EXTINF:{},{}\n{}\n", seconds.clamp(2, 3600), display_name, full_path));
    }
    if content.lines().count() <= 1 { anyhow::bail!("В слайд-шоу нет пригодных имён файлов"); }
    let temporary = playlist.with_extension("m3u.tmp");
    std::fs::write(&temporary, content)?;
    std::fs::rename(&temporary, &playlist).or_else(|error| {
        if playlist.exists() { std::fs::remove_file(&playlist)?; }
        std::fs::rename(&temporary, &playlist).map_err(|_| error)
    })?;
    Ok(playlist)
}
