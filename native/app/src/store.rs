//! Where the app keeps tunes, datalogs and the wheel mapping: %APPDATA%\HondaSim on Windows,
//! ~/.honda-sim elsewhere.

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("HondaSim"))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".honda-sim")))
        .unwrap_or_else(|| PathBuf::from("HondaSim data"));
    let _ = std::fs::create_dir_all(&base);
    base
}

pub fn sub(name: &str) -> PathBuf {
    let d = data_dir().join(name);
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn load_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub fn save_json<T: serde::Serialize>(path: &PathBuf, v: &T) -> Result<(), String> {
    std::fs::write(path, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", path.display()))
}

/// Saved tunes, newest first.
pub fn list_tunes() -> Vec<(String, PathBuf)> {
    let mut v: Vec<(std::time::SystemTime, String, PathBuf)> = std::fs::read_dir(sub("tunes")).into_iter().flatten().flatten()
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path().file_stem().unwrap().to_string_lossy().to_string(), e.path()))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().map(|(_, n, p)| (n, p)).collect()
}

pub fn safe_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let n = n.trim().to_string();
    if n.is_empty() { "tune".into() } else { n }
}

/// Open a folder in the file manager.
pub fn open_folder(p: &PathBuf) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}
