//! Stories and publishing: the Obsidian vault side, story notes and their theme, and pushing to Garden.
//!
//! Lifted out of `lib.rs` unchanged — see that file's header.

use crate::*;

/// Where a developed JPEG should land inside the Obsidian vault so notes can
/// reference it — the vault's own `attachmentFolderPath` (read from
/// `.obsidian/app.json`), resolved against the vault root. Falls back to the
/// vault root when unset or set to note-relative (`./`), and creates it. This
/// is a filesystem export INTO the vault, distinct from Garden publishing.
#[tauri::command]
pub(crate) fn vault_attachment_dir(app: tauri::AppHandle) -> Result<String, String> {
    let vault = vault_path(&app);
    if !vault.exists() {
        return Err(format!(
            "Obsidian vault path does not exist: '{}'. Please configure a valid vault in Settings.",
            vault.display()
        ));
    }
    let rel = std::fs::read_to_string(vault.join(".obsidian/app.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| {
            v.get("attachmentFolderPath")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    // "" or "/" → vault root; "./" or "./x" is note-relative and has no single
    // path, so we also fall back to the root there.
    let dir = if rel.is_empty() || rel == "/" || rel.starts_with("./") {
        vault
    } else {
        vault.join(rel.trim_start_matches('/'))
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create attachment directory '{}': {e}", dir.display()))?;
    Ok(dir.to_string_lossy().into_owned())
}

/// The vault path (for Garden credentials) — pref with a sane default.
pub(crate) fn vault_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    let prefs = app
        .path()
        .app_data_dir()
        .map(|d| d.join("prefs.json"))
        .ok();
    if let Some(p) = prefs {
        if let Ok(raw) = std::fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(vault) = v
                    .get("vault")
                    .and_then(|x| x.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    return std::path::PathBuf::from(vault);
                }
            }
        }
    }
    dirs_home(Some(app)).join("Documents/Atelier")
}

fn dirs_home(app: Option<&tauri::AppHandle>) -> std::path::PathBuf {
    if let Some(app) = app {
        if let Ok(home) = app.path().home_dir() {
            return home;
        }
    }
    std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default()
}

/// Photo stems currently marked into the folder's story note.
#[tauri::command]
pub(crate) async fn story_stems(dir: String) -> Vec<String> {
    story::stems(std::path::Path::new(&dir))
}

/// Which of these folders carry a story note with photos — the sidebar's
/// red "cette journée a une histoire" dots (Swift: `folderHasStory`).
#[tauri::command]
pub(crate) async fn story_dirs(dirs: Vec<String>) -> Vec<String> {
    dirs.into_iter()
        .filter(|d| !story::stems(std::path::Path::new(d)).is_empty())
        .collect()
}

/// Toggle a photo in the folder's story. Returns the new stems.
#[tauri::command]
pub(crate) async fn story_toggle(dir: String, path: String) -> Result<Vec<String>, String> {
    story::toggle(std::path::Path::new(&dir), std::path::Path::new(&path))
        .map_err(|e| e.to_string())
}

/// Load the raw markdown story note.
#[tauri::command]
pub(crate) async fn load_story_note(dir: String) -> String {
    let dirp = std::path::Path::new(&dir);
    let path = story::note_path(dirp);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| "---\npublish: true\n---\n\n".to_string())
}

/// Save the story note's body, preserving the on-disk frontmatter verbatim.
/// Read-modify-write from disk (invariant 3): the composer sends serialized
/// frontmatter + body, but only its BODY is trusted — the disk note's
/// frontmatter is kept as-is so a body save can never clobber a concurrent
/// theme/pin write that landed between the composer's load and its save.
#[tauri::command]
pub(crate) async fn save_story_note(dir: String, content: String) -> Result<(), String> {
    let dirp = std::path::Path::new(&dir);
    // Parse the incoming content to extract just its body (the composer's blocks).
    let incoming = story::StoryNote::parse(&content, &story::note_path(dirp));
    // Load the current on-disk note and replace only its body.
    let mut note = story::StoryNote::load(dirp);
    note.body = incoming.body;
    note.save().map_err(|e| e.to_string())
}

/// Read the story note's theme tokens (None = unspecified, inherits default).
#[tauri::command]
pub(crate) async fn story_load_theme(dir: String) -> story::ThemeTokens {
    story::StoryNote::load(std::path::Path::new(&dir)).theme_tokens()
}

/// Write the theme tokens to the note's frontmatter, deriving light + fg.
/// Read-modify-write from disk — never from an in-memory copy (invariant 3).
#[tauri::command]
pub(crate) async fn story_set_theme(dir: String, tokens: story::ThemeTokens) -> Result<(), String> {
    let mut note = story::StoryNote::load(std::path::Path::new(&dir));
    note.set_theme(&tokens);
    note.save().map_err(|e| e.to_string())
}

/// Set or clear pin state on the story note. Read-modify-write from disk.
#[tauri::command]
pub(crate) async fn story_set_pinned(dir: String, pinned: bool, pinned_at: Option<String>) -> Result<(), String> {
    let mut note = story::StoryNote::load(std::path::Path::new(&dir));
    note.set_pinned(pinned, pinned_at.as_deref());
    note.save().map_err(|e| e.to_string())
}

/// Scan folders for story notes — feeds the ÉPINGLÉES + RÉCENTES lists.
#[tauri::command]
pub(crate) async fn list_story_notes(dirs: Vec<String>) -> Vec<story::StoryNoteInfo> {
    story::list_story_notes(&dirs)
}

fn parse_markdown_story(raw: &str, dirp: &std::path::Path) -> (String, String) {
    let mut title = dirp.file_name().unwrap_or_default().to_string_lossy().to_string();
    let body;
    let text = raw.replace("\r\n", "\n");
    if text.starts_with("---\n") {
        if let Some(end) = text[4..].find("\n---") {
            let fm_section = &text[4..4 + end];
            for line in fm_section.lines() {
                if let Some(rest) = line.strip_prefix("title:") {
                    title = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
            body = text[4 + end + 4..].to_string();
        } else {
            body = text;
        }
    } else {
        body = text;
    }
    (title, body)
}

fn markdown_to_html(body: &str, urls: &[(String, String)]) -> String {
    let mut html = String::new();
    let paragraphs = body.split("\n\n");
    for p in paragraphs {
        let p_trimmed = p.trim();
        if p_trimmed.is_empty() {
            continue;
        }
        if p_trimmed.starts_with("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", &p_trimmed[2..]));
        } else if p_trimmed.starts_with("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", &p_trimmed[3..]));
        } else if p_trimmed.starts_with("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", &p_trimmed[4..]));
        } else {
            let mut has_embeds = false;
            let mut embeds_html = String::new();
            let mut rest = p_trimmed;
            while let Some(start) = rest.find("![[") {
                has_embeds = true;
                rest = &rest[start + 3..];
                if let Some(end) = rest.find("]]") {
                    let inner = &rest[..end];
                    let link = inner.split('|').next().unwrap_or(inner).trim();
                    let stem = link.strip_suffix(".jpg").or_else(|| link.strip_suffix(".jpeg")).unwrap_or(link);
                    if let Some((_, url)) = urls.iter().find(|(s, _)| s == stem) {
                        embeds_html.push_str(&format!("<img src=\"{}\" alt=\"{}\" />\n", url, stem));
                    }
                    rest = &rest[end + 2..];
                } else {
                    break;
                }
            }
            if has_embeds {
                html.push_str(&format!("<div class=\"photo-row\">\n{}</div>\n", embeds_html));
            } else {
                html.push_str(&format!("<p>{}</p>\n", p_trimmed.replace("\n", "<br>")));
            }
        }
    }
    html
}

/// Export the story folder to a local directory with index.html and developed JPEGs.
#[tauri::command]
pub(crate) async fn export_local_story(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    dir: String,
    dest: String,
    long_edge: u32,
    border_frac: f32,
) -> Result<(), String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let dirp = std::path::Path::new(&dir);
        let destp = std::path::Path::new(&dest);
        let stems = story::stems(dirp);
        if stems.is_empty() {
            return Err("no photos in the story".to_string());
        }
        let images_dir = destp.join("images");
        std::fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;
        let total = stems.len();
        let emit = |done: usize, current: &str, phase: &str| {
            let _ = app.emit(
                "export-progress",
                serde_json::json!({ "done": done, "total": total, "current": current, "phase": phase }),
            );
        };
        let mut urls: Vec<(String, String)> = Vec::new();
        for (i, stem) in stems.iter().enumerate() {
            emit(i, stem, "developing");
            let raw = reveal_decode::RAW_EXTENSIONS
                .iter()
                .map(|e| dirp.join(format!("{stem}.{e}")))
                .chain(
                    reveal_decode::RAW_EXTENSIONS
                        .iter()
                        .map(|e| dirp.join(format!("{stem}.{}", e.to_uppercase()))),
                )
                .find(|p| p.exists())
                .ok_or_else(|| format!("RAW not found for {stem}"))?;
            let recipe = reveal_meta::read(&raw)
                .ok()
                .flatten()
                .and_then(|s| s.engine_settings)
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            let (jpeg, _, _) = engine
                .export_jpeg(&raw, &recipe, long_edge, border_frac)
                .map_err(|e| format!("develop {stem}: {e:#}"))?;
            let jpg_path = images_dir.join(format!("{stem}.jpg"));
            std::fs::write(&jpg_path, &jpeg).map_err(|e| e.to_string())?;
            urls.push((stem.clone(), format!("images/{stem}.jpg")));
        }
        let raw_md = std::fs::read_to_string(story::note_path(dirp)).map_err(|e| e.to_string())?;
        let (title, body) = parse_markdown_story(&raw_md, dirp);
        let body_html = markdown_to_html(&body, &urls);
        let html_content = format!(r#"<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <style>
    :root {{
      --bg: #15110d;
      --fg: #eae5de;
      --accent: #d6202c;
      --line: rgba(234, 229, 222, 0.1);
      --font-serif: "Newsreader", "Georgia", serif;
      --font-sans: "Inter", "Helvetica Neue", sans-serif;
    }}
    @media (prefers-color-scheme: light) {{
      :root {{
        --bg: #faf8f5;
        --fg: #1a1a1a;
        --line: rgba(0, 0, 0, 0.08);
      }}
    }}
    body {{
      background: var(--bg);
      color: var(--fg);
      font-family: var(--font-serif);
      margin: 0;
      padding: 3rem 1.5rem;
      line-height: 1.6;
      display: flex;
      flex-direction: column;
      align-items: center;
    }}
    main {{
      max-width: 50rem;
      width: 100%;
    }}
    header {{
      text-align: center;
      margin-bottom: 4rem;
    }}
    h1 {{
      font-family: var(--font-sans);
      font-weight: 300;
      font-size: 2.25rem;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      color: var(--accent);
      margin: 0 0 1rem 0;
    }}
    p {{
      font-size: 1.15rem;
      margin: 1.5rem 0;
    }}
    h2, h3, h4 {{
      font-family: var(--font-sans);
      font-weight: 600;
      letter-spacing: 0.02em;
      margin-top: 2.5rem;
    }}
    img {{
      display: block;
      max-width: 100%;
      height: auto;
      margin: 3rem auto;
      border-radius: 4px;
      box-shadow: 0 4px 20px rgba(0,0,0,0.15);
    }}
    .photo-row {{
      display: flex;
      justify-content: center;
      gap: 1.5rem;
      margin: 3rem 0;
      flex-wrap: wrap;
    }}
    .photo-row img {{
      margin: 0;
      flex: 1 1 200px;
      object-fit: contain;
      max-height: 500px;
    }}
    @media print {{
      body {{
        background: #fff !important;
        color: #000 !important;
        padding: 0;
      }}
      main {{
        max-width: 100%;
      }}
      img {{
        page-break-inside: avoid;
        box-shadow: none;
      }}
    }}
  </style>
</head>
<body>
  <main>
    <header>
      <h1>{title}</h1>
    </header>
    <article>
      {body_html}
    </article>
  </main>
</body>
</html>"#, title = title, body_html = body_html);
        std::fs::write(destp.join("index.html"), html_content).map_err(|e| e.to_string())?;
        emit(total, "", "note");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Publish the folder's story to the Garden: develop each marked photo
/// (2048 px JPEG beside the RAW), upload attachments (content-addressed
/// dedup), rewrite embeds, PUT the note. `dry_run` stops before any write
/// to the server (existence checks only). Emits `publish-progress`.
#[tauri::command]
pub(crate) async fn publish_story(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    dir: String,
    dry_run: bool,
) -> Result<String, String> {
    let engine = state.0.clone();
    let client = garden_client(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let dirp = std::path::Path::new(&dir);
        let stems = story::stems(dirp);
        if stems.is_empty() {
            return Err("no photos in the story".to_string());
        }
        let total = stems.len();
        let emit = |done: usize, current: &str, phase: &str| {
            let _ = app.emit(
                "publish-progress",
                serde_json::json!({ "done": done, "total": total, "current": current, "phase": phase }),
            );
        };

        // 1. resolve each marked photo to <stem>.jpg beside the RAW
        let mut urls: Vec<(String, String)> = Vec::new();
        for (i, stem) in stems.iter().enumerate() {
            emit(i, stem, "preparing");

            // Look for pre-developed preview sidecar first (<stem>.preview.jpg, <stem>.jpg, etc.)
            let candidates = [
                dirp.join(format!("{stem}.preview.jpg")),
                dirp.join(format!("{stem}.jpg")),
                dirp.join(format!("{stem}.JPG")),
                dirp.join(format!("{stem}.jpeg")),
            ];

            let mut existing_jpeg: Option<Vec<u8>> = candidates.iter().find_map(|p| {
                if p.exists() {
                    std::fs::read(p).ok().filter(|b| !b.is_empty())
                } else {
                    None
                }
            });

            let raw_opt = reveal_decode::RAW_EXTENSIONS
                .iter()
                .map(|e| dirp.join(format!("{stem}.{e}")))
                .chain(
                    reveal_decode::RAW_EXTENSIONS
                        .iter()
                        .map(|e| dirp.join(format!("{stem}.{}", e.to_uppercase()))),
                )
                .find(|p| p.exists());

            let recipe = raw_opt.as_ref().and_then(|raw| {
                reveal_meta::read(raw)
                    .ok()
                    .flatten()
                    .and_then(|s| s.engine_settings)
                    .and_then(|v| serde_json::from_value(v).ok())
            }).unwrap_or_default();

            if existing_jpeg.is_none() {
                if let Some(raw) = &raw_opt {
                    if let Ok(cache_p) = developed_preview_cache_path(&app, raw, 2048, served_preview_mtime(raw)) {
                        if cache_p.exists() {
                            existing_jpeg = std::fs::read(&cache_p).ok().filter(|b| !b.is_empty());
                        }
                    }
                }
            }

            let jpeg = match existing_jpeg {
                Some(bytes) => bytes,
                None => {
                    let raw = raw_opt.ok_or_else(|| format!("RAW not found for {stem}"))?;
                    emit(i, stem, "developing");
                    let (bytes, _, _) = engine
                        .export_jpeg(&raw, &recipe, 2048, 0.0)
                        .map_err(|e| format!("develop {stem}: {e:#}"))?;
                    bytes
                }
            };

            let jpg_path = dirp.join(format!("{stem}.jpg"));
            let should_write = match std::fs::read(&jpg_path) {
                Ok(existing) => existing != jpeg,
                Err(_) => true,
            };
            if should_write {
                let _ = std::fs::write(&jpg_path, &jpeg);
            }

            // 2. upload (dedup) — dry run stops at the existence check
            emit(i, stem, if dry_run { "verifying" } else { "uploading" });
            if dry_run {
                urls.push((stem.clone(), format!("dry-run://{stem}.jpg")));
            } else {
                let url = client
                    .ensure_attachment(&jpeg, &format!("{stem}.jpg"))
                    .map_err(|e| e.to_string())?;
                urls.push((stem.clone(), url));
            }
        }

        // 3. the note
        let name = dirp.file_name().unwrap_or_default().to_string_lossy().to_string();
        let slug = reveal_publish::slugify(&name);
        let content = story::content_for_publish(dirp, &urls).map_err(|e| e.to_string())?;
        emit(total, &name, "note");
        if dry_run {
            eprintln!("publish dry-run: slug={slug}, {} photos, note ok", urls.len());
            return Ok(format!("dry-run — slug {slug}, {} photos ready", urls.len()));
        }
        let live = client.put_note(&slug, &name, &content).map_err(|e| e.to_string())?;
        let _ = story::stamp_garden_url(dirp, &live);
        eprintln!("publié: {live}");
        Ok(live)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// PUBLIER, one frame — the Swift `publishSelectedPhotoToGarden`: develop
/// with the photo's own recipe, upload (content-addressed, dedup'd), PUT a
/// one-image note. Same credentials and slug rule as the story path.
#[tauri::command]
pub(crate) async fn publish_photo(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    path: String,
    allow_download: bool,
) -> Result<String, String> {
    let engine = state.0.clone();
    let client = garden_client(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let raw = std::path::PathBuf::from(&path);
        let stem = raw
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let emit = |phase: &str| {
            let _ = app.emit(
                "publish-progress",
                serde_json::json!({ "done": 0, "total": 1, "current": stem, "phase": phase }),
            );
        };

        emit("developing");
        let sidecar = reveal_meta::read(&apple_photos::metadata_path(&path)?).map_err(|e| e.to_string())?;
        let recipe = sidecar
            .as_ref()
            .and_then(|s| s.engine_settings.clone())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let (jpeg, _, _) = engine
            .export_jpeg(&apple_photos::source(&path)?, &recipe, 2048, 0.0)
            .map_err(|e| format!("develop {stem}: {e:#}"))?;

        emit("uploading");
        let url = client
            .ensure_attachment(&jpeg, &format!("{stem}.jpg"))
            .map_err(|e| e.to_string())?;

        emit("note");
        let slug = if apple_photos::is_asset(&path) {
            format!("{}-{:016x}", reveal_publish::slugify(&stem), developed_preview_source_key(&raw))
        } else {
            reveal_publish::slugify(&stem)
        };
        let caption = sidecar.and_then(|s| s.description).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();
        let content = format!(
            "---\npublish: true\ntype: gallery\ntheme: gallery\ndownload: {allow_download}\ntags:\n  - gallery\n  - photo\ncreated: {now}\nmodified: {now}\n---\n\n![]({url})\n\n{caption}\n"
        );
        client
            .put_note(&slug, &stem, &content)
            .map_err(|e| e.to_string())?;
        let live = client.live_url(&slug);
        eprintln!("publié: {live}");
        Ok(live)
    })
    .await
    .map_err(|e| e.to_string())?
}
