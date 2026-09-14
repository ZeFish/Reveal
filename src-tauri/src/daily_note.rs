use chrono::{DateTime, Datelike, Local};
use std::path::PathBuf;

/// Appends Obsidian embeds and captions into the vault's daily note (`Logs/yymmdd.md`).
///
/// Conforms to the Standard specification (`3-archives/2.5-daily-note-standard.md` & `vault/journal`):
/// - Daily note lives at `<vault>/<logs_folder>/yymmdd.md` (e.g. `Logs/260816.md`).
/// - Created from the standard template when missing (`tags: [log]`, `# <d> <mois>`).
/// - Entries stack chronologically under `## HH:MM`.
/// - Captions are formatted as an Obsidian callout (`> [!caption]`).
pub struct DailyNote {
    pub vault_path: PathBuf,
    pub logs_folder: String,
}

impl DailyNote {
    pub fn new(vault_path: PathBuf, logs_folder: Option<String>) -> Self {
        let logs_folder = logs_folder
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Logs".to_string());
        Self { vault_path, logs_folder }
    }

    /// The path to the daily note for a given date.
    pub fn note_path(&self, dt: &DateTime<Local>) -> PathBuf {
        let stamp = dt.format("%y%m%d").to_string();
        self.vault_path
            .join(&self.logs_folder)
            .join(format!("{stamp}.md"))
    }

    /// Appends one daily-note entry for one or more photo attachments.
    ///
    /// Creates the directory and template note if missing.
    pub fn append_photos(
        &self,
        attachment_names: &[String],
        caption: Option<&str>,
        dt: DateTime<Local>,
    ) -> Result<PathBuf, String> {
        if attachment_names.is_empty() {
            return Err("No attachment names provided".to_string());
        }

        if self.vault_path.as_os_str().is_empty() || !self.vault_path.exists() {
            return Err(format!(
                "Vault directory does not exist: '{}'",
                self.vault_path.display()
            ));
        }

        let logs_dir = self.vault_path.join(&self.logs_folder);
        std::fs::create_dir_all(&logs_dir)
            .map_err(|e| format!("Could not create daily note directory '{}': {e}", logs_dir.display()))?;

        let note_file = self.note_path(&dt);
        let content = if note_file.exists() {
            std::fs::read_to_string(&note_file).map_err(|e| e.to_string())?
        } else {
            Self::template(&dt)
        };

        let time_str = dt.format("%H:%M").to_string();
        let embeds = attachment_names
            .iter()
            .map(|name| format!("![[{name}]]"))
            .collect::<Vec<_>>()
            .join("\n");

        let mut block = format!("## {time_str}\n\n{embeds}\n");
        if let Some(c) = caption {
            let trimmed = c.trim();
            if !trimmed.is_empty() {
                let body = trimmed
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                block.push_str(&format!("\n> [!caption]\n{body}\n"));
            }
        }

        let trimmed_content = content.trim_end();
        let updated = format!("{trimmed_content}\n\n{block}\n");

        std::fs::write(&note_file, updated)
            .map_err(|e| format!("Could not write daily note '{}': {e}", note_file.display()))?;
        Ok(note_file)
    }

    /// Initial note template when the day file does not exist yet.
    fn template(dt: &DateTime<Local>) -> String {
        const MONTHS_FR: [&str; 12] = [
            "janvier", "février", "mars", "avril", "mai", "juin",
            "juillet", "août", "septembre", "octobre", "novembre", "décembre",
        ];
        let iso = dt.format("%Y-%m-%d").to_string();
        let day = dt.day();
        let month_idx = (dt.month() as usize).saturating_sub(1).min(11);
        let month_fr = MONTHS_FR[month_idx];
        let human = format!("{day} {month_fr}");

        format!(
            "---\ntitle:\naliases:\ncreated: {iso}\nmodified: {iso}\ntags: [log]\ntype: note\npublish: false\nvisibility: private\n---\n# {human}\n\n"
        )
    }
}
