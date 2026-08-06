//! Garden publishing — the exact HTTP contract the Obsidian
//! `standard-garden` plugin speaks (verified against
//! `apps/obsidian-standard-garden/src/core/garden/index.js`):
//!
//!   GET  {api}/publish/attachment?hash=<sha256hex>&ext=<ext>  → {exists, url}
//!   POST {api}/publish/attachment   multipart field "file"    → {url}
//!   PUT  {api}/publish/{slug}       JSON {title, content, slug}
//!   auth: `x-api-key` header on everything
//!
//! Zero config: credentials come from either Reveal's own signed-in key
//! (`GardenClient::from_key`, verified via `GET /api/me`) or, absent that,
//! the SAME file the Obsidian plugin already stores,
//! `<vault>/.obsidian/plugins/garden/data.json`.
//!
//! Slug rule (MUST match the plugin, or its two-way sync would duplicate
//! the note): basename lowercased, non-alphanumeric runs → "-", trimmed.

use std::path::Path;

use sha2::Digest;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_API_URL: &str = "https://standard.garden/api";

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("config: {0}")]
    Config(String),
    #[error("Clé Garden invalide ou expirée — reconnecte-toi (Connect Garden dans la barre latérale)")]
    Auth,
    #[error("http: {0}")]
    Http(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Map a ureq error to a friendly PublishError. A 401/403 is nearly always a
/// stale/missing key, so it gets a clear, actionable message instead of a raw
/// status line leaking the request URL and hash.
fn http_error(context: &str, e: ureq::Error) -> PublishError {
    match e {
        ureq::Error::Status(401 | 403, _) => PublishError::Auth,
        ureq::Error::Status(code, _) => PublishError::Http(format!("{context}: HTTP {code}")),
        other => PublishError::Http(format!("{context}: {other}")),
    }
}

/// Sign-in identity for the sidebar's account row — the display fields from
/// `GET /api/me`, cached username/tier are all the app persists.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountInfo {
    pub username: String,
    pub id: String,
    pub tier: Option<String>,
    pub notes_count: i64,
    pub total_views: i64,
}

pub struct GardenClient {
    pub api_url: String,
    pub username: String,
    api_key: String,
}

impl GardenClient {
    /// Reveal's own signed-in key — no vault required at all.
    pub fn from_key(api_url: &str, api_key: &str, username: &str) -> Self {
        Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            username: username.to_string(),
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// `GET {api}/me` — the same call the Obsidian plugin uses to confirm a
    /// key is real and fetch the username for canonical URLs. Called once
    /// when a key is pasted into the sidebar's sign-in field.
    pub fn verify_key(api_url: &str, api_key: &str) -> Result<AccountInfo, PublishError> {
        let api_url = api_url.trim_end_matches('/');
        let json: serde_json::Value = ureq::get(&format!("{api_url}/me"))
            .set("x-api-key", api_key)
            .call()
            .map_err(|e| http_error("clé", e))?
            .into_json()
            .map_err(|e| PublishError::Http(format!("réponse /me invalide: {e}")))?;
        let username = json["username"]
            .as_str()
            .ok_or_else(|| PublishError::Http("réponse /me sans username".into()))?
            .to_string();
        let id = json["id"].as_str().unwrap_or_default().to_string();
        Ok(AccountInfo {
            username,
            id,
            tier: json["tier"].as_str().map(str::to_string),
            notes_count: json["notesCount"].as_i64().unwrap_or(0),
            total_views: json["totalViews"].as_i64().unwrap_or(0),
        })
    }

    /// Read credentials from the vault's garden plugin config.
    pub fn from_vault(vault: &Path) -> Result<Self, PublishError> {
        let path = vault.join(".obsidian/plugins/garden/data.json");
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            PublishError::Config(format!("garden data.json introuvable ({}): {e}", path.display()))
        })?;
        let data: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| PublishError::Config(format!("data.json invalide: {e}")))?;
        let field = |k: &str| {
            data.get(k)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .ok_or_else(|| PublishError::Config(format!("clé {k} absente de data.json")))
        };
        Ok(Self {
            api_url: field("apiUrl")?.trim_end_matches('/').to_string(),
            api_key: field("apiKey")?,
            username: field("apiUsername").unwrap_or_else(|_| "francis".into()),
        })
    }

    /// Content-addressed upload: returns the CDN url, uploading only when
    /// the server doesn't already hold these bytes.
    pub fn ensure_attachment(
        &self,
        bytes: &[u8],
        filename: &str,
    ) -> Result<String, PublishError> {
        let hash = hex(&sha2::Sha256::digest(bytes));
        let ext = filename.rsplit('.').next().unwrap_or("bin").to_lowercase();

        let check: serde_json::Value = ureq::get(&format!(
            "{}/publish/attachment?hash={hash}&ext={ext}",
            self.api_url
        ))
        .set("x-api-key", &self.api_key)
        .call()
        .map_err(|e| http_error("check attachment", e))?
        .into_json()
        .map_err(|e| PublishError::Http(format!("check attachment json: {e}")))?;

        if check["exists"].as_bool() == Some(true) {
            if let Some(url) = check["url"].as_str() {
                return Ok(self.absolute(url));
            }
        }

        // Hand-rolled multipart, single "file" field (the plugin's shape).
        let boundary = format!("----RevealBoundary{hash}");
        let mime = match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            _ => "application/octet-stream",
        };
        let mut body = Vec::with_capacity(bytes.len() + 512);
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());

        let resp: serde_json::Value = ureq::post(&format!("{}/publish/attachment", self.api_url))
            .set("x-api-key", &self.api_key)
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={boundary}"),
            )
            .send_bytes(&body)
            .map_err(|e| http_error("upload", e))?
            .into_json()
            .map_err(|e| PublishError::Http(format!("upload json: {e}")))?;

        resp["url"]
            .as_str()
            .map(|u| self.absolute(u))
            .ok_or_else(|| PublishError::Http("réponse upload sans url".into()))
    }

    pub fn put_note(&self, slug: &str, title: &str, content: &str) -> Result<String, PublishError> {
        let resp: serde_json::Value = ureq::put(&format!("{}/publish/{slug}", self.api_url))
            .set("x-api-key", &self.api_key)
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "title": title,
                "content": content,
                "slug": slug,
            }))
            .map_err(|e| http_error("PUT note", e))?
            .into_json()
            .map_err(|e| PublishError::Http(format!("note JSON invalid: {e}")))?;

        if let Some(url) = resp["url"].as_str() {
            Ok(self.absolute(url))
        } else {
            Ok(self.live_url(slug))
        }
    }

    /// The public page for a slug (standard.garden profile URL).
    pub fn live_url(&self, slug: &str) -> String {
        format!("https://standard.garden/@{}/{slug}", self.username)
    }

    fn absolute(&self, url: &str) -> String {
        if url.starts_with("http") {
            url.to_string()
        } else if let Some(rest) = url.strip_prefix('/') {
            // origin of api_url + absolute path
            let origin: String = self
                .api_url
                .splitn(4, '/')
                .take(3)
                .collect::<Vec<_>>()
                .join("/");
            format!("{origin}/{rest}")
        } else {
            format!("{}/{url}", self.api_url)
        }
    }
}

/// The plugin's slug rule, byte for byte.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_matches_plugin_rule() {
        assert_eq!(slugify("2026-07-16"), "2026-07-16");
        assert_eq!(slugify("Été à Montréal!"), "t-montr-al");
        assert_eq!(slugify("--Hello  World--"), "hello-world");
    }
}
