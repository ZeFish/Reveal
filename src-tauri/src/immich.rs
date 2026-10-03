//! Immich integration: server verification, asset uploads, remote browsing, and thumbnail/source caching.

use std::io::Read;
use std::path::PathBuf;
use std::time::SystemTime;
use tauri::Manager;

pub const PREFIX: &str = "immich://";

pub fn is_asset(path: &str) -> bool {
    path.starts_with(PREFIX)
}

pub fn parse_asset(path: &str) -> Result<(String, String), String> {
    let rest = path.strip_prefix(PREFIX).ok_or("Not an Immich asset")?;
    let (id, filename) = rest.split_once('/').ok_or("Invalid Immich asset path")?;
    if id.is_empty() || filename.is_empty() {
        return Err("Invalid Immich asset path components".to_string());
    }
    Ok((id.to_string(), filename.to_string()))
}

pub fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if let Some(stripped) = trimmed.strip_suffix("/api") {
        stripped.trim_end_matches('/').to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn metadata_path(app: &tauri::AppHandle, path: &str) -> Result<PathBuf, String> {
    let (id, _) = parse_asset(path)?;
    let edits = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("immich/edits")
        .join(id);
    Ok(edits.join("photo"))
}

pub fn thumbnail(app: &tauri::AppHandle, path: &str, size: u32) -> Result<Vec<u8>, String> {
    let (id, _) = parse_asset(path)?;
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("immich/thumbnails");
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_file = cache_dir.join(format!("{id}_{size}.jpg"));

    if let Ok(bytes) = std::fs::read(&cache_file) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    let prefs = crate::load_preferences(app.clone());
    let url = prefs
        .get("immich_url")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let key = prefs
        .get("immich_api_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if url.is_empty() || key.is_empty() {
        return Err("Immich server URL or API key is not configured".to_string());
    }
    let base = normalize_url(url);
    let immich_size = if size > 768 { "preview" } else { "thumbnail" };

    let response = ureq::get(&format!("{base}/api/assets/{id}/thumbnail?size={immich_size}"))
        .set("x-api-key", key.trim())
        .set("Accept", "image/*")
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("Immich thumbnail request failed: {e}"))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;

    if !bytes.is_empty() {
        let _ = std::fs::write(&cache_file, &bytes);
    }
    Ok(bytes)
}

pub fn source(app: &tauri::AppHandle, path: &str) -> Result<PathBuf, String> {
    let (id, filename) = parse_asset(path)?;
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("immich/originals")
        .join(&id);
    let _ = std::fs::create_dir_all(&cache_dir);
    let target = cache_dir.join(&filename);

    if target.is_file() {
        if let Ok(meta) = std::fs::metadata(&target) {
            if meta.len() > 0 {
                return Ok(target);
            }
        }
    }

    let prefs = crate::load_preferences(app.clone());
    let url = prefs
        .get("immich_url")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let key = prefs
        .get("immich_api_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if url.is_empty() || key.is_empty() {
        return Err("Immich server URL or API key is not configured".to_string());
    }
    let base = normalize_url(url);

    let response = ureq::get(&format!("{base}/api/assets/{id}/original"))
        .set("x-api-key", key.trim())
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| format!("Immich original download failed: {e}"))?;

    let temp = cache_dir.join(format!("{filename}.tmp"));
    let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
    std::io::copy(&mut response.into_reader(), &mut file).map_err(|e| e.to_string())?;
    std::fs::rename(temp, &target).map_err(|e| e.to_string())?;

    Ok(target)
}

pub fn test_connection(url: &str, api_key: &str) -> Result<serde_json::Value, String> {
    let base = normalize_url(url);
    if base.is_empty() {
        return Err("Immich server URL is empty".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("Immich API key is empty".to_string());
    }

    let user_resp = ureq::get(&format!("{base}/api/users/me"))
        .set("x-api-key", api_key.trim())
        .set("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(401 | 403, _) => {
                "Authentication failed: invalid or expired Immich API key".to_string()
            }
            ureq::Error::Status(code, _) => format!("Immich returned HTTP {code}"),
            other => format!("Failed to reach Immich server: {other}"),
        })?;

    let user_json: serde_json::Value = user_resp
        .into_json()
        .map_err(|e| format!("Failed to parse user info: {e}"))?;

    let version = ureq::get(&format!("{base}/api/server/version"))
        .set("x-api-key", api_key.trim())
        .set("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .ok()
        .and_then(|r| r.into_json::<serde_json::Value>().ok())
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else if let (Some(major), Some(minor), Some(patch)) = (
                v.get("major").and_then(|x| x.as_i64()),
                v.get("minor").and_then(|x| x.as_i64()),
                v.get("patch").and_then(|x| x.as_i64()),
            ) {
                Some(format!("v{major}.{minor}.{patch}"))
            } else {
                None
            }
        });

    Ok(serde_json::json!({
        "success": true,
        "user": {
            "id": user_json.get("id").and_then(|v| v.as_str()).unwrap_or(""),
            "email": user_json.get("email").and_then(|v| v.as_str()).unwrap_or(""),
            "name": user_json.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        },
        "version": version,
    }))
}

pub fn upload_photo(
    url: &str,
    api_key: &str,
    jpeg_bytes: &[u8],
    filename: &str,
    capture_at: Option<i64>,
) -> Result<serde_json::Value, String> {
    let base = normalize_url(url);
    if base.is_empty() {
        return Err("Immich server URL is not configured".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("Immich API key is not configured".to_string());
    }

    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let boundary = format!("----RevealImmich{stamp}");

    let created_at_iso = capture_at
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
        .unwrap_or_else(chrono::Utc::now)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let mut body = Vec::with_capacity(jpeg_bytes.len() + 1024);

    let add_field = |buf: &mut Vec<u8>, name: &str, val: &str| {
        buf.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{val}\r\n")
                .as_bytes(),
        );
    };

    add_field(&mut body, "deviceId", "Reveal");
    add_field(&mut body, "deviceAssetId", filename);
    add_field(&mut body, "fileCreatedAt", &created_at_iso);
    add_field(&mut body, "fileModifiedAt", &created_at_iso);
    add_field(&mut body, "isFavorite", "false");

    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"assetData\"; filename=\"{filename}\"\r\nContent-Type: image/jpeg\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(jpeg_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let resp = ureq::post(&format!("{base}/api/assets"))
        .set("x-api-key", api_key.trim())
        .set("Accept", "application/json")
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .timeout(std::time::Duration::from_secs(60))
        .send_bytes(&body)
        .map_err(|e| match e {
            ureq::Error::Status(401 | 403, _) => "Immich auth error: invalid API key".to_string(),
            ureq::Error::Status(code, resp) => {
                let err_text = resp.into_string().unwrap_or_default();
                format!("Immich upload failed (HTTP {code}): {err_text}")
            }
            other => format!("Immich upload network error: {other}"),
        })?;

    let res_json: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Invalid JSON response from Immich: {e}"))?;

    Ok(res_json)
}

#[tauri::command]
pub async fn immich_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let prefs = crate::load_preferences(app);
        let url = prefs
            .get("immich_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let key = prefs
            .get("immich_api_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if url.is_empty() || key.is_empty() {
            return Ok(serde_json::json!({ "connected": false }));
        }
        test_connection(url, key)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn immich_albums(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let prefs = crate::load_preferences(app);
        let url = prefs
            .get("immich_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let key = prefs
            .get("immich_api_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if url.is_empty() || key.is_empty() {
            return Ok(serde_json::json!({
                "connected": false,
                "albums": [],
                "total": 0
            }));
        }
        let base = normalize_url(url);
        let resp = ureq::get(&format!("{base}/api/albums"))
            .set("x-api-key", key.trim())
            .set("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(10))
            .call()
            .map_err(|e| format!("Failed to fetch Immich albums: {e}"))?;

        let list: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
        let empty_vec = Vec::new();
        let raw_albums = list.as_array().unwrap_or(&empty_vec);

        let mut total = 0usize;
        let mut albums = Vec::new();
        for a in raw_albums {
            let id = a.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = a
                .get("albumName")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled Album");
            let count = a.get("assetCount").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            total += count;
            albums.push(serde_json::json!({
                "id": id,
                "title": name,
                "count": count,
                "kind": "album",
                "children": []
            }));
        }

        Ok(serde_json::json!({
            "connected": true,
            "albums": albums,
            "total": total
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn immich_list(
    app: tauri::AppHandle,
    album: Option<String>,
    offset: usize,
    descending: bool,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let prefs = crate::load_preferences(app.clone());
        let url = prefs
            .get("immich_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let key = prefs
            .get("immich_api_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if url.is_empty() || key.is_empty() {
            return Err("Immich is not configured in Settings".to_string());
        }
        let base = normalize_url(url);

        let page_size = 100usize;
        let page_num = (offset / page_size) + 1;

        let mut payload = serde_json::json!({
            "page": page_num,
            "size": page_size,
            "order": if descending { "desc" } else { "asc" }
        });
        if let Some(ref alb) = album {
            if !alb.is_empty() {
                payload["albumIds"] = serde_json::json!([alb]);
            }
        }

        let resp = ureq::post(&format!("{base}/api/search/metadata"))
            .set("x-api-key", key.trim())
            .set("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(15))
            .send_json(payload)
            .map_err(|e| format!("Immich search failed: {e}"))?;

        let res_json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;

        let assets_obj = res_json.get("assets").cloned().unwrap_or(serde_json::json!({}));
        let total = assets_obj.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let empty_vec = Vec::new();
        let items = assets_obj
            .get("items")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);

        let mut frames = Vec::with_capacity(items.len());
        for item in items {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let original_name = item
                .get("originalFileName")
                .and_then(|v| v.as_str())
                .unwrap_or("photo.jpg");
            let name = if original_name.contains('.') {
                original_name.to_string()
            } else {
                format!("{original_name}.jpg")
            };
            let path = format!("{PREFIX}{id}/{name}");

            let capture_at = item
                .get("fileCreatedAt")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp());

            let metadata = metadata_path(&app, &path)?;
            let sidecar = reveal_meta::read(&metadata).ok().flatten();
            let rating = sidecar.and_then(|s| s.rating).unwrap_or(0);

            frames.push(serde_json::json!({
                "path": path,
                "name": name,
                "capture_at": capture_at,
                "rating": rating,
                "previewVersion": 0
            }));
        }

        let next = offset + frames.len();
        Ok(serde_json::json!({
            "frames": frames,
            "total": total,
            "next": next
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}
