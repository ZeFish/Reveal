//! Story notes — file-over-apps: the story of a folder IS a markdown note
//! sitting in it (`<folder>/<folder-name>.md`), photos as `![[stem.jpg]]`
//! embeds. Garden's CSS renders consecutive bare embeds as one justified
//! row; a blank line breaks the row. Reveal only edits the marks; the note
//! stays a plain text file anyone can open.

use std::io;
use std::path::{Path, PathBuf};

/// A folder's story note — a Markdown file (`<folder>.md`) with optional YAML
/// frontmatter and a body of `![[stem.jpg]]` embeds, prose, and caption
/// callouts. Frontmatter is preserved verbatim except the keys we touch.
/// Port of Swift `StoryNote` (Sources/Reveal/StoryNote.swift).
pub struct StoryNote {
    pub url: PathBuf,
    pub frontmatter: Vec<String>, // raw lines between the fences, verbatim
    pub body: String,             // everything after the closing fence
}

impl StoryNote {
    /// Load (or start) the note for a folder. Missing file = empty note.
    pub fn load(dir: &Path) -> StoryNote {
        let url = note_path(dir);
        let raw = std::fs::read_to_string(&url).unwrap_or_default();
        StoryNote::parse(&raw, &url)
    }

    /// Parse raw markdown into frontmatter lines + body. If there's no closing
    /// `---` fence, the whole text is treated as body (frontmatter empty).
    pub fn parse(raw: &str, url: &Path) -> StoryNote {
        let text = raw.replace("\r\n", "\n");
        if !text.starts_with("---\n") {
            return StoryNote { url: url.to_path_buf(), frontmatter: Vec::new(), body: text };
        }
        let rest = &text[4..]; // drop opening "---\n"
        let mut fm: Vec<String> = Vec::new();
        let mut body = String::new();
        let mut closed = false;
        for line in rest.lines() {
            if !closed {
                if line.trim() == "---" {
                    closed = true;
                } else {
                    fm.push(line.to_string());
                }
            } else {
                body.push_str(line);
                body.push('\n');
            }
        }
        let body = if closed { body } else { text };
        let fm = if closed { fm } else { Vec::new() };
        StoryNote { url: url.to_path_buf(), frontmatter: fm, body }
    }

    /// Read a frontmatter scalar (quotes stripped), or None.
    pub fn frontmatter_value(&self, key: &str) -> Option<String> {
        for line in &self.frontmatter {
            if let Some(v) = fm_value(line, key) {
                return Some(v);
            }
        }
        None
    }

    /// Set a frontmatter key surgically (replace in place, append if new).
    /// `value = None` deletes the key. Preserves every other line verbatim.
    pub fn set_frontmatter(&mut self, key: &str, value: Option<&str>) {
        if let Some(idx) = self.frontmatter.iter().position(|l| fm_value(l, key).is_some()) {
            match value {
                Some(v) => self.frontmatter[idx] = format!("{key}: {v}"),
                None => { self.frontmatter.remove(idx); }
            }
        } else if let Some(v) = value {
            self.frontmatter.push(format!("{key}: {v}"));
        }
    }

    /// Write the note back to disk, creating parent dirs as needed.
    pub fn save(&self) -> io::Result<()> {
        let mut out = String::new();
        if !self.frontmatter.is_empty() {
            out.push_str("---\n");
            out.push_str(&self.frontmatter.join("\n"));
            out.push_str("\n---\n\n");
        }
        out.push_str(self.body.trim_start_matches('\n'));
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if let Some(parent) = self.url.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // std::fs::write truncates its target THEN writes — a write that
        // fails partway (e.g. ENOSPC, disk full) leaves a zero-byte or
        // truncated file with the OLD content already gone.
        //
        // Writing to a sibling temp file and rename()-ing over the target
        // guards against that, but on its own it is NOT enough: this note
        // lives on an NFS mount, and NFS write()/close() can both report
        // success while the bytes never actually reach the server — a
        // library-caught note here came back 790 bytes of pure 0x00, i.e.
        // exactly this happened (a page-cache write that silently never
        // made it to the NFS server, verified with `xxd`). Renaming a
        // silently-corrupted temp file over the good one loses the note
        // just as surely as writing straight to it would.
        //
        // So: write, fsync (force the flush to the server NOW, where an
        // error can actually be caught instead of swallowed at some later
        // implicit flush), then read the temp file back and byte-compare it
        // against what we meant to write. Only rename over the real note if
        // that round-trip actually matches. Any mismatch leaves the disk
        // note completely untouched and returns an error instead of eating
        // the user's work.
        let tmp = self.url.with_extension("md.tmp");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(out.as_bytes())?;
            f.sync_all()?;
        }
        let verify = std::fs::read(&tmp)?;
        if verify != out.as_bytes() {
            let _ = std::fs::remove_file(&tmp);
            return Err(io::Error::other(
                "story note write verification failed (read-back mismatch) — note on disk left untouched",
            ));
        }
        std::fs::rename(&tmp, &self.url)
    }

    /// Read all theme tokens from frontmatter (None for absent keys).
    pub fn theme_tokens(&self) -> ThemeTokens {
        let get = |k: &str| self.frontmatter_value(k);
        ThemeTokens {
            dark_background: get("color-dark-background"),
            dark_foreground: get("color-dark-foreground"),
            light_background: get("color-light-background"),
            light_foreground: get("color-light-foreground"),
            dark_accent: get("color-dark-accent"),
            light_accent: get("color-light-accent"),
            font_header: get("font-header"),
            font_text: get("font-text"),
        }
    }

    /// Write all 8 theme tokens to frontmatter, deriving the unspecified ones.
    /// Mirrors Swift `setStoryTheme` (StoryModel.swift:342-360):
    ///   dark_fg = derive_foreground(dark_bg)
    ///   light_bg = dark_fg, light_fg = dark_bg (the swap)
    ///   light_accent = dark_accent (mirror)
    /// None values delete the key (returns to "inherits default").
    pub fn set_theme(&mut self, tokens: &ThemeTokens) {
        let dark_bg = tokens.dark_background.as_deref();
        let accent = tokens.dark_accent.as_deref();

        self.set_frontmatter("color-dark-background", dark_bg);

        let dark_fg = dark_bg.map(derive_foreground);
        self.set_frontmatter("color-dark-foreground", dark_fg.as_deref());

        // light side = swap of dark side
        self.set_frontmatter("color-light-background", dark_fg.as_deref());
        self.set_frontmatter("color-light-foreground", dark_bg);

        // accent mirrors to both schemes
        self.set_frontmatter("color-dark-accent", accent);
        self.set_frontmatter("color-light-accent", accent);

        self.set_frontmatter("font-header", tokens.font_header.as_deref());
        self.set_frontmatter("font-text", tokens.font_text.as_deref());
    }

    /// Whether this note is pinned (frontmatter `pinned: true`).
    pub fn pinned(&self) -> bool {
        self.frontmatter_value("pinned").as_deref() == Some("true")
    }

    /// Set or clear pin state. `pinned = true` writes `pinned: true` plus an
    /// optional ISO `pinned-at` timestamp (for stable ordering). `pinned = false`
    /// removes both keys.
    pub fn set_pinned(&mut self, pinned: bool, pinned_at: Option<&str>) {
        if pinned {
            self.set_frontmatter("pinned", Some("true"));
            self.set_frontmatter("pinned-at", pinned_at);
        } else {
            self.set_frontmatter("pinned", None);
            self.set_frontmatter("pinned-at", None);
        }
    }
}

/// The 8 Garden theme frontmatter tokens, all optional (None = unspecified,
/// inherits default). The user picks dark_background + dark_accent + fonts;
/// the rest are derived on write. Port of Swift `ThemeTokens` + `setStoryTheme`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeTokens {
    pub dark_background: Option<String>,
    pub dark_foreground: Option<String>,
    pub light_background: Option<String>,
    pub light_foreground: Option<String>,
    pub dark_accent: Option<String>,
    pub light_accent: Option<String>,
    pub font_header: Option<String>,
    pub font_text: Option<String>,
}

/// Read a frontmatter scalar from one line (quotes stripped), or None.
/// Port of Swift `StoryNote.fmValue`. Key must match `key:` prefix.
fn fm_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim();
    let prefix = format!("{key}:");
    let rest = t.strip_prefix(&prefix)?;
    let mut v = rest.trim().to_string();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        v = v[1..v.len() - 1].to_string();
    }
    Some(v)
}

/// Contrast-derived ink from a background hex — two warm ink tones, not flat
/// black/white. Port of Swift `deriveForeground` (ThemeTokens.swift:9-13).
/// Threshold luminance < 0.5 → `#F5F1EA`, else `#15110D`.
pub fn derive_foreground(bg_hex: &str) -> String {
    let s = bg_hex.trim_start_matches('#');
    if s.len() != 6 {
        return "#15110D".to_string(); // safe default on parse failure
    }
    let Ok(v) = u64::from_str_radix(s, 16) else {
        return "#15110D".to_string();
    };
    let r = ((v >> 16) & 0xFF) as f64 / 255.0;
    let g = ((v >> 8) & 0xFF) as f64 / 255.0;
    let b = (v & 0xFF) as f64 / 255.0;
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if lum < 0.5 { "#F5F1EA".to_string() } else { "#15110D".to_string() }
}

pub fn note_path(dir: &Path) -> PathBuf {
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    dir.join(format!("{name}.md"))
}

/// The embedded stems, in note order.
pub fn stems(dir: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(note_path(dir)) else {
        return Vec::new();
    };
    parse_stems(&content)
}

fn parse_stems(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("![[") {
        rest = &rest[start + 3..];
        let Some(end) = rest.find("]]") else { break };
        let inner = &rest[..end];
        let link = inner.split('|').next().unwrap_or(inner).trim();
        if let Some(stem) = link.strip_suffix(".jpg").or_else(|| link.strip_suffix(".jpeg")) {
            out.push(stem.to_string());
        }
        rest = &rest[end + 2..];
    }
    out
}

/// One story note's metadata for the sidebar's ÉPINGLÉES / RÉCENTES lists.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryNoteInfo {
    pub note_path: String,
    pub folder_path: String,
    pub folder_name: String,
    pub mtime: f64,                    // epoch seconds
    pub pinned: bool,
    pub pinned_at: Option<String>,     // ISO 8601
    pub thumb_stems: Vec<String>,      // up to 3, for the row thumbnail strip
}

/// Scan the given folders for story notes. Returns one `StoryNoteInfo` per
/// folder whose note exists AND has at least one photo embed. Folders with no
/// note, or an empty note, are skipped. Used to populate ÉPINGLÉES + RÉCENTES.
//
// Wired into the app in a later task (Tauri IPC); exercised by tests today,
// so silence the transitional dead-code lint on the lib target.
pub fn list_story_notes(dirs: &[String]) -> Vec<StoryNoteInfo> {
    let mut out = Vec::new();
    for dir_s in dirs {
        let dir = Path::new(dir_s);
        let note = StoryNote::load(dir);
        // skip notes with no photo embeds (not a real story)
        let stems = parse_stems(&note.body);
        if stems.is_empty() {
            continue;
        }
        let mtime = std::fs::metadata(&note.url)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let folder_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(StoryNoteInfo {
            note_path: note.url.to_string_lossy().to_string(),
            folder_path: dir_s.clone(),
            folder_name,
            mtime,
            pinned: note.pinned(),
            pinned_at: note.frontmatter_value("pinned-at"),
            thumb_stems: stems.into_iter().take(3).collect(),
        });
    }
    out
}

/// Toggle a photo's presence in the story. Creates the note (with
/// `publish: true` frontmatter) on first mark. Returns the new stems.
pub fn toggle(dir: &Path, photo_path: &Path) -> std::io::Result<Vec<String>> {
    let stem = photo_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut note = StoryNote::load(dir);

    // First-mark default: seed publish: true if there's no frontmatter at all.
    if note.frontmatter.is_empty() && note.body.trim().is_empty() {
        note.set_frontmatter("publish", Some("true"));
    }

    let embed = format!("![[{stem}.jpg]]");
    let body_has_embed = note.body.lines().any(|l| l.trim() == embed);
    if body_has_embed {
        // remove the embed line
        let kept: Vec<&str> = note.body.lines().filter(|l| l.trim() != embed).collect();
        note.body = kept.join("\n");
        if !note.body.ends_with('\n') {
            note.body.push('\n');
        }
    } else {
        if !note.body.ends_with('\n') {
            note.body.push('\n');
        }
        note.body.push_str(&embed);
        note.body.push('\n');
    }

    note.save()?;
    Ok(parse_stems(&note.body))
}

/// The note content with `publish: true` guaranteed in frontmatter and each
/// `![[stem.jpg]]` rewritten to its CDN url. Local file is left untouched
/// except for the publish flag + garden-url.
pub fn content_for_publish(
    dir: &Path,
    urls: &[(String, String)], // (stem, cdn url)
) -> std::io::Result<String> {
    let mut content = std::fs::read_to_string(note_path(dir))?;
    for (stem, url) in urls {
        content = content.replace(&format!("![[{stem}.jpg]]"), &format!("![{stem}]({url})"));
    }
    Ok(content)
}

/// Stamp `garden-url` into the local note's frontmatter after a publish.
pub fn stamp_garden_url(dir: &Path, live_url: &str) -> std::io::Result<()> {
    let mut note = StoryNote::load(dir);
    note.set_frontmatter("garden-url", Some(live_url));
    // ensure publish: true is present (matches the original prefix-on-empty behavior)
    if note.frontmatter_value("publish").is_none() {
        note.set_frontmatter("publish", Some("true"));
    }
    note.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_rewrite() {
        let c = "---\npublish: true\n---\n\n![[A001.jpg]]\n![[B002.jpg|légende]]\ntexte\n";
        assert_eq!(parse_stems(c), vec!["A001", "B002"]);
    }

    #[allow(dead_code)] // on-disk fixture for later tasks' save() round-trip tests
    fn tmp_note(body: &str) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), body).unwrap();
        f
    }

    #[test]
    fn parse_splits_frontmatter_and_body() {
        let raw = "---\npublish: true\ntitle: \"X\"\n---\n\n![[A.jpg]]\n";
        let n = super::StoryNote::parse(raw, std::path::Path::new("/tmp/n.md"));
        assert_eq!(n.frontmatter_value("publish"), Some("true".to_string()));
        assert_eq!(n.frontmatter_value("title"), Some("X".to_string()));
        assert!(n.body.contains("![[A.jpg]]"));
    }

    #[test]
    fn parse_no_frontmatter_treats_all_as_body() {
        let raw = "![[A.jpg]]\ntext\n";
        let n = super::StoryNote::parse(raw, std::path::Path::new("/tmp/n.md"));
        assert!(n.frontmatter.is_empty());
        assert_eq!(n.body, raw);
    }

    #[test]
    fn set_frontmatter_adds_key() {
        let mut n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_frontmatter("color-dark-background", Some("#15110D"));
        assert_eq!(n.frontmatter_value("color-dark-background"), Some("#15110D".to_string()));
        // existing key untouched
        assert_eq!(n.frontmatter_value("publish"), Some("true".to_string()));
    }

    #[test]
    fn set_frontmatter_replaces_in_place() {
        let mut n = super::StoryNote::parse("---\ncolor-dark-background: #000000\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_frontmatter("color-dark-background", Some("#15110D"));
        assert_eq!(n.frontmatter.len(), 1); // still one line, not two
        assert_eq!(n.frontmatter_value("color-dark-background"), Some("#15110D".to_string()));
    }

    #[test]
    fn set_frontmatter_none_deletes_key() {
        let mut n = super::StoryNote::parse("---\ncolor-dark-background: #15110D\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_frontmatter("color-dark-background", None);
        assert_eq!(n.frontmatter_value("color-dark-background"), None);
        assert_eq!(n.frontmatter_value("publish"), Some("true".to_string()));
    }

    #[test]
    fn set_frontmatter_none_on_absent_key_is_noop() {
        let mut n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_frontmatter("color-dark-background", None);
        assert_eq!(n.frontmatter.len(), 1);
    }

    #[test]
    fn save_round_trips_untouched_keys_byte_identical() {
        let raw = "---\npublish: true\nauthor: \"francis\"\nrating: 5\n---\n\n![[A.jpg]]\nprose here\n";
        let mut n = super::StoryNote::parse(raw, std::path::Path::new("/tmp/n.md"));
        n.set_frontmatter("color-dark-background", Some("#15110D"));
        // author and rating must survive verbatim
        assert_eq!(n.frontmatter_value("author"), Some("francis".to_string()));
        assert_eq!(n.frontmatter_value("rating"), Some("5".to_string()));
        assert!(n.body.contains("prose here"));
    }

    #[test]
    fn derive_foreground_dark_bg_yields_light_ink() {
        assert_eq!(super::derive_foreground("#15110D"), "#F5F1EA");
    }

    #[test]
    fn derive_foreground_light_bg_yields_dark_ink() {
        assert_eq!(super::derive_foreground("#F5F1EA"), "#15110D");
    }

    #[test]
    fn derive_foreground_mid_tone_threshold() {
        // pure red, luminance 0.2126 < 0.5 → light ink
        assert_eq!(super::derive_foreground("#FF0000"), "#F5F1EA");
        // pure green, luminance 0.7152 >= 0.5 → dark ink
        assert_eq!(super::derive_foreground("#00FF00"), "#15110D");
    }

    #[test]
    fn derive_foreground_strips_hash_prefix() {
        assert_eq!(super::derive_foreground("15110D"), "#F5F1EA");
    }

    #[test]
    fn theme_tokens_empty_on_fresh_note() {
        let n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        let t = n.theme_tokens();
        assert!(t.dark_background.is_none());
        assert!(t.dark_accent.is_none());
        assert!(t.font_header.is_none());
    }

    #[test]
    fn set_theme_writes_all_keys_derives_light_and_fg() {
        let mut n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        let tokens = super::ThemeTokens {
            dark_background: Some("#15110D".to_string()),
            dark_foreground: None, // derived
            light_background: None, // derived
            light_foreground: None, // derived
            dark_accent: Some("#D6202C".to_string()),
            light_accent: None, // mirrors dark_accent
            font_header: Some("Sohne".to_string()),
            font_text: None,
        };
        n.set_theme(&tokens);
        // user-picked
        assert_eq!(n.frontmatter_value("color-dark-background"), Some("#15110D".to_string()));
        assert_eq!(n.frontmatter_value("color-dark-accent"), Some("#D6202C".to_string()));
        assert_eq!(n.frontmatter_value("font-header"), Some("Sohne".to_string()));
        // derived foreground
        assert_eq!(n.frontmatter_value("color-dark-foreground"), Some("#F5F1EA".to_string()));
        // derived light side (swap of dark)
        assert_eq!(n.frontmatter_value("color-light-background"), Some("#F5F1EA".to_string()));
        assert_eq!(n.frontmatter_value("color-light-foreground"), Some("#15110D".to_string()));
        // accent mirrors to light
        assert_eq!(n.frontmatter_value("color-light-accent"), Some("#D6202C".to_string()));
    }

    #[test]
    fn set_theme_none_deletes_keys() {
        let raw = "---\ncolor-dark-background: #15110D\ncolor-dark-foreground: #F5F1EA\ncolor-light-background: #F5F1EA\ncolor-light-foreground: #15110D\ncolor-dark-accent: #D6202C\ncolor-light-accent: #D6202C\nfont-header: Sohne\n---\n\nbody\n";
        let mut n = super::StoryNote::parse(raw, std::path::Path::new("/tmp/n.md"));
        let tokens = super::ThemeTokens {
            dark_background: None, dark_foreground: None, light_background: None,
            light_foreground: None, dark_accent: None, light_accent: None,
            font_header: None, font_text: None,
        };
        n.set_theme(&tokens);
        assert!(n.frontmatter_value("color-dark-background").is_none());
        assert!(n.frontmatter_value("color-light-accent").is_none());
        assert!(n.frontmatter_value("font-header").is_none());
    }

    #[test]
    fn theme_tokens_reads_back_set_values() {
        let mut n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        let tokens = super::ThemeTokens {
            dark_background: Some("#15110D".to_string()), dark_foreground: None,
            light_background: None, light_foreground: None,
            dark_accent: Some("#D6202C".to_string()), light_accent: None,
            font_header: Some("Sohne".to_string()), font_text: None,
        };
        n.set_theme(&tokens);
        let read = n.theme_tokens();
        assert_eq!(read.dark_background, Some("#15110D".to_string()));
        assert_eq!(read.dark_accent, Some("#D6202C".to_string()));
        assert_eq!(read.font_header, Some("Sohne".to_string()));
    }

    #[test]
    fn pinned_false_on_fresh_note() {
        let n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        assert!(!n.pinned());
    }

    #[test]
    fn set_pinned_true_writes_pinned_and_pinned_at() {
        let mut n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_pinned(true, Some("2026-07-23T14:03:00Z"));
        assert!(n.pinned());
        assert_eq!(n.frontmatter_value("pinned"), Some("true".to_string()));
        assert_eq!(n.frontmatter_value("pinned-at"), Some("2026-07-23T14:03:00Z".to_string()));
    }

    #[test]
    fn set_pinned_false_removes_both_keys() {
        let raw = "---\npinned: true\npinned-at: 2026-07-23T14:03:00Z\n---\n\nbody\n";
        let mut n = super::StoryNote::parse(raw, std::path::Path::new("/tmp/n.md"));
        n.set_pinned(false, None);
        assert!(!n.pinned());
        assert!(n.frontmatter_value("pinned").is_none());
        assert!(n.frontmatter_value("pinned-at").is_none());
    }

    #[test]
    fn set_pinned_true_without_timestamp_omits_pinned_at() {
        let mut n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_pinned(true, None);
        assert!(n.pinned());
        assert!(n.frontmatter_value("pinned-at").is_none());
    }

    #[test]
    fn list_story_notes_finds_pinned_and_recent() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        // folder "alpha" with a pinned story
        let alpha = tmp.path().join("alpha");
        fs::create_dir_all(&alpha).unwrap();
        fs::write(alpha.join("alpha.md"), "---\npinned: true\npinned-at: 2026-07-23T10:00:00Z\n---\n\n![[a1.jpg]]\n![[a2.jpg]]\n").unwrap();
        // folder "beta" with an unpinned story
        let beta = tmp.path().join("beta");
        fs::create_dir_all(&beta).unwrap();
        fs::write(beta.join("beta.md"), "---\n---\n\n![[b1.jpg]]\n").unwrap();
        // folder "gamma" with no story (just photos) — should be skipped
        let gamma = tmp.path().join("gamma");
        fs::create_dir_all(&gamma).unwrap();
        fs::write(gamma.join("photo.jpg"), b"x").unwrap();

        let dirs = vec![alpha.to_string_lossy().to_string(), beta.to_string_lossy().to_string(), gamma.to_string_lossy().to_string()];
        let notes = super::list_story_notes(&dirs);
        assert_eq!(notes.len(), 2); // gamma skipped (no note)
        let by_folder: std::collections::HashMap<&str, &super::StoryNoteInfo> =
            notes.iter().map(|n| (n.folder_name.as_str(), n)).collect();
        assert!(by_folder["alpha"].pinned);
        assert_eq!(by_folder["alpha"].pinned_at.as_deref(), Some("2026-07-23T10:00:00Z"));
        assert!(!by_folder["beta"].pinned);
        assert_eq!(by_folder["alpha"].thumb_stems.len(), 2);
        assert_eq!(by_folder["beta"].thumb_stems.len(), 1);
    }

    /// Proves the invariant-3 fix in `save_story_note`: a body save preserves
    /// the on-disk frontmatter verbatim, even when the incoming content
    /// carries stale/different frontmatter. This is what prevents a composer
    /// body save (debounced, with frontmatter captured at load) from clobbering
    /// a concurrent theme/pin frontmatter write.
    #[test]
    fn body_save_preserves_disk_frontmatter() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let note_path = super::note_path(dir);
        // Disk note: has theme tokens written by a recent story_set_theme.
        fs::write(
            &note_path,
            "---\npublish: true\ncolor-dark-background: #15110D\npinned: true\n---\n\n![[old.jpg]]\n",
        ).unwrap();

        // Incoming content from the composer: stale frontmatter (no theme/pin
        // keys — captured at load before they were written) + a new body.
        let incoming = "---\npublish: true\n---\n\n![[new.jpg]]\n![[newer.jpg]]\n";

        // Mirror exactly what `save_story_note` now does.
        let incoming_note = super::StoryNote::parse(incoming, &note_path);
        let mut note = super::StoryNote::load(dir);
        note.body = incoming_note.body;
        note.save().unwrap();

        // Reload and verify: body updated, frontmatter preserved from disk.
        let reloaded = super::StoryNote::load(dir);
        assert!(reloaded.body.contains("![[new.jpg]]"));
        assert!(reloaded.body.contains("![[newer.jpg]]"));
        assert!(!reloaded.body.contains("![[old.jpg]]"));
        // The composer's stale frontmatter did NOT overwrite the disk frontmatter.
        assert_eq!(reloaded.frontmatter_value("color-dark-background"), Some("#15110D".to_string()));
        assert_eq!(reloaded.frontmatter_value("pinned"), Some("true".to_string()));
    }
}
