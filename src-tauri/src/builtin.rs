use crate::model::WallpaperItem;
use std::{
    fs,
    path::{Path, PathBuf},
};

const WALLPAPERS: &[(&str, &str)] = &[
    ("Aurora Drift", include_str!("../../public/wallpapers/aurora-drift/index.html")),
    ("Orbital Garden", include_str!("../../public/wallpapers/orbital-garden/index.html")),
    ("Neon Rain", include_str!("../../public/wallpapers/neon-rain/index.html")),
];

pub fn install(app_data_dir: &Path) -> anyhow::Result<PathBuf> {
    let root = app_data_dir.join("wallpapers").join("OpenWallpaper Originals");
    for (name, html) in WALLPAPERS {
        let folder = root.join(name);
        fs::create_dir_all(&folder)?;
        let entrypoint = folder.join("index.html");
        if !entrypoint.exists() {
            fs::write(entrypoint, html)?;
        }
    }
    Ok(root)
}

pub fn default_wallpaper_id(items: &[WallpaperItem]) -> Option<String> {
    items.iter()
        .find(|item| item.source == "OpenWallpaper" && item.title == "Aurora Drift")
        .map(|item| item.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{library, model::PersistedData};

    #[test]
    fn installs_standalone_wallpapers_and_library_scanner_finds_them() {
        let temp = tempfile::tempdir().unwrap();
        let root = install(temp.path()).unwrap();
        let mut data = PersistedData::default();
        data.library_roots.push(root.to_string_lossy().into_owned());

        library::scan_library(&mut data, &temp.path().join("thumbnails")).unwrap();

        assert_eq!(data.wallpapers.len(), WALLPAPERS.len());
        assert!(data.wallpapers.iter().all(|item| item.kind == crate::model::WallpaperKind::Web));
        assert!(data.wallpapers.iter().all(|item| item.source == "OpenWallpaper"));
    }

    #[test]
    fn keeps_user_changes_to_an_existing_builtin_wallpaper() {
        let temp = tempfile::tempdir().unwrap();
        let root = install(temp.path()).unwrap();
        let custom = root.join("Aurora Drift").join("index.html");
        fs::write(&custom, "user-edited wallpaper").unwrap();

        install(temp.path()).unwrap();

        assert_eq!(fs::read_to_string(custom).unwrap(), "user-edited wallpaper");
    }

    #[test]
    fn first_run_wallpaper_is_an_openwallpaper_original() {
        let temp = tempfile::tempdir().unwrap();
        let root = install(temp.path()).unwrap();
        let mut data = PersistedData::default();
        data.library_roots.push(root.to_string_lossy().into_owned());
        library::scan_library(&mut data, &temp.path().join("thumbnails")).unwrap();

        let selected = default_wallpaper_id(&data.wallpapers).unwrap();
        let wallpaper = data.wallpapers.iter().find(|item| item.id == selected).unwrap();
        assert_eq!(wallpaper.title, "Aurora Drift");
        assert_eq!(wallpaper.source, "OpenWallpaper");
    }
}

