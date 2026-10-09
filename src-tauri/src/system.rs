//! System integration: notifications, external editors, Finder actions, and catalog notes.

use crate::blocking;

/// Show a native macOS notification. Fallback no-op on other platforms.
#[tauri::command]
pub async fn notify_user(title: String, body: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification {} with title {}",
            applescript_string(&body),
            applescript_string(&title)
        );
        std::process::Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Open a path in Finder.
#[tauri::command]
pub async fn open_path(path: String) -> Result<(), String> {
    blocking(move || {
        crate::apple_photos::require_file(&path)?;
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("/usr/bin/open")
                .arg(path)
                .status()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
}

/// Reveal a file in Finder with the file selected.
#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), String> {
    blocking(move || {
        crate::apple_photos::require_file(&path)?;
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("/usr/bin/open")
                .args(["-R", &path])
                .status()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
}

pub fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// IPC smoke test — proves invoke() reaches Rust.
#[tauri::command]
pub fn ping() -> String {
    format!(
        "pong — reveal_lib {} (decode {}, engine {})",
        env!("CARGO_PKG_VERSION"),
        reveal_decode::VERSION,
        reveal_engine::VERSION,
    )
}

/// Dev/test hook: `REVEAL_AUTODEV=<raw path>` makes the frontend develop
/// that file on mount — lets the whole webview→IPC→engine round trip be
/// verified headlessly from the app's log.
#[tauri::command]
pub fn autoload_path() -> Option<String> {
    if let Some(path) = std::env::var("REVEAL_AUTODEV").ok().filter(|s| !s.is_empty()) {
        return Some(path);
    }
    std::env::args()
        .skip(1)
        .find(|path| {
            matches!(
                std::path::Path::new(path).extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref(),
                Some("jpg" | "jpeg")
            )
        })
}

pub struct EditorInfo {
    pub name: &'static str,
    pub ids: &'static [&'static str],
}

pub const CANDIDATES: &[EditorInfo] = &[
    EditorInfo { name: "Photoshop", ids: &["com.adobe.Photoshop"] },
    EditorInfo { name: "Lightroom Classic", ids: &["com.adobe.LightroomClassicCC7", "com.adobe.LightroomClassic"] },
    EditorInfo { name: "Lightroom", ids: &["com.adobe.lightroomCC"] },
    EditorInfo { name: "Capture One", ids: &["com.captureone.captureone16", "com.captureone.captureone15", "com.phaseone.captureone", "com.captureone.captureone"] },
    EditorInfo { name: "Affinity Photo", ids: &["com.seriflabs.affinityphoto2", "com.seriflabs.affinityphoto", "com.canva.affinity"] },
    EditorInfo { name: "Pixelmator Pro", ids: &["com.pixelmatorteam.pixelmator.x"] },
    EditorInfo { name: "Luminar Neo", ids: &["com.skylum.luminarneo", "com.skylum.luminarAI", "com.skylum.luminar4"] },
    EditorInfo { name: "DxO PhotoLab", ids: &["com.dxo.PhotoLab8", "com.dxo.PhotoLab7", "com.dxo.PhotoLab6", "com.dxo.PhotoLab5", "com.dxo.PhotoLab4"] },
    EditorInfo { name: "ON1 Photo RAW", ids: &["com.ononesoftware.ON1PhotoRAW2025.premium", "com.ononesoftware.ON1PhotoRAW2024.premium", "com.ononesoftware.ON1PhotoRAW2023.premium"] },
    EditorInfo { name: "darktable", ids: &["org.darktable.darktable", "photos.darktable.darktable"] },
    EditorInfo { name: "RawTherapee", ids: &["com.rawtherapee.RawTherapee"] },
    EditorInfo { name: "GIMP", ids: &["org.gimp.gimp-2.10", "org.gimp.GIMP", "org.gimp.gimp"] },
];

#[tauri::command]
pub async fn list_external_editors() -> Vec<(String, String)> {
    let mut installed = Vec::new();
    for candidate in CANDIDATES {
        for id in candidate.ids {
            let output = std::process::Command::new("mdfind")
                .arg(format!("kMDItemCFBundleIdentifier == \"{}\"", id))
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout_str = String::from_utf8_lossy(&out.stdout);
                    if let Some(path) = stdout_str.lines().next() {
                        let path = path.trim().to_string();
                        if !path.is_empty() {
                            installed.push((candidate.name.to_string(), path));
                            break;
                        }
                    }
                }
            }
        }
    }
    installed
}

#[tauri::command]
pub async fn open_in_editor(file_path: String, app_path: String) -> Result<(), String> {
    blocking(move || {
        crate::apple_photos::require_file(&file_path)?;
        std::process::Command::new("open")
            .args(&["-a", &app_path, &file_path])
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn load_catalog_note(root: String) -> String {
    let fresh = || "---\ntype: reveal-catalog\ncreated: 2026-07-16\n---\n\n".to_string();
    blocking(move || {
        let path = std::path::Path::new(&root).join("reveal.md");
        Ok(std::fs::read_to_string(&path).unwrap_or_else(|_| fresh()))
    })
    .await
    .unwrap_or_else(|_| fresh())
}

#[tauri::command]
pub async fn save_catalog_note(root: String, content: String) -> Result<(), String> {
    blocking(move || {
        let path = std::path::Path::new(&root).join("reveal.md");
        std::fs::write(&path, content).map_err(|e| e.to_string())
    })
    .await
}

pub fn is_volume_mounted(path: &std::path::Path) -> bool {
    if let Ok(strip) = path.strip_prefix("/Volumes") {
        if let Some(vol_name) = strip.components().next() {
            let vol_path = std::path::Path::new("/Volumes").join(vol_name);
            if !vol_path.is_dir() {
                return false;
            }
        }
    }
    true
}

/// Minimal percent-decoding (enough for file paths from
/// `encodeURIComponent`).
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Protocol smoke test — proves the webview can load pixels straight from
/// Rust through `reveal://`. Real handlers (thumb/proxy/preview) land in M2+.
pub const PING_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 150">
  <rect width="240" height="150" fill="#1f1f1e"/>
  <rect x="10" y="10" width="220" height="130" fill="none" stroke="#d6202c" stroke-width="2"/>
  <text x="120" y="70" fill="#ebebeb" font-family="monospace" font-size="16" text-anchor="middle">reveal://ping</text>
  <text x="120" y="95" fill="#d6202c" font-family="monospace" font-size="12" text-anchor="middle">served from Rust</text>
</svg>"##;
