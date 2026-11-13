use crate::model::{PersistedData, SceneManifest, WallpaperItem, WallpaperKind};
use image::{ImageFormat, ImageReader};
use std::{collections::{HashMap, HashSet}, fs, io::BufWriter, path::{Path, PathBuf}, time::UNIX_EPOCH};
use walkdir::WalkDir;

const VIDEO_EXT: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "divx",
    "mpg", "mpeg", "mpe", "m1v", "m2v", "mpv",
    "ts", "m2t", "m2ts", "mts", "m2p", "vob",
    "wmv", "asf", "flv", "f4v", "ogv", "ogg",
    "3gp", "3g2", "mxf", "dv", "rm", "rmvb",
    "nut", "y4m", "wtv", "dvr-ms", "nsv", "amv",
];
const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp"];

pub fn stable_id(path: &Path) -> String {
    let normalized = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let value = normalized.to_string_lossy().to_lowercase();
    blake3::hash(value.as_bytes()).to_hex()[..20].to_string()
}

pub fn classify_path(path: &Path) -> Option<WallpaperKind> {
    if path.is_dir() {
        if path.join("scene.json").is_file() { return Some(WallpaperKind::Scene); }
        if path.join("index.html").is_file() { return Some(WallpaperKind::Web); }
        return None;
    }
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    match ext.as_str() {
        "gif" => Some(WallpaperKind::Gif),
        "html" | "htm" => Some(WallpaperKind::Web),
        _ if VIDEO_EXT.contains(&ext.as_str()) => Some(WallpaperKind::Video),
        _ if IMAGE_EXT.contains(&ext.as_str()) => Some(WallpaperKind::Image),
        _ => None,
    }
}

pub fn add_roots(data: &mut PersistedData, roots: &[String]) -> anyhow::Result<usize> {
    let mut added = 0;
    for root in roots {
        let path = PathBuf::from(root);
        if !path.is_dir() { continue; }
        let canonical = path.canonicalize()?.to_string_lossy().to_string();
        if !data.library_roots.iter().any(|r| r.eq_ignore_ascii_case(&canonical)) {
            data.library_roots.push(canonical);
            added += 1;
        }
    }
    Ok(added)
}

pub fn scan_library(data: &mut PersistedData, thumbnail_cache: &Path) -> anyhow::Result<usize> {
    let favorites: HashSet<String> = data.wallpapers.iter().filter(|w| w.favorite).map(|w| w.id.clone()).collect();
    let mut found: HashMap<String, WallpaperItem> = data.wallpapers.iter()
        .filter(|item| item.kind == WallpaperKind::Web && (item.path.starts_with("https://") || item.path.starts_with("http://")))
        .map(|item| (item.id.clone(), item.clone()))
        .collect();
    for root in &data.library_roots {
        let root_path = PathBuf::from(root);
        if !root_path.is_dir() { continue; }
        let source = source_label(&root_path);
        for entry in WalkDir::new(&root_path).follow_links(false).max_depth(9).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !entry.file_type().is_file() { continue; }
            let lower_name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            if lower_name == "scene.json" {
                let parent = path.parent().unwrap_or(path);
                if let Some(mut item) = scene_item(parent, &source) {
                    item.favorite = favorites.contains(&item.id);
                    found.insert(item.id.clone(), item);
                }
                continue;
            }
            if lower_name == "index.html" {
                let parent = path.parent().unwrap_or(path);
                if let Some(mut item) = make_item(parent, WallpaperKind::Web, &source, thumbnail_cache) {
                    item.favorite = favorites.contains(&item.id);
                    found.insert(item.id.clone(), item);
                }
                continue;
            }
            let Some(kind) = classify_path(path) else { continue; };
            if kind == WallpaperKind::Web && !matches!(lower_name.as_str(), "index.html" | "wallpaper.html") {
                continue;
            }
            if let Some(mut item) = make_item(path, kind, &source, thumbnail_cache) {
                item.favorite = favorites.contains(&item.id);
                found.insert(item.id.clone(), item);
            }
        }
    }

    // Each folder with at least two still images is also offered as a slideshow.
    let mut groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for item in found.values().filter(|w| w.kind == WallpaperKind::Image) {
        let p = PathBuf::from(&item.path);
        if let Some(parent) = p.parent() { groups.entry(parent.to_path_buf()).or_default().push(p); }
    }
    for (folder, mut files) in groups {
        if files.len() < 2 { continue; }
        files.sort();
        let source = source_label(&folder);
        let id = format!("slideshow-{}", stable_id(&folder));
        let item = WallpaperItem {
            id: id.clone(),
            title: folder.file_name().unwrap_or_default().to_string_lossy().to_string(),
            kind: WallpaperKind::Slideshow,
            path: folder.to_string_lossy().to_string(),
            preview_data_uri: None,
            preview_path: make_preview(&files[0], thumbnail_cache),
            source,
            favorite: favorites.contains(&id),
            tags: vec!["slideshow".into()],
            description: Some(format!("{} изображений", files.len())),
            file_count: files.len(),
        };
        found.insert(id, item);
    }

    let mut items: Vec<WallpaperItem> = found.into_values().collect();
    items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    data.wallpapers = items;
    Ok(data.wallpapers.len())
}

fn make_item(path: &Path, kind: WallpaperKind, source: &str, thumbnail_cache: &Path) -> Option<WallpaperItem> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let name = if canonical.is_dir() {
        canonical.file_name()?.to_string_lossy().to_string()
    } else {
        canonical.file_stem()?.to_string_lossy().to_string()
    };
    let id = stable_id(&canonical);
    let preview_path = if kind == WallpaperKind::Image || kind == WallpaperKind::Gif {
        make_preview(&canonical, thumbnail_cache)
    } else {
        None
    };
    Some(WallpaperItem {
        id,
        title: name,
        kind,
        path: canonical.to_string_lossy().to_string(),
        preview_data_uri: None,
        preview_path,
        source: source.to_string(),
        favorite: false,
        tags: Vec::new(),
        description: None,
        file_count: 1,
    })
}

fn scene_item(folder: &Path, source: &str) -> Option<WallpaperItem> {
    let manifest_path = folder.join("scene.json");
    let raw = fs::read_to_string(manifest_path).ok()?;
    let manifest: SceneManifest = serde_json::from_str(&raw).ok()?;
    if manifest.version != 1 || manifest.layers.is_empty() { return None; }
    let id = format!("scene-{}", stable_id(folder));
    Some(WallpaperItem {
        id,
        title: manifest.name,
        kind: WallpaperKind::Scene,
        path: folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf()).to_string_lossy().to_string(),
        preview_data_uri: None,
        preview_path: None,
        source: source.to_string(),
        favorite: false,
        tags: vec!["scene".into()],
        description: manifest.description,
        file_count: manifest.layers.len(),
    })
}

fn make_preview(path: &Path, cache_dir: &Path) -> Option<String> {
    // Cache keys change when a file is replaced or edited, without reading the full file.
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    let identity = format!("{}:{}:{}", path.canonicalize().ok()?.to_string_lossy(), metadata.len(), modified);
    let key = blake3::hash(identity.as_bytes()).to_hex().to_string();
    let cached = cache_dir.join(format!("{key}.png"));
    if cached.is_file() {
        return Some(cached.to_string_lossy().to_string());
    }

    fs::create_dir_all(cache_dir).ok()?;
    let reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let decoded = reader.decode().ok()?;
    let thumbnail = decoded.thumbnail(480, 270);
    let nonce = std::time::SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_nanos();
    let temporary = cache_dir.join(format!("{key}.{}.{}.tmp.png", std::process::id(), nonce));
    {
        let file = fs::File::create(&temporary).ok()?;
        let mut writer = BufWriter::new(file);
        thumbnail.write_to(&mut writer, ImageFormat::Png).ok()?;
        use std::io::Write;
        writer.flush().ok()?;
    }
    if fs::rename(&temporary, &cached).is_err() {
        // Another scan may have produced the same thumbnail concurrently.
        let _ = fs::remove_file(&temporary);
        if !cached.is_file() { return None; }
    }
    Some(cached.to_string_lossy().to_string())
}

fn source_label(path: &Path) -> String {
    let text = path.to_string_lossy().to_lowercase();
    if text.contains("workshop") && text.contains("431960") { "Steam Workshop".into() }
    else if text.contains("wallpaper engine") || text.contains("wallpaper_engine") { "Wallpaper Engine".into() }
    else if text.contains("openwallpaper originals") { "OpenWallpaper".into() }
    else { "Импортировано".into() }
}

pub fn safe_scene_layer_path(scene_dir: &Path, source: &str) -> anyhow::Result<PathBuf> {
    let candidate = scene_dir.join(source);
    let base = scene_dir.canonicalize()?;
    let resolved = candidate.canonicalize()?;
    if !resolved.starts_with(&base) {
        anyhow::bail!("Scene layer escapes the scene directory");
    }
    if !resolved.is_file() { anyhow::bail!("Scene layer is not a file"); }
    Ok(resolved)
}

pub fn list_slideshow_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && classify_path(&path) == Some(WallpaperKind::Image) { files.push(path); }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn recognizes_supported_formats() {
        assert_eq!(classify_path(Path::new("clip.mp4")), Some(WallpaperKind::Video));
        assert_eq!(classify_path(Path::new("clip.webm")), Some(WallpaperKind::Video));
        assert_eq!(classify_path(Path::new("concert.m2ts")), Some(WallpaperKind::Video));
        assert_eq!(classify_path(Path::new("old-movie.wmv")), Some(WallpaperKind::Video));
        assert_eq!(classify_path(Path::new("camera.3gp")), Some(WallpaperKind::Video));
        assert_eq!(classify_path(Path::new("archive.rmvb")), Some(WallpaperKind::Video));
        assert_eq!(classify_path(Path::new("broadcast.mxf")), Some(WallpaperKind::Video));
        assert_eq!(classify_path(Path::new("anim.gif")), Some(WallpaperKind::Gif));
        assert_eq!(classify_path(Path::new("still.webp")), Some(WallpaperKind::Image));
        assert_eq!(classify_path(Path::new("text.txt")), None);
    }

    #[test]
    fn scene_path_cannot_escape_scene_directory() {
        let temp = tempfile::tempdir().unwrap();
        let scene = temp.path().join("scene");
        fs::create_dir_all(&scene).unwrap();
        image::RgbImage::new(2, 2).save(scene.join("ok.png")).unwrap();
        assert!(safe_scene_layer_path(&scene, "ok.png").is_ok());
        assert!(safe_scene_layer_path(&scene, "../secret.txt").is_err());
    }

    #[test]
    fn image_folder_is_also_offered_as_a_slideshow() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        fs::create_dir_all(&root).unwrap();
        image::RgbImage::new(8, 8).save(root.join("one.png")).unwrap();
        image::RgbImage::new(8, 8).save(root.join("two.jpg")).unwrap();
        let mut data = PersistedData::default();
        data.library_roots.push(temp.path().to_string_lossy().to_string());
        let cache = temp.path().join("cache");
        scan_library(&mut data, &cache).unwrap();
        assert!(data.wallpapers.iter().any(|w| w.kind == WallpaperKind::Slideshow && w.file_count == 2));
    }

    #[test]
    fn manually_added_remote_web_wallpapers_survive_library_rescans() {
        let temp = tempfile::tempdir().unwrap();
        let mut data = PersistedData::default();
        data.wallpapers.push(WallpaperItem {
            id: "remote-wallpaper".into(),
            title: "Remote page".into(),
            kind: WallpaperKind::Web,
            path: "https://example.com/wallpaper".into(),
            preview_data_uri: None,
            preview_path: None,
            source: "Веб-обои".into(),
            favorite: true,
            tags: vec!["web".into()],
            description: None,
            file_count: 1,
        });
        scan_library(&mut data, &temp.path().join("cache")).unwrap();
        let remote = data.wallpapers.iter().find(|item| item.id == "remote-wallpaper").unwrap();
        assert!(remote.favorite);
        assert_eq!(remote.path, "https://example.com/wallpaper");
    }

    #[test]
    fn thumbnails_are_cached_on_disk_and_not_embedded_as_base64() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("wallpapers");
        let cache = temp.path().join("cache");
        fs::create_dir_all(&root).unwrap();
        image::RgbImage::new(32, 18).save(root.join("image.png")).unwrap();
        let mut data = PersistedData::default();
        data.library_roots.push(root.to_string_lossy().to_string());
        scan_library(&mut data, &cache).unwrap();
        let item = data.wallpapers.iter().find(|item| item.kind == WallpaperKind::Image).unwrap();
        let preview_path = item.preview_path.as_ref().unwrap().clone();
        assert!(Path::new(&preview_path).is_file());
        assert!(item.preview_data_uri.is_none());
        let first_modified = fs::metadata(&preview_path).unwrap().modified().unwrap();
        scan_library(&mut data, &cache).unwrap();
        let item = data.wallpapers.iter().find(|item| item.kind == WallpaperKind::Image).unwrap();
        assert_eq!(item.preview_path.as_deref(), Some(preview_path.as_str()));
        assert_eq!(fs::metadata(&preview_path).unwrap().modified().unwrap(), first_modified);
    }

    #[test]
    fn stable_id_is_repeatable() {
        let p = Path::new("C:/test/wallpaper.mp4");
        assert_eq!(stable_id(p), stable_id(p));
        assert_eq!(stable_id(p).len(), 20);
    }
}

