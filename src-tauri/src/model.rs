use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum WallpaperKind {
    #[serde(alias = "video")] Video,
    #[serde(alias = "gif")] Gif,
    #[serde(alias = "image")] Image,
    #[serde(alias = "web")] Web,
    #[serde(alias = "slideshow")] Slideshow,
    #[serde(alias = "scene")] Scene,
}

impl WallpaperKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Video => "Видео",
            Self::Gif => "GIF",
            Self::Image => "Изображение",
            Self::Web => "Веб",
            Self::Slideshow => "Слайд-шоу",
            Self::Scene => "Сцена",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperItem {
    pub id: String,
    pub title: String,
    pub kind: WallpaperKind,
    pub path: String,
    #[serde(default)]
    pub preview_data_uri: Option<String>,
    #[serde(default)]
    pub preview_path: Option<String>,
    pub source: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub autostart: bool,
    pub start_minimized: bool,
    pub pause_in_games: bool,
    pub pause_on_lock: bool,
    pub pause_on_battery: bool,
    pub fps_limit: u32,
    pub volume: u8,
    pub muted: bool,
    pub slideshow_seconds: u32,
    pub game_processes: Vec<String>,
    pub monitor_wallpapers: HashMap<String, String>,
    pub monitor_fps: HashMap<String, u32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            autostart: false,
            start_minimized: false,
            pause_in_games: true,
            pause_on_lock: true,
            pause_on_battery: false,
            fps_limit: 30,
            volume: 0,
            muted: true,
            slideshow_seconds: 8,
            game_processes: vec![
                "cs2.exe".into(),
                "valorant.exe".into(),
                "gta5.exe".into(),
                "eldenring.exe".into(),
                "fortniteclient-win64-shipping.exe".into(),
            ],
            monitor_wallpapers: HashMap::new(),
            monitor_fps: HashMap::new(),
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.fps_limit = self.fps_limit.clamp(10, 120);
        self.slideshow_seconds = self.slideshow_seconds.clamp(2, 3600);
        for fps in self.monitor_fps.values_mut() {
            *fps = (*fps).clamp(10, 120);
        }
        self.game_processes = self.game_processes.into_iter()
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub wallpaper_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistedData {
    pub library_roots: Vec<String>,
    pub wallpapers: Vec<WallpaperItem>,
    pub collections: Vec<Collection>,
    pub settings: AppSettings,
    pub schema_version: u32,
}

impl Default for PersistedData {
    fn default() -> Self {
        Self {
            library_roots: Vec::new(),
            wallpapers: Vec::new(),
            collections: Vec::new(),
            settings: AppSettings::default(),
            schema_version: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneManifest {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub layers: Vec<SceneLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneLayer {
    #[serde(rename = "type")]
    pub layer_type: String,
    pub source: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "default_full")]
    pub width: f32,
    #[serde(default = "default_full")]
    pub height: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub fit: Option<String>,
    #[serde(default)]
    pub muted: bool,
}

fn default_full() -> f32 { 100.0 }
fn default_opacity() -> f32 { 1.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopItem {
    pub published_file_id: String,
    pub title: String,
    pub preview_url: Option<String>,
    pub item_url: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeWallpaper {
    pub id: String,
    pub title: String,
    pub kind: WallpaperKind,
    pub path: String,
    pub url: Option<String>,
    pub scene: Option<SceneManifest>,
    pub fps_limit: u32,
    pub paused: bool,
    pub volume: u8,
    pub muted: bool,
    pub slideshow_seconds: u32,
}


#[cfg(test)]
mod tests {
    use super::WallpaperKind;

    #[test]
    fn wallpaper_kind_serializes_for_frontend_and_reads_legacy_values() {
        assert_eq!(serde_json::to_string(&WallpaperKind::Video).unwrap(), "\"Video\"");
        assert_eq!(serde_json::from_str::<WallpaperKind>("\"video\"").unwrap(), WallpaperKind::Video);
        assert_eq!(serde_json::from_str::<WallpaperKind>("\"Web\"").unwrap(), WallpaperKind::Web);
    }

    #[test]
    fn settings_are_clamped_to_supported_runtime_limits() {
        let mut settings = super::AppSettings::default();
        settings.fps_limit = 500;
        settings.slideshow_seconds = 0;
        settings.monitor_fps.insert("monitor-a".into(), 1);
        settings.game_processes = vec!["  GAME.EXE  ".into(), " ".into()];
        let normalized = settings.normalized();
        assert_eq!(normalized.fps_limit, 120);
        assert_eq!(normalized.slideshow_seconds, 2);
        assert_eq!(normalized.monitor_fps.get("monitor-a"), Some(&10));
        assert_eq!(normalized.game_processes, vec!["game.exe".to_string()]);
    }
}

