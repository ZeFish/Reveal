//! Google Photos integration: OAuth 2.0 authorization, token refresh, and media item export.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static CACHED_ACCESS_TOKEN: Mutex<Option<(String, Instant, u64)>> = Mutex::new(None);
static AUTH_CANCELLED: AtomicBool = AtomicBool::new(false);

fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

fn url_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(val);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            result.push(b' ');
            i += 1;
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

/// Retrieve a valid access token for Google Photos, using memory cache or refreshing via refresh_token.
pub fn get_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<String, String> {
    let client_id = client_id.trim();
    let client_secret = client_secret.trim();
    let refresh_token = refresh_token.trim();

    if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
        return Err("Google Photos credentials or refresh token missing".to_string());
    }

    if let Ok(guard) = CACHED_ACCESS_TOKEN.lock() {
        if let Some((token, instant, expires_in)) = guard.as_ref() {
            // Buffer by 60 seconds before expiration
            if instant.elapsed().as_secs() + 60 < *expires_in {
                return Ok(token.clone());
            }
        }
    }

    let resp = ureq::post("https://oauth2.googleapis.com/token")
        .timeout(Duration::from_secs(15))
        .send_form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .map_err(|e| format!("Failed to refresh Google Photos access token: {e}"))?;

    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse token response: {e}"))?;

    let access_token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Google did not return an access token".to_string())?
        .to_string();

    let expires_in = json
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .unwrap_or(3600);

    if let Ok(mut guard) = CACHED_ACCESS_TOKEN.lock() {
        *guard = Some((access_token.clone(), Instant::now(), expires_in));
    }

    Ok(access_token)
}

/// Uploads a JPEG image to Google Photos and creates the media item in the user's library.
pub fn upload_photo(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    jpeg_bytes: &[u8],
    filename: &str,
) -> Result<serde_json::Value, String> {
    let token = get_access_token(client_id, client_secret, refresh_token)?;

    // Step 1: Upload the byte stream to obtain an uploadToken
    let upload_token = ureq::post("https://photoslibrary.googleapis.com/v1/uploads")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/octet-stream")
        .set("X-Goog-Upload-Content-Type", "image/jpeg")
        .set("X-Goog-Upload-Protocol", "raw")
        .timeout(Duration::from_secs(60))
        .send_bytes(jpeg_bytes)
        .map_err(|e| format!("Google Photos upload bytes failed: {e}"))?
        .into_string()
        .map_err(|e| format!("Failed to read Google Photos upload token: {e}"))?;

    let upload_token = upload_token.trim();
    if upload_token.is_empty() {
        return Err("Google Photos returned an empty upload token".to_string());
    }

    // Step 2: Create media item in user's library
    let payload = serde_json::json!({
        "newMediaItems": [
            {
                "description": "Exported from Reveal",
                "simpleMediaItem": {
                    "fileName": filename,
                    "uploadToken": upload_token
                }
            }
        ]
    });

    let resp = ureq::post("https://photoslibrary.googleapis.com/v1/mediaItems:batchCreate")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(30))
        .send_json(payload)
        .map_err(|e| format!("Google Photos batchCreate failed: {e}"))?;

    let res_json: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse Google Photos batchCreate response: {e}"))?;

    let result_status = res_json
        .get("newMediaItemResults")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("status"));

    if let Some(st) = result_status {
        let code = st.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        let msg = st.get("message").and_then(|v| v.as_str()).unwrap_or("");
        if code != 0 && !msg.eq_ignore_ascii_case("success") {
            return Err(format!("Google Photos media item creation error: {msg} (code {code})"));
        }
    }

    Ok(res_json)
}

/// Tests authentication and reachability to Google Photos.
pub fn test_connection(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<serde_json::Value, String> {
    let token = get_access_token(client_id, client_secret, refresh_token)?;

    let resp = ureq::get("https://photoslibrary.googleapis.com/v1/albums?pageSize=1")
        .set("Authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Google Photos verification request failed: {e}"))?;

    if resp.status() == 200 {
        Ok(serde_json::json!({
            "success": true,
            "message": "Successfully connected to Google Photos"
        }))
    } else {
        Err(format!("Google Photos responded with HTTP status {}", resp.status()))
    }
}

/// Tauri command to start the OAuth 2.0 loopback flow.
#[tauri::command]
pub(crate) async fn google_photos_start_auth(
    app: tauri::AppHandle,
    client_id: String,
    client_secret: String,
) -> Result<serde_json::Value, String> {
    AUTH_CANCELLED.store(false, Ordering::Release);
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let client_id = client_id.trim();
        let client_secret = client_secret.trim();

        if client_id.is_empty() || client_secret.is_empty() {
            return Err("Google Client ID and Client Secret are required".to_string());
        }

        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("Failed to bind loopback listener: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();

        let redirect_uri = format!("http://127.0.0.1:{port}");

        let auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
            url_encode(client_id),
            url_encode(&redirect_uri),
            url_encode("https://www.googleapis.com/auth/photoslibrary.appendonly")
        );

        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("/usr/bin/open")
                .arg(&auth_url)
                .status();
        }

        // Wait for loopback request with 120s timeout
        let (tx, rx) = std::sync::mpsc::channel();
        let listener_clone = listener.try_clone().map_err(|e| e.to_string())?;

        std::thread::spawn(move || {
            let conn = listener_clone.accept();
            let _ = tx.send(conn);
        });

        let (mut stream, _) = rx
            .recv_timeout(Duration::from_secs(120))
            .map_err(|_| "Google Photos sign-in timed out. Please try again.".to_string())?
            .map_err(|e| format!("Failed to accept authentication connection: {e}"))?;

        let mut buffer = [0u8; 4096];
        let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        let request_str = String::from_utf8_lossy(&buffer[..n]);

        let request_line = request_str.lines().next().unwrap_or("");
        let query_start = request_line.find('?');
        let query_end = request_line.find(" HTTP/");

        let code = match (query_start, query_end) {
            (Some(s), Some(e)) if s < e => {
                let query = &request_line[s + 1..e];
                let mut found_code = None;
                for part in query.split('&') {
                    if let Some(val) = part.strip_prefix("code=") {
                        found_code = Some(url_decode(val));
                        break;
                    } else if let Some(err) = part.strip_prefix("error=") {
                        let err_msg = url_decode(err);
                        let html = format!(
                            r#"<!DOCTYPE html><html><body style="font-family:system-ui;background:#18181b;color:#f4f4f5;text-align:center;padding:50px;"><h2>Authorization cancelled</h2><p>Google returned error: {}</p></body></html>"#,
                            err_msg
                        );
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            html.len(),
                            html
                        );
                        let _ = stream.write_all(resp.as_bytes());
                        return Err(format!("Google authorization denied: {err_msg}"));
                    }
                }
                found_code
            }
            _ => None,
        };

        let code = match code {
            Some(c) => c,
            None => {
                return Err("Failed to extract authorization code from Google response".to_string());
            }
        };

        let html = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Reveal &mdash; Connected</title></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background-color: #121214; color: #f4f4f5; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; margin: 0;">
  <div style="background: #1c1c1f; border: 1px solid #2e2e32; border-radius: 12px; padding: 32px 40px; text-align: center; max-width: 440px; box-shadow: 0 8px 30px rgba(0,0,0,0.4);">
    <h2 style="margin: 0 0 12px; font-size: 20px; font-weight: 600;">Connected to Google Photos</h2>
    <p style="margin: 0; color: #a1a1aa; font-size: 14px; line-height: 1.5;">Reveal is now authorized to export photos to your Google Photos library. You can close this tab and return to Reveal.</p>
  </div>
</body>
</html>"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();

        // Exchange code for tokens
        let token_resp: serde_json::Value = ureq::post("https://oauth2.googleapis.com/token")
            .timeout(Duration::from_secs(15))
            .send_form(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("code", &code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", &redirect_uri),
            ])
            .map_err(|e| format!("Failed to exchange authorization code: {e}"))?
            .into_json()
            .map_err(|e| format!("Invalid token response: {e}"))?;

        let refresh_token = token_resp
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                "Google did not return a refresh token. Please re-authenticate and ensure access is approved.".to_string()
            })?
            .to_string();

        let access_token = token_resp
            .get("access_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let expires_in = token_resp
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        if !access_token.is_empty() {
            if let Ok(mut guard) = CACHED_ACCESS_TOKEN.lock() {
                *guard = Some((access_token, Instant::now(), expires_in));
            }
        }

        // Save credentials and refresh token to preferences
        let mut current_prefs = crate::load_preferences(app_handle.clone());
        if let Some(obj) = current_prefs.as_object_mut() {
            obj.insert(
                "google_photos_client_id".to_string(),
                serde_json::json!(client_id),
            );
            obj.insert(
                "google_photos_client_secret".to_string(),
                serde_json::json!(client_secret),
            );
            obj.insert(
                "google_photos_refresh_token".to_string(),
                serde_json::json!(&refresh_token),
            );
            let _ = crate::save_preferences(app_handle, current_prefs);
        }

        Ok(serde_json::json!({
            "success": true,
            "refresh_token": refresh_token
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Tauri command to test Google Photos connection from current preferences.
#[tauri::command]
pub(crate) async fn google_photos_test_connection(
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let prefs = crate::load_preferences(app);
        let client_id = prefs
            .get("google_photos_client_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let client_secret = prefs
            .get("google_photos_client_secret")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let refresh_token = prefs
            .get("google_photos_refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
            return Err("Google Photos is not connected in Settings".to_string());
        }

        test_connection(client_id, client_secret, refresh_token)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Tauri command to disconnect Google Photos.
#[tauri::command]
pub(crate) async fn google_photos_disconnect(
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Ok(mut guard) = CACHED_ACCESS_TOKEN.lock() {
            *guard = None;
        }

        let mut current_prefs = crate::load_preferences(app.clone());
        if let Some(obj) = current_prefs.as_object_mut() {
            obj.insert(
                "google_photos_refresh_token".to_string(),
                serde_json::json!(""),
            );
            obj.insert(
                "google_photos_export_enabled".to_string(),
                serde_json::json!(false),
            );
            let _ = crate::save_preferences(app, current_prefs);
        }

        Ok(serde_json::json!({ "success": true }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Tauri command to manually upload a single photo to Google Photos.
#[tauri::command]
pub(crate) async fn upload_photo_to_google_photos(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::EngineState>,
    path: String,
    long_edge: Option<u32>,
) -> Result<serde_json::Value, String> {
    let engine = state.0.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let prefs = crate::load_preferences(app_handle.clone());
        let client_id = prefs
            .get("google_photos_client_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let client_secret = prefs
            .get("google_photos_client_secret")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let refresh_token = prefs
            .get("google_photos_refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
            return Err("Google Photos is not configured in Settings".to_string());
        }

        let metadata = crate::apple_photos::metadata_path(&path)?;
        let source = crate::apple_photos::source(&path)?;
        let recipe = reveal_meta::read(&metadata)
            .map_err(|e| e.to_string())?
            .and_then(|s| s.engine_settings)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let edge = long_edge.unwrap_or(2048);
        let (jpeg, _, _) = engine
            .export_jpeg(&source, &recipe, edge, 0.0)
            .map_err(|e| format!("Develop failed: {e:#}"))?;

        let src = std::path::Path::new(&path);
        let stem = src.file_stem().unwrap_or_default().to_string_lossy();
        let filename = format!("{stem}.jpg");

        upload_photo(client_id, client_secret, refresh_token, &jpeg, &filename)
    })
    .await
    .map_err(|e| e.to_string())?
}
