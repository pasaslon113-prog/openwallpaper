use std::{fs, path::PathBuf};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

pub fn discover_wallpaper_engine_roots() -> Vec<String> {
    let mut steam_roots = Vec::new();
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(path) = key.get_value::<String, _>("SteamPath") {
            steam_roots.push(PathBuf::from(path));
        }
    }
    if let Ok(program_files) = std::env::var("ProgramFiles(x86)") {
        steam_roots.push(PathBuf::from(program_files).join("Steam"));
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        steam_roots.push(PathBuf::from(program_files).join("Steam"));
    }

    let mut libraries = Vec::new();
    for steam in &steam_roots {
        libraries.push(steam.clone());
        let vdf = steam.join("steamapps").join("libraryfolders.vdf");
        if let Ok(content) = fs::read_to_string(vdf) {
            for line in content.lines() {
                if !line.contains("\"path\"") { continue; }
                let quoted: Vec<&str> = line.split('"').collect();
                if let Some(raw) = quoted.get(3) {
                    let path = PathBuf::from(raw.replace("\\\\", "\\"));
                    if path.exists() { libraries.push(path); }
                }
            }
        }
    }

    let mut roots = Vec::new();
    for library in libraries {
        let workshop = library.join("steamapps/workshop/content/431960");
        let projects = library.join("steamapps/common/Wallpaper Engine/projects");
        for path in [workshop, projects] {
            if path.is_dir() {
                let text = path.to_string_lossy().to_string();
                if !roots.iter().any(|r: &String| r.eq_ignore_ascii_case(&text)) { roots.push(text); }
            }
        }
    }
    roots
}
