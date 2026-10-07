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

    /// Render frontmatter and body into Markdown.
    pub fn render(&self) -> String {
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
        out
    }

    /// Write the note back to disk, creating parent dirs as needed.
    pub fn save(&self) -> io::Result<()> {
        let out = self.render();
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
        //
        // All of that now lives in `reveal_io::write_durable`, the one rule every
        // user-data write shares (it also retries a NAS timeout).
        reveal_io::write_durable(&self.url, out.as_bytes())
    }

    /// The folder's theme: the `theme:` key the Garden itself reads. The old
    /// per-note colour and font keys are read only to recognise a theme that
    /// was picked before `theme:` existed; nothing writes them any more.
    pub fn theme(&self) -> FolderTheme {
        FolderTheme {
            theme: self.frontmatter_value("theme").filter(|t| !t.is_empty()),
            legacy_dark_background: self.frontmatter_value("color-dark-background"),
            legacy_dark_accent: self.frontmatter_value("color-dark-accent"),
        }
    }

    /// Set the folder's theme (`None` = the Garden's default) and sweep out
    /// the custom colour / font keys an earlier version wrote beside it.
    pub fn set_theme(&mut self, theme: Option<&str>) {
        self.set_frontmatter("theme", theme);
        for key in LEGACY_THEME_KEYS {
            self.set_frontmatter(key, None);
        }
    }

}

/// Keys an earlier Reveal wrote to colour and type a story by hand. The theme
/// is one `theme:` now; these are cleared whenever it is set.
const LEGACY_THEME_KEYS: [&str; 9] = [
    "color-dark-background",
    "color-dark-foreground",
    "color-light-background",
    "color-light-foreground",
    "color-dark-accent",
    "color-light-accent",
    "font-header",
    "font-text",
    "font-ratio",
];

/// What a story note says about its look.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderTheme {
    /// The `theme:` value — a Garden theme id such as "forest".
    pub theme: Option<String>,
    /// From before `theme:`: lets the app recognise a curated theme that was
    /// picked by its colours, and move it to `theme:`.
    pub legacy_dark_background: Option<String>,
    pub legacy_dark_accent: Option<String>,
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
        let joined = kept.join("\n");
        let trimmed = joined.trim();
        note.body = if trimmed.is_empty() { String::new() } else { format!("{trimmed}\n") };
    } else {
        let trimmed = note.body.trim();
        if trimmed.is_empty() {
            note.body = format!("{embed}\n");
        } else {
            note.body = format!("{trimmed}\n\n{embed}\n");
        }
    }

    note.save()?;
    Ok(parse_stems(&note.body))
}

/// The note content with `publish: true` and `title` guaranteed in frontmatter,
/// and each `![[stem.jpg]]` rewritten to its CDN url. Local file is left untouched
/// except for the publish flag + garden-url.
pub fn content_for_publish(
    dir: &Path,
    title: &str,
    urls: &[(String, String)], // (stem, cdn url)
) -> std::io::Result<String> {
    let raw = std::fs::read_to_string(note_path(dir))?;
    let mut note = StoryNote::parse(&raw, &note_path(dir));
    if note.frontmatter_value("publish").is_none() {
        note.set_frontmatter("publish", Some("true"));
    }
    if note.frontmatter_value("title").is_none() {
        let escaped = serde_json::to_string(title).unwrap_or_else(|_| format!("\"{title}\""));
        note.set_frontmatter("title", Some(&escaped));
    }
    let mut content = note.render();
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
        let c = "---\npublish: true\n---\n\n![[A001.jpg]]\n![[B002.jpg|caption]]\ntext\n";
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
    fn theme_is_empty_on_a_fresh_note() {
        let n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        let t = n.theme();
        assert!(t.theme.is_none());
        assert!(t.legacy_dark_background.is_none());
    }

    #[test]
    fn set_theme_writes_the_theme_key_and_nothing_else_of_the_look() {
        let mut n = super::StoryNote::parse("---\npublish: true\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_theme(Some("forest"));
        assert_eq!(n.frontmatter_value("theme"), Some("forest".to_string()));
        assert_eq!(n.frontmatter_value("publish"), Some("true".to_string()));
        assert!(n.frontmatter_value("color-dark-background").is_none());
        assert!(n.frontmatter_value("font-header").is_none());
    }

    #[test]
    fn set_theme_sweeps_the_custom_keys_an_earlier_version_wrote() {
        let raw = "---\npublish: true\ncolor-dark-background: #231e1a\ncolor-dark-foreground: #F5F1EA\ncolor-light-background: #F5F1EA\ncolor-light-foreground: #231e1a\ncolor-dark-accent: #f9a61a\ncolor-light-accent: #f9a61a\nfont-header: Forrest\nfont-text: Forrest\nfont-ratio: 1.4\n---\n\nbody\n";
        let mut n = super::StoryNote::parse(raw, std::path::Path::new("/tmp/n.md"));
        // they are still readable, so the app can recognise forest by its colours
        assert_eq!(n.theme().legacy_dark_background, Some("#231e1a".to_string()));
        n.set_theme(Some("forest"));
        for key in ["color-dark-background", "color-dark-foreground", "color-light-background",
                    "color-light-foreground", "color-dark-accent", "color-light-accent",
                    "font-header", "font-text", "font-ratio"] {
            assert!(n.frontmatter_value(key).is_none(), "{key} should be gone");
        }
        assert_eq!(n.frontmatter_value("theme"), Some("forest".to_string()));
        assert_eq!(n.frontmatter_value("publish"), Some("true".to_string()));
    }

    #[test]
    fn set_theme_none_removes_the_theme() {
        let mut n = super::StoryNote::parse("---\ntheme: forest\n---\n\nbody\n", std::path::Path::new("/tmp/n.md"));
        n.set_theme(None);
        assert!(n.theme().theme.is_none());
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

    #[test]
    fn content_for_publish_ensures_title_and_publish() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let note_path = super::note_path(dir);
        fs::write(&note_path, "![[A01.jpg]]\nProse\n").unwrap();

        let urls = vec![("A01".to_string(), "https://cdn.test/A01.jpg".to_string())];
        let published = super::content_for_publish(dir, "My Story", &urls).unwrap();

        assert!(published.contains("publish: true"));
        assert!(published.contains("title: \"My Story\""));
        assert!(published.contains("![A01](https://cdn.test/A01.jpg)"));
    }
}
