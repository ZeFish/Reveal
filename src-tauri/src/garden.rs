//! Garden cloud connection and authentication.
//!
//! Handles Garden API keys, device IDs, browser connect nonces, and account info.

use tauri::{AppHandle, Emitter, Manager, State};
use crate::shell_prefs::{prefs_file, read_shell_prefs, set_or_remove};

/// The sidebar account row's state — mirrors Swift's `GardenAccountModel`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GardenAccountInfo {
    pub signed_in: bool,
    pub username: Option<String>,
    pub tier: Option<String>,
    pub notes_count: i64,
    pub total_views: i64,
}

impl GardenAccountInfo {
    pub fn signed_out() -> Self {
        Self { signed_in: false, username: None, tier: None, notes_count: 0, total_views: 0 }
    }
}

/// The signed-in key alone — kept out of `ShellPrefs` so it never rides on
/// the `shell-prefs-changed` broadcast the frontend listens to.
pub fn read_garden_api_key(app: &AppHandle) -> Option<String> {
    if let Ok(path) = prefs_file(app) {
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(key) = value.get("garden_api_key").and_then(|v| v.as_str()) {
                    if !key.is_empty() {
                        return Some(key.to_string());
                    }
                }
            }
        }
    }
    // Fallback: check vault's obsidian garden plugin config
    if let Ok(client) = reveal_publish::GardenClient::from_vault(&crate::publishing::vault_path(app)) {
        return Some(client.api_key().to_string());
    }
    None
}

pub fn write_garden_prefs(
    app: &AppHandle,
    api_key: Option<&str>,
    username: Option<&str>,
    tier: Option<&str>,
) -> Result<(), String> {
    let path = prefs_file(app)?;
    let mut value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !value.is_object() {
        value = serde_json::json!({});
    }
    set_or_remove(&mut value, "garden_api_key", api_key);
    set_or_remove(&mut value, "garden_username", username);
    set_or_remove(&mut value, "garden_tier", tier);
    let raw = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())
}

/// Own signed-in key first (parity with Swift's `loadConfig()`), falling
/// back to the vault's Obsidian `garden` plugin config so publishing still
/// works before anyone signs in via the sidebar.
pub fn garden_client(app: &AppHandle) -> Result<reveal_publish::GardenClient, String> {
    if let Some(key) = read_garden_api_key(app) {
        let username = read_shell_prefs(app).garden_username.unwrap_or_else(|| "francis".into());
        return Ok(reveal_publish::GardenClient::from_key(
            reveal_publish::DEFAULT_API_URL,
            &key,
            &username,
        ));
    }
    reveal_publish::GardenClient::from_vault(&crate::publishing::vault_path(app)).map_err(|e| e.to_string())
}

/// Verify a key and store it as Reveal's own signed-in credential, emitting
/// `garden-account-changed` for the sidebar. Shared by the paste-key command
/// and the browser deep-link callback — same outcome, two ways in.
pub async fn sign_in_with_key(app: &AppHandle, key: &str) -> Result<GardenAccountInfo, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("empty key".to_string());
    }
    let info = {
        let key = key.clone();
        tauri::async_runtime::spawn_blocking(move || {
            reveal_publish::GardenClient::verify_key(reveal_publish::DEFAULT_API_URL, &key)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?
    };
    write_garden_prefs(app, Some(&key), Some(&info.username), info.tier.as_deref())?;
    let out = GardenAccountInfo {
        signed_in: true,
        username: Some(info.username),
        tier: info.tier,
        notes_count: info.notes_count,
        total_views: info.total_views,
    };
    let _ = app.emit("garden-account-changed", &out);
    Ok(out)
}

/// Verify a pasted API key and store it as Reveal's own signed-in
/// credential — the sidebar's sign-in popover (Swift `GardenAccountModel.signIn`).
#[tauri::command]
pub async fn garden_sign_in(app: AppHandle, api_key: String) -> Result<GardenAccountInfo, String> {
    sign_in_with_key(&app, &api_key).await
}

/// A browser sign-in started from THIS app: the one-time `state` the callback
/// must echo back, and when it stops being valid.
///
/// `reveal://garden-callback?key=…` can be opened by any web page or document.
/// Without this, a link could replace the signed-in Garden account with someone
/// else's, and Reveal would then publish your photos into THEIR garden. So a
/// callback counts only if we started the sign-in ourselves, it is recent, and
/// it carries our nonce.
#[derive(Default)]
pub struct GardenConnectState(pub std::sync::Mutex<Option<(String, std::time::Instant)>>);

/// How long a started browser sign-in stays valid.
const GARDEN_CONNECT_WINDOW: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// `bytes` random bytes from the system, hex-encoded. Reveal only runs on
/// macOS, where /dev/urandom is the OS's own CSPRNG.
pub fn random_hex(bytes: usize) -> Result<String, String> {
    use std::io::Read;
    let mut buf = vec![0u8; bytes];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .map_err(|e| format!("no randomness: {e}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// An id for this Mac, kept in the app's own data folder, so each device that
/// connects to Garden gets its own key (connecting another never signs this
/// one out). Created on first use.
pub fn garden_device_id(app: &AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("garden-device-id");
    if let Ok(existing) = std::fs::read_to_string(&file) {
        let existing = existing.trim();
        if (6..=32).contains(&existing.len())
            && existing.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Ok(existing.to_string());
        }
    }
    let id = random_hex(8)?;
    std::fs::write(&file, &id).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Start a browser sign-in: remember a fresh nonce and hand back the page to
/// open. The sidebar's "Connect via browser" opens exactly this URL.
#[tauri::command]
pub fn garden_begin_connect(
    app: AppHandle,
    pending: State<'_, GardenConnectState>,
) -> Result<String, String> {
    let nonce = random_hex(16)?;
    let device = garden_device_id(&app)?;
    *pending.0.lock().map_err(|e| e.to_string())? = Some((nonce.clone(), std::time::Instant::now()));
    Ok(format!("https://standard.garden/connect/reveal?state={nonce}&device={device}"))
}

/// Whether a deep-link callback echoes the nonce of a sign-in we started and
/// that is still fresh. The nonce is single-use: a match consumes it.
pub fn take_connect_nonce(app: &AppHandle, state: Option<&str>) -> bool {
    let pending = app.state::<GardenConnectState>();
    let Ok(mut slot) = pending.0.lock() else { return false };
    match (slot.take(), state) {
        (Some((nonce, started)), Some(given)) => {
            started.elapsed() < GARDEN_CONNECT_WINDOW && constant_time_eq(nonce.as_bytes(), given.as_bytes())
        }
        _ => false,
    }
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Sidebar "Sign out" — clears the stored key entirely (falls back to any
/// vault config, same as before anyone signed in).
#[tauri::command]
pub fn garden_sign_out(app: AppHandle) -> Result<GardenAccountInfo, String> {
    write_garden_prefs(&app, None, None, None)?;
    let out = GardenAccountInfo::signed_out();
    let _ = app.emit("garden-account-changed", &out);
    Ok(out)
}

/// Silent re-verify of a stored key on launch (Swift `refreshIfNeeded`) — a
/// network hiccup keeps the cached username/tier showing rather than
/// surfacing an error the instant the app opens.
#[tauri::command]
pub async fn garden_refresh(app: AppHandle) -> GardenAccountInfo {
    let Some(key) = read_garden_api_key(&app) else {
        return GardenAccountInfo::signed_out();
    };
    let cached = read_shell_prefs(&app);
    let result = {
        let key = key.clone();
        tauri::async_runtime::spawn_blocking(move || {
            reveal_publish::GardenClient::verify_key(reveal_publish::DEFAULT_API_URL, &key)
        })
        .await
    };
    match result {
        Ok(Ok(info)) => {
            let _ = write_garden_prefs(&app, Some(&key), Some(&info.username), info.tier.as_deref());
            GardenAccountInfo {
                signed_in: true,
                username: Some(info.username),
                tier: info.tier,
                notes_count: info.notes_count,
                total_views: info.total_views,
            }
        }
        _ => {
            let fallback_user = cached.garden_username.or_else(|| {
                reveal_publish::GardenClient::from_vault(&crate::publishing::vault_path(&app)).ok().map(|c| c.username.clone())
            });
            GardenAccountInfo {
                signed_in: fallback_user.is_some(),
                username: fallback_user,
                tier: cached.garden_tier,
                notes_count: 0,
                total_views: 0,
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_nonce_comparison_is_exact() {
        assert!(constant_time_eq(b"abc123", b"abc123"));
        assert!(!constant_time_eq(b"abc123", b"abc124"));
        assert!(!constant_time_eq(b"abc123", b"abc12"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn random_hex_is_the_right_length_and_not_constant() {
        let a = random_hex(16).unwrap();
        let b = random_hex(16).unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
