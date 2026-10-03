//! Tidy — a plan for filing a folder's photos by the import rule.
//!
//! The import files a card's photos into `<archive>/<pattern>` by capture date
//! (`%Y/%Y-%m-%d`, set in Settings). This module asks the same question of
//! photos that are already on disk: where would the rule put each of them?
//!
//! It only ever *plans*. Nothing here moves, writes or deletes a file; the
//! output is a description of what could be done, for a person to read first.
//!
//! The rule is applied with respect for what a person already did:
//!
//! - A day folder may carry a name after the date (`2018-04-03 - Portrait`).
//!   That is the rule plus a label, and counts as following it.
//! - A photo that sits inside a folder a person named (`190305 - Kenya`) is
//!   *kept*: the name is a filing decision, and the tool does not undo it.
//! - A photo with no capture date is left where it is. A date taken from the
//!   file's modification time is not a capture date; it is flagged, and left too.
//! - A capture date in the future means the camera's clock was wrong. Filing
//!   by it would build a folder for a year that has not happened, so the photo
//!   is left, and counted apart.
//! - Nothing is ever planned onto a name that is already taken.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Where a photo's date came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dating {
    /// Read from the camera's own metadata: a capture date.
    Exif,
    /// Only the file's modification time: not a capture date.
    FileTime,
    /// None at all.
    None,
}

/// Why a photo would move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// In a folder named for the right day, but not where the rule puts it
    /// (a missing year folder, for instance).
    WrongPlace,
    /// In a folder named for another day.
    WrongDay,
    /// In no day folder at all.
    NotFiled,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    /// Already where the rule puts it.
    InPlace,
    /// Would move to `to`.
    Move { reason: Reason },
    /// Inside a folder a person named; left alone.
    Kept { folder: String },
    /// Would move, but something is in the way.
    Conflict { why: String },
    /// No capture date; left alone.
    Undated,
}

/// One photo as found on disk, before any decision.
#[derive(Clone, Debug)]
pub struct Photo {
    pub path: PathBuf,
    pub ts: Option<i64>,
    pub dating: Dating,
    /// Files that belong with it and would travel with it.
    pub companions: Vec<PathBuf>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Item {
    pub from: PathBuf,
    /// The file it would become. `None` when it stays.
    pub to: Option<PathBuf>,
    pub verdict: Verdict,
    pub dating: Dating,
    pub companions: usize,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Summary {
    pub photos: usize,
    pub in_place: usize,
    pub to_move: usize,
    pub kept: usize,
    pub conflicts: usize,
    pub undated: usize,
    /// Of the undated, those that have only a file time.
    pub file_time_only: usize,
    /// Of the undated, those whose capture date is in the future.
    pub future_dated: usize,
    /// Files that are not photos, left untouched.
    pub other_files: usize,
    /// Day folders the moves would create.
    pub new_folders: usize,
    /// Companion files (sidecars, videos, audio notes, camera JPEGs) that would follow moved photos.
    pub companions: usize,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TidyPlan {
    pub root: PathBuf,
    pub base: PathBuf,
    pub pattern: String,
    pub items: Vec<Item>,
    pub summary: Summary,
}

/// What a plan is made against.
pub struct Config<'a> {
    /// The folder being tidied.
    pub root: &'a Path,
    /// Where the rule's folders live: the archive the import files into.
    pub base: &'a Path,
    /// The `strftime` pattern, as in Settings.
    pub pattern: &'a str,
    /// Now, in unix seconds: nothing later than tomorrow can be a capture date.
    pub now: i64,
}

/// A name a person chose for an event or a trip: a date, then " - ", then words.
/// `190305 - Kenya`, `20190305 - Kenya`, `2019-03-05 - Kenya`.
pub fn is_labelled_event(name: &str) -> bool {
    let (date, label) = match name.split_once(" - ").or_else(|| name.split_once(" \u{2013} ")) {
        Some(parts) => parts,
        None => return false,
    };
    let all_digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let iso = date.len() == 10
        && date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    (iso || (all_digits(date) && (date.len() == 6 || date.len() == 8))) && !label.trim().is_empty()
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a.components().eq(b.components())
}

fn last(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Whether `current` is `expected`, or `expected` with a label after its last
/// component (`2018-04-03` is followed by `2018-04-03 - Portrait`).
fn follows_rule(current: &Path, expected: &Path) -> bool {
    if same_dir(current, expected) {
        return true;
    }
    match (current.parent(), expected.parent()) {
        (Some(cp), Some(ep)) if same_dir(cp, ep) => {
            let (c, e) = (last(current), last(expected));
            c.strip_prefix(&e)
                .map(|rest| rest.starts_with(" - ") || rest.starts_with(" \u{2013} "))
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// `YYYY-MM-DD`, possibly with a label after it: a folder named for a day.
fn looks_like_a_day(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
        && (b.len() == 10 || name[10..].starts_with(" - ") || name[10..].starts_with(" \u{2013} "))
}

/// The first folder, between the tidied one (exclusive) and the photo's own
/// (inclusive), that a person named. A photo inside one is kept.
fn named_ancestor(root: &Path, dir: &Path) -> Option<String> {
    let rel = dir.strip_prefix(root).ok()?;
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .find(|name| is_labelled_event(name))
}

/// Plan the filing of `photos`. `exists` answers whether a path is taken, so the
/// plan can be made against a real disk or a pretend one.
pub fn plan(cfg: &Config<'_>, photos: Vec<Photo>, other_files: usize, exists: &dyn Fn(&Path) -> bool) -> TidyPlan {
    let mut summary = Summary { other_files, ..Summary::default() };
    let mut items = Vec::with_capacity(photos.len());
    // What the plan itself has already claimed, case-insensitively: a Mac volume
    // does not tell `A.RAF` from `a.raf`.
    let mut claimed: HashMap<String, PathBuf> = HashMap::new();
    let mut new_dirs: BTreeMap<PathBuf, ()> = BTreeMap::new();

    for photo in photos {
        summary.photos += 1;
        let from = photo.path.clone();
        let name = last(&from);
        let cur_dir = from.parent().map(Path::to_path_buf).unwrap_or_default();
        let dated = photo.ts.filter(|_| photo.dating == Dating::Exif);
        let future = dated.is_some_and(|t| t > cfg.now + 86_400);

        let Some(ts) = dated.filter(|_| !future) else {
            summary.undated += 1;
            if photo.dating == Dating::FileTime {
                summary.file_time_only += 1;
            }
            if future {
                summary.future_dated += 1;
            }
            items.push(Item { from, to: None, verdict: Verdict::Undated, dating: photo.dating, companions: photo.companions.len() });
            continue;
        };

        let dest_dir = crate::dated_dir_for_timestamp(cfg.base, cfg.pattern, ts);

        if follows_rule(&cur_dir, &dest_dir) {
            summary.in_place += 1;
            items.push(Item { from, to: None, verdict: Verdict::InPlace, dating: photo.dating, companions: photo.companions.len() });
            continue;
        }
        if let Some(folder) = named_ancestor(cfg.root, &cur_dir) {
            summary.kept += 1;
            items.push(Item { from, to: None, verdict: Verdict::Kept { folder }, dating: photo.dating, companions: photo.companions.len() });
            continue;
        }

        let cur_name = last(&cur_dir);
        let expected_name = last(&dest_dir);
        let reason = if cur_name == expected_name {
            Reason::WrongPlace
        } else if looks_like_a_day(&cur_name) {
            Reason::WrongDay
        } else {
            Reason::NotFiled
        };

        let to = dest_dir.join(&name);
        // Everything that would travel, to the names it would take.
        let mut targets: Vec<PathBuf> = vec![to.clone()];
        for c in &photo.companions {
            targets.push(dest_dir.join(last(c)));
        }
        let blocked = targets.iter().find_map(|t| {
            let key = t.to_string_lossy().to_lowercase();
            if exists(t) {
                Some(format!("“{}” is already in {}", last(t), last(&dest_dir)))
            } else if let Some(other) = claimed.get(&key) {
                Some(format!("another photo, {}, would take the same name", last(other)))
            } else {
                None
            }
        });

        match blocked {
            Some(why) => {
                summary.conflicts += 1;
                items.push(Item { from, to: Some(to), verdict: Verdict::Conflict { why }, dating: photo.dating, companions: photo.companions.len() });
            }
            None => {
                for t in &targets {
                    claimed.insert(t.to_string_lossy().to_lowercase(), from.clone());
                }
                if !exists(&dest_dir) {
                    new_dirs.insert(dest_dir.clone(), ());
                }
                summary.to_move += 1;
                summary.companions += photo.companions.len();
                items.push(Item { from, to: Some(to), verdict: Verdict::Move { reason }, dating: photo.dating, companions: photo.companions.len() });
            }
        }
    }
    summary.new_folders = new_dirs.len();

    TidyPlan { root: cfg.root.to_path_buf(), base: cfg.base.to_path_buf(), pattern: cfg.pattern.to_string(), items, summary }
}

// ------------------------------------------------------------------- finding

/// Extensions treated as standard photo images when no corresponding RAW exists.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "heic", "heif", "png", "tiff", "tif", "webp",
];

/// Read capture timestamp from either RAW metadata (libraw) or standard image EXIF (kamadak-exif).
pub fn capture_timestamp(path: &Path) -> Option<i64> {
    if let Some(ts) = reveal_decode::capture_timestamp(path) {
        if ts > 0 {
            return Some(ts);
        }
    }

    let file = std::fs::File::open(path).ok()?;
    let mut bufreader = std::io::BufReader::new(file);
    let exifreader = exif::Reader::new();
    let exif_data = exifreader.read_from_container(&mut bufreader).ok()?;

    let tag = exif_data
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif_data.get_field(exif::Tag::DateTimeDigitized, exif::In::PRIMARY))
        .or_else(|| exif_data.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;

    let date_str = match &tag.value {
        exif::Value::Ascii(ref vec) if !vec.is_empty() => std::str::from_utf8(&vec[0]).ok()?,
        _ => return None,
    };

    use chrono::TimeZone;
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(date_str.trim(), "%Y:%m:%d %H:%M:%S") {
        if let Some(local) = chrono::Local.from_local_datetime(&naive).single() {
            return Some(local.timestamp());
        }
    }

    None
}

/// Files that belong with a photo and would travel with it: its sidecars (both
/// naming styles), its JPEG renders, Live Photo clips, and audio notes.
pub fn companions_of(photo: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name), Some(stem)) = (
        photo.parent(),
        photo.file_name().map(|n| n.to_string_lossy().into_owned()),
        photo.file_stem().map(|n| n.to_string_lossy().into_owned()),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for candidate in [
        format!("{name}.xmp"),
        format!("{stem}.xmp"),
        format!("{stem}.jpg"),
        format!("{stem}.JPG"),
        format!("{stem}.jpeg"),
        format!("{stem}.JPEG"),
        format!("{stem}.preview.jpg"),
        format!("{stem}.mov"),
        format!("{stem}.MOV"),
        format!("{stem}.mp4"),
        format!("{stem}.MP4"),
        format!("{stem}.m4v"),
        format!("{stem}.M4V"),
        format!("{stem}.wav"),
        format!("{stem}.WAV"),
        format!("{stem}.m4a"),
        format!("{stem}.M4A"),
    ] {
        let p = dir.join(candidate);
        if p != photo && p.is_file() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Every photo under `root` (RAWs and standalone images; hidden folders and files skipped),
/// with its companions, and the number of other visible files, which stay untouched.
pub fn find_photos(root: &Path) -> (Vec<PathBuf>, usize) {
    let mut photos = Vec::new();
    let mut others = 0usize;
    let mut stack = vec![root.to_path_buf()];

    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        let mut dir_raws = Vec::new();
        let mut dir_images = Vec::new();
        let mut dir_other_files = Vec::new();

        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
                if reveal_decode::RAW_EXTENSIONS.contains(&ext.as_str()) {
                    dir_raws.push(p);
                } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                    dir_images.push(p);
                } else {
                    dir_other_files.push(p);
                }
            }
        }

        let mut claimed_companions: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

        // 1. Process all RAWs first (primary photos)
        for raw in &dir_raws {
            let companions = companions_of(raw);
            for c in companions {
                claimed_companions.insert(c);
            }
            photos.push(raw.clone());
        }

        // 2. Process images that are NOT claimed as companions of any RAW (standalone photos)
        for img in dir_images {
            if claimed_companions.contains(&img) {
                continue;
            }
            let companions = companions_of(&img);
            for c in companions {
                claimed_companions.insert(c);
            }
            photos.push(img);
        }

        // 3. Count other files that are not companions of any photo
        for other in dir_other_files {
            if !claimed_companions.contains(&other) {
                others += 1;
            }
        }
    }

    photos.sort();
    (photos, others)
}

/// Read each photo's date, in parallel, calling `progress(done, total)`.
/// `known` may answer from somewhere cheaper than the file (the index).
pub fn read_photos(
    photos: Vec<PathBuf>,
    known: &(dyn Fn(&Path) -> Option<i64> + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Vec<Photo> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let total = photos.len();
    let done = AtomicUsize::new(0);
    let result: Vec<Photo> = photos
        .into_par_iter()
        .map(|path| {
            let (ts, dating) = match known(&path).filter(|t| *t > 0).or_else(|| capture_timestamp(&path)) {
                Some(t) => (Some(t), Dating::Exif),
                None => match std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                {
                    Some(d) => (Some(d.as_secs() as i64), Dating::FileTime),
                    None => (None, Dating::None),
                },
            };
            let companions = companions_of(&path);
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 200 == 0 || n == total {
                progress(n, total);
            }
            Photo { path, ts, dating, companions }
        })
        .collect();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Noon local time on a date: far from any midnight, so timezone never decides the day.
    fn noon(y: i32, m: u32, d: u32) -> i64 {
        chrono::Local.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap().timestamp()
    }

    fn photo(path: &str, ts: Option<i64>) -> Photo {
        Photo { path: PathBuf::from(path), ts, dating: if ts.is_some() { Dating::Exif } else { Dating::None }, companions: vec![] }
    }

    fn run(root: &str, photos: Vec<Photo>, taken: &[&str]) -> TidyPlan {
        let taken: Vec<String> = taken.iter().map(|s| s.to_string()).collect();
        let cfg = Config { root: Path::new(root), base: Path::new("/a/Capture"), pattern: "%Y/%Y-%m-%d", now: noon(2026, 10, 1) };
        plan(&cfg, photos, 0, &|p: &Path| taken.iter().any(|t| Path::new(t) == p))
    }

    fn verdicts(plan: &TidyPlan) -> Vec<&Verdict> {
        plan.items.iter().map(|i| &i.verdict).collect()
    }

    #[test]
    fn a_photo_in_the_right_day_folder_stays() {
        let p = run("/a", vec![photo("/a/Capture/2026/2026-09-30/A.RAF", Some(noon(2026, 9, 30)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::InPlace]);
        assert_eq!(p.summary.in_place, 1);
    }

    #[test]
    fn a_day_folder_with_a_name_after_the_date_follows_the_rule() {
        let p = run("/a", vec![photo("/a/Capture/2018/2018-04-03 - Portrait/A.RAF", Some(noon(2018, 4, 3)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::InPlace]);
    }

    #[test]
    fn the_right_day_without_its_year_folder_would_move() {
        let p = run("/a", vec![photo("/a/Capture/2024-06-29/A.RAF", Some(noon(2024, 6, 29)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::Move { reason: Reason::WrongPlace }]);
        assert_eq!(p.items[0].to.as_deref(), Some(Path::new("/a/Capture/2024/2024-06-29/A.RAF")));
        assert_eq!(p.summary.new_folders, 1);
    }

    #[test]
    fn a_day_folder_holding_another_day_would_move() {
        let p = run("/a", vec![photo("/a/Capture/2025/2025-09-13/A.RAF", Some(noon(2025, 8, 2)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::Move { reason: Reason::WrongDay }]);
    }

    #[test]
    fn a_photo_in_no_day_folder_would_move() {
        let p = run("/a", vec![photo("/a/Temp Import/A.RAF", Some(noon(2025, 8, 2)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::Move { reason: Reason::NotFiled }]);
        assert_eq!(p.items[0].to.as_deref(), Some(Path::new("/a/Capture/2025/2025-08-02/A.RAF")));
    }

    #[test]
    fn photos_in_a_folder_a_person_named_are_kept_even_deep_inside_it() {
        let p = run("/a", vec![photo("/a/190305 - Kenya/Capture/Day 2/A.RAF", Some(noon(2019, 3, 6)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::Kept { folder: "190305 - Kenya".into() }]);
        assert!(p.items[0].to.is_none());
    }

    #[test]
    fn pointing_the_tool_at_the_named_folder_itself_tidies_what_is_in_it() {
        // The tidied folder's own name is not a reason to keep: the person chose it.
        let p = run("/a/190305 - Kenya", vec![photo("/a/190305 - Kenya/A.RAF", Some(noon(2019, 3, 6)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::Move { reason: Reason::NotFiled }]);
    }

    #[test]
    fn a_photo_without_a_capture_date_is_left_and_a_file_time_is_flagged_not_trusted() {
        let mut by_file_time = photo("/a/Temp/B.RAF", Some(noon(2020, 1, 1)));
        by_file_time.dating = Dating::FileTime;
        let p = run("/a", vec![photo("/a/Temp/A.RAF", None), by_file_time], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::Undated, &Verdict::Undated]);
        assert_eq!((p.summary.undated, p.summary.file_time_only), (2, 1));
    }

    #[test]
    fn a_capture_date_in_the_future_is_a_wrong_clock_not_a_folder() {
        let p = run("/a", vec![photo("/a/Temp/A.RAF", Some(noon(2028, 6, 20)))], &[]);
        assert_eq!(verdicts(&p), vec![&Verdict::Undated]);
        assert_eq!((p.summary.undated, p.summary.future_dated, p.summary.to_move), (1, 1, 0));
        // Tomorrow is fine: clocks and time zones disagree by less than a day.
        let p = run("/a", vec![photo("/a/Temp/B.RAF", Some(noon(2026, 10, 1) + 3600))], &[]);
        assert!(matches!(verdicts(&p)[0], Verdict::Move { .. }));
    }

    #[test]
    fn nothing_is_planned_onto_a_name_already_taken() {
        let p = run(
            "/a",
            vec![photo("/a/Temp/A.RAF", Some(noon(2025, 8, 2)))],
            &["/a/Capture/2025/2025-08-02/A.RAF"],
        );
        assert!(matches!(verdicts(&p)[0], Verdict::Conflict { .. }));
        assert_eq!(p.summary.to_move, 0);
    }

    #[test]
    fn two_photos_aiming_at_one_name_do_not_both_move_even_if_the_case_differs() {
        let p = run(
            "/a",
            vec![
                photo("/a/Temp/IMG_1.RAF", Some(noon(2025, 8, 2))),
                photo("/a/Other/img_1.raf", Some(noon(2025, 8, 2))),
            ],
            &[],
        );
        assert!(matches!(verdicts(&p)[0], Verdict::Move { .. }));
        assert!(matches!(verdicts(&p)[1], Verdict::Conflict { .. }));
    }

    #[test]
    fn companions_travel_and_block_together() {
        let mut ph = photo("/a/Temp/A.RAF", Some(noon(2025, 8, 2)));
        ph.companions = vec![PathBuf::from("/a/Temp/A.RAF.xmp"), PathBuf::from("/a/Temp/A.jpg")];
        let ok = run("/a", vec![ph.clone()], &[]);
        assert_eq!(ok.items[0].companions, 2);
        assert!(matches!(ok.items[0].verdict, Verdict::Move { .. }));
        // The sidecar's name is taken at the destination: the whole frame waits.
        let blocked = run("/a", vec![ph], &["/a/Capture/2025/2025-08-02/A.RAF.xmp"]);
        assert!(matches!(blocked.items[0].verdict, Verdict::Conflict { .. }));
    }

    #[test]
    fn the_plan_follows_the_pattern_it_is_given() {
        let cfg = Config { root: Path::new("/a"), base: Path::new("/a/Capture"), pattern: "%Y-%m", now: noon(2026, 10, 1) };
        let p = plan(&cfg, vec![photo("/a/Temp/A.RAF", Some(noon(2025, 8, 2)))], 0, &|_| false);
        assert_eq!(p.items[0].to.as_deref(), Some(Path::new("/a/Capture/2025-08/A.RAF")));
    }

    #[test]
    fn what_counts_as_a_named_event() {
        for yes in ["190305 - Kenya", "20190305 - Kenya", "2019-03-05 - Kenya", "190305 \u{2013} Kenya"] {
            assert!(is_labelled_event(yes), "{yes}");
        }
        for no in ["Capture", "2019-03-05", "190305", "Temp Import", "1903 - Kenya", "190305 - ", "2018"] {
            assert!(!is_labelled_event(no), "{no}");
        }
    }

    #[test]
    fn find_photos_handles_raw_pairs_standalone_jpegs_and_multimedia_companions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // 1. RAW with JPEG, sidecar, and video clip
        std::fs::write(root.join("DSC0001.ARW"), b"raw").unwrap();
        std::fs::write(root.join("DSC0001.JPG"), b"jpg").unwrap();
        std::fs::write(root.join("DSC0001.xmp"), b"xmp").unwrap();
        std::fs::write(root.join("DSC0001.MOV"), b"mov").unwrap();

        // 2. Standalone JPEG with live photo video companion
        std::fs::write(root.join("IMG_0002.JPG"), b"jpg2").unwrap();
        std::fs::write(root.join("IMG_0002.MOV"), b"mov2").unwrap();

        // 3. Unrelated non-photo file
        std::fs::write(root.join("notes.txt"), b"text").unwrap();

        let (photos, others) = find_photos(root);

        // Should find exactly 2 primary photos: DSC0001.ARW and IMG_0002.JPG
        assert_eq!(photos.len(), 2);
        assert_eq!(others, 1); // only notes.txt is an untouched other file

        let raw_photo = photos.iter().find(|p| p.ends_with("DSC0001.ARW")).unwrap();
        let raw_companions = companions_of(raw_photo);
        assert!(raw_companions.contains(&root.join("DSC0001.JPG")));
        assert!(raw_companions.contains(&root.join("DSC0001.xmp")));
        assert!(raw_companions.contains(&root.join("DSC0001.MOV")));

        let standalone_jpeg = photos.iter().find(|p| p.ends_with("IMG_0002.JPG")).unwrap();
        let jpeg_companions = companions_of(standalone_jpeg);
        assert!(jpeg_companions.contains(&root.join("IMG_0002.MOV")));
    }
}
