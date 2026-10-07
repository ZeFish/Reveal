//! `reveal://thumb` — the grid's (and the viewer's) picture of a photo.
//!
//! One request in, one JPEG out. The path a request takes:
//!
//!   1. **Parse** it ([`ThumbRequest::parse`]): which photo, what size, which version, and
//!      whether it is the one photo on screen (`priority`).
//!   2. **Local cache** — bytes already on this disk answer before anything touches the NAS.
//!   3. **Offline** — if the photo's volume is not mounted, serve what this machine remembers.
//!   4. **Wait for a slot** ([`crate::thumb_queue`]) — newest request first; one that nobody has
//!      waited for in a long while is dropped.
//!   5. **Climb the ladder** ([`climb`]) — the first source that has the picture wins:
//!      developed sidecar → the camera's embedded preview → the companion JPEG → a neutral
//!      render of the RAW → the file read as a plain image.
//!
//! Until now all of this was one closure in `lib.rs`. The ladder's order is the part that
//! matters and the part that regressed before, so it is a function over a trait
//! ([`Rungs`]) and tested without a photo or an app.

use tauri::http::Response as HttpResponse;
use tauri::{Emitter, Manager};

use crate::photo::{Origin, Photo};
use crate::preview::*;
use crate::thumb_queue;

// ── 1. the request ──────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ThumbRequest {
    pub(crate) path: String,
    /// Longest edge asked for, clamped to what a thumbnail may be.
    pub(crate) size: u32,
    /// The version the frontend believes this photo's preview is at. Part of the cache key,
    /// so an edit to `.preview.jpg` outside Reveal moves the key and the stale entry is not
    /// found.
    pub(crate) version: u64,
    /// The photo actually on screen. It must not queue behind the grid: restoring a session
    /// fires ~120 cell requests and then opens one photo, and on a loaded NAS a cell took 4–6 s.
    /// There is at most one of these at a time, so it skips the queue entirely.
    pub(crate) priority: bool,
    /// The photo as the camera shot it, whatever develop settings it carries: skip the developed
    /// sidecar and the cache of developed renders, and write nothing back. Asked for by the
    /// switch to "None", which must show the original at once — before (and whether or not) the
    /// photo's develop settings have been cleared from disk.
    pub(crate) as_shot: bool,
}

fn param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
}

impl ThumbRequest {
    /// `None` when the request names no photo.
    pub(crate) fn parse(query: Option<&str>) -> Option<Self> {
        let query = query.unwrap_or_default();
        let path = param(query, "p").map(crate::percent_decode).unwrap_or_default();
        if path.is_empty() {
            return None;
        }
        Some(Self {
            path,
            size: param(query, "size")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(GRID_PREVIEW_EDGE)
                .clamp(256, 2560),
            version: param(query, "v").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
            priority: query.split('&').any(|kv| kv == "priority=1"),
            as_shot: query.split('&').any(|kv| kv == "asshot=1"),
        })
    }

    fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

// ── the answer ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Reply {
    Jpeg { bytes: Vec<u8>, cache_control: &'static str },
    Fail { status: u16, body: Vec<u8> },
}

impl Reply {
    const CACHEABLE: &'static str = "max-age=3600";

    fn jpeg(bytes: Vec<u8>) -> Self {
        Self::Jpeg { bytes, cache_control: Self::CACHEABLE }
    }
    fn not_found() -> Self {
        Self::Fail { status: 404, body: Vec::new() }
    }
    /// Could not be served now, and may be asked for again (the queue gave up on it, or a
    /// source was unreachable).
    fn unavailable(body: Vec<u8>) -> Self {
        Self::Fail { status: 503, body }
    }

    fn into_http(self) -> HttpResponse<Vec<u8>> {
        match self {
            Self::Jpeg { bytes, cache_control } => HttpResponse::builder()
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", cache_control)
                .body(bytes)
                .unwrap(),
            Self::Fail { status, body } => HttpResponse::builder().status(status).body(body).unwrap(),
        }
    }
}

// ── 5. the ladder ───────────────────────────────────────────────────────────────────────

/// What a photo can be asked for, one source at a time. Each returns bytes already sized for
/// the request (and has already kept a copy where that is worth doing).
pub(crate) trait Rungs {
    /// The developed `.preview.jpg` next to the RAW — the truth, when the photo was developed.
    /// `None`: there is none. `Some(Err)`: there is one and it could not be read.
    fn sidecar(&self) -> Option<Result<Vec<u8>, String>>;
    /// The camera's own embedded preview. Cheapest by far: a grid cell needs a few hundred
    /// pixels, and decoding a full companion JPEG (60–100+ MB once decoded) for that once
    /// pushed memory into the tens of GB opening one RAW+JPEG folder.
    fn embedded(&self) -> Result<Vec<u8>, String>;
    /// The JPEG the camera wrote beside the RAW. `None`: there is none.
    fn companion(&self) -> Option<Result<Vec<u8>, String>>;
    /// A neutral render of the RAW (Rapid at its defaults), kept as `.preview.jpg` so this is
    /// a one-time cost. Touches only the durable JPEG, never the recipe: the photo still reads
    /// as "None" in Develop.
    fn neutral(&self) -> Result<Vec<u8>, String>;
    /// The file read as an ordinary image: a Google Takeout export hands back JPEGs still
    /// named `.DNG`, which every RAW route refuses.
    fn plain(&self) -> Option<Vec<u8>>;
}

/// Walk the ladder. `None`: nothing could produce a picture (answer 404).
///
/// A sidecar or companion that exists but cannot be read ends the climb: there is a picture,
/// it is just not reachable now, and guessing a lesser one would show the wrong photo state.
///
/// `as_shot` leaves the first rung out: the developed sidecar is exactly what was not asked for.
pub(crate) fn climb(rungs: &dyn Rungs, as_shot: bool) -> Option<Vec<u8>> {
    if !as_shot {
        if let Some(sidecar) = rungs.sidecar() {
            return sidecar.ok();
        }
    }
    if let Ok(bytes) = rungs.embedded() {
        return Some(bytes);
    }
    if let Some(companion) = rungs.companion() {
        return companion.ok();
    }
    rungs.neutral().ok().or_else(|| rungs.plain())
}

/// The ladder over real files and the real engine.
struct FileRungs<'a> {
    app: &'a tauri::AppHandle,
    req: &'a ThumbRequest,
    started: std::time::Instant,
}

impl FileRungs<'_> {
    fn source(&self) -> &std::path::Path {
        std::path::Path::new(&self.req.path)
    }
    fn ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }
    /// Keep a render as the photo's durable `.preview.jpg` (see `persist_thumb_cache`) — unless it
    /// is an as-shot look, which must never become the developed preview.
    fn keep(&self, bytes: &[u8], size: u32) {
        if !self.req.as_shot {
            persist_thumb_cache(self.source(), bytes, size);
        }
    }
}

impl Rungs for FileRungs<'_> {
    fn sidecar(&self) -> Option<Result<Vec<u8>, String>> {
        let preview_path = Photo::new(&self.req.path).preview_sidecar().filter(|p| p.is_file())?;
        Some(match std::fs::read(&preview_path) {
            Ok(bytes) => {
                eprintln!(
                    "thumb: {} (developed sidecar, {} ko, {} ms)",
                    self.req.name(),
                    bytes.len() / 1024,
                    self.ms()
                );
                // Resize once, then both serve and keep it: the NAS round trip happens once
                // per photo per size, and the decode and re-encode once rather than per request.
                let sized = downscale_grid_thumb(bytes, self.req.size, 1);
                cache_developed_preview_locally(self.app, self.source(), &sized, self.req.size, self.req.version);
                Ok(sized)
            }
            Err(e) => {
                eprintln!("thumb {} legacy preview: {e}", preview_path.display());
                Err(e.to_string())
            }
        })
    }

    fn embedded(&self) -> Result<Vec<u8>, String> {
        match reveal_decode::extract_thumb_preview(self.source()) {
            Ok(preview) => {
                eprintln!(
                    "thumb: {} ({}, {} ko, {} ms)",
                    self.req.name(),
                    preview.mime,
                    preview.bytes.len() / 1024,
                    self.ms()
                );
                let small = downscale_grid_thumb(preview.bytes, self.req.size, preview.orientation);
                self.keep(&small, self.req.size);
                Ok(small)
            }
            Err(e) => {
                eprintln!("thumb {}: no embedded preview ({e})", self.req.path);
                Err(e.to_string())
            }
        }
    }

    fn companion(&self) -> Option<Result<Vec<u8>, String>> {
        let companion = Photo::new(&self.req.path).companion_jpeg()?;
        // The most faithful "as shot" source, correct even for a monochrome film simulation
        // the sensor data alone cannot reproduce (a RAW is always colour).
        eprintln!("thumb {}: trying the companion jpg", self.req.path);
        Some(match std::fs::read(&companion) {
            Ok(bytes) => {
                eprintln!(
                    "thumb: {} (companion jpg, {} ko, {} ms)",
                    self.req.name(),
                    bytes.len() / 1024,
                    self.ms()
                );
                let small = downscale_grid_thumb(bytes, self.req.size, 1);
                self.keep(&small, self.req.size);
                Ok(small)
            }
            Err(e) => {
                eprintln!("thumb {} companion jpg read: {e}", companion.display());
                Err(e.to_string())
            }
        })
    }

    fn neutral(&self) -> Result<Vec<u8>, String> {
        eprintln!("thumb {}: generating a neutral preview", self.req.path);
        let engine = self.app.state::<crate::EngineState>().0.clone();
        let neutral = reveal_engine::Recipe { engine: "rapid".to_string(), ..reveal_engine::Recipe::default() };
        match engine.develop_jpeg(self.source(), &neutral, 2048) {
            Ok(out) => {
                eprintln!("thumb-fallback: {} ({} ko, {} ms)", self.req.name(), out.jpeg.len() / 1024, self.ms());
                self.keep(&out.jpeg, DURABLE_PREVIEW_EDGE);
                Ok(downscale_grid_thumb(out.jpeg, self.req.size, 1))
            }
            Err(e) => {
                eprintln!("thumb fallback {}: {e:#}", self.req.path);
                Err(format!("{e:#}"))
            }
        }
    }

    fn plain(&self) -> Option<Vec<u8>> {
        let bytes = plain_image_bytes(self.source())?;
        eprintln!("thumb: {} (not a raw — plain image, {} ko)", self.req.name(), bytes.len() / 1024);
        Some(downscale_grid_thumb(bytes, self.req.size, 1))
    }
}

// ── the handler ─────────────────────────────────────────────────────────────────────────

/// Answer one `reveal://thumb` request. Returns at once; the answer arrives through `responder`.
pub(crate) fn serve(
    app: tauri::AppHandle,
    request: &tauri::http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let Some(req) = ThumbRequest::parse(request.uri().query()) else {
        responder.respond(Reply::not_found().into_http());
        return;
    };

    // The local cache comes before the mount guard, deliberately. These bytes are on this disk;
    // whether the NAS is awake is beside the point. Guarding first meant that on a cold start
    // (the NFS automount not yet materialised) the photo you were editing came back "Preview
    // unavailable" while its pixels sat in the cache.
    if let Some(bytes) = (!req.as_shot).then(|| local_cache_hit(&app, &req)).flatten() {
        eprintln!("thumb: {} (local cache, {} ko)", req.name(), bytes.len() / 1024);
        responder.respond(Reply::jpeg(bytes).into_http());
        return;
    }

    if !crate::is_volume_mounted(std::path::Path::new(&req.path)) {
        responder.respond(offline(&app, &req).into_http());
        return;
    }

    let queue = app.state::<thumb_queue::ThumbQueueState>().0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Bounds how many of these run at once, newest request first, and drops one nobody has
        // waited for in a long while — see `thumb_queue`. A priority request holds no slot: it
        // is the photo on screen.
        let _slot = if req.priority {
            None
        } else {
            match queue.acquire(thumb_queue::MAX_WAIT) {
                Some(slot) => Some(slot),
                None => {
                    // The cell asked long ago and has most likely scrolled away; if it is
                    // still there, it asks again.
                    responder.respond(Reply::unavailable(Vec::new()).into_http());
                    return;
                }
            }
        };
        responder.respond(decode(&app, &req).into_http());
    });
}

fn local_cache_hit(app: &tauri::AppHandle, req: &ThumbRequest) -> Option<Vec<u8>> {
    let local = developed_preview_cache_path(app, std::path::Path::new(&req.path), req.size, req.version).ok()?;
    let bytes = std::fs::read(&local).ok()?;
    // An entry that is there but empty is not a hit. Serving it leaves the cell blank for good;
    // dropping it lets this request fall through and the next one re-cache properly.
    if bytes.is_empty() {
        let _ = std::fs::remove_file(&local);
        return None;
    }
    Some(bytes)
}

/// The volume is not reachable. The exact cache key needed a version the NAS alone can tell
/// us, so fall back to the newest render this disk holds for the photo, and tell the UI the
/// source is unreachable: whether or not a cached copy answers, the photographer should know
/// they are looking at what this machine remembers rather than at the archive.
fn offline(app: &tauri::AppHandle, req: &ThumbRequest) -> Reply {
    let _ = app.emit("source-offline", serde_json::json!({ "path": req.path }));
    match newest_cached_render(app, std::path::Path::new(&req.path), req.size).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => {
            eprintln!("thumb: {} (offline — newest local copy, {} ko)", req.name(), bytes.len() / 1024);
            Reply::jpeg(bytes)
        }
        None => Reply::not_found(),
    }
}

fn decode(app: &tauri::AppHandle, req: &ThumbRequest) -> Reply {
    let origin = Photo::new(&req.path).origin();
    if origin == Origin::ApplePhotos {
        return match crate::apple_photos::thumbnail(&req.path, req.size) {
            Ok(bytes) => Reply::Jpeg {
                bytes: if req.size <= 768 { downscale_grid_thumb(bytes, req.size, 1) } else { bytes },
                cache_control: "no-cache",
            },
            Err(error) => {
                eprintln!("Apple Photos thumbnail: {error}");
                Reply::unavailable(error.into_bytes())
            }
        };
    }
    if origin == Origin::Immich {
        return match crate::immich::thumbnail(app, &req.path, req.size) {
            Ok(bytes) => Reply::jpeg(if req.size <= 768 { downscale_grid_thumb(bytes, req.size, 1) } else { bytes }),
            Err(error) => {
                eprintln!("Immich thumbnail: {error}");
                Reply::unavailable(error.into_bytes())
            }
        };
    }
    let rungs = FileRungs { app, req, started: std::time::Instant::now() };
    match climb(&rungs, req.as_shot) {
        Some(bytes) => Reply::jpeg(bytes),
        None => Reply::not_found(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    // ── the request ──

    #[test]
    fn a_request_without_a_photo_is_refused() {
        assert_eq!(ThumbRequest::parse(None), None);
        assert_eq!(ThumbRequest::parse(Some("v=3&size=768")), None);
        assert_eq!(ThumbRequest::parse(Some("p=")), None);
    }

    #[test]
    fn a_request_reads_its_photo_size_version_and_priority() {
        let req = ThumbRequest::parse(Some("p=%2Fmnt%2Fa%20b%2FIMG_1.NEF&v=42&size=1024&priority=1")).unwrap();
        assert_eq!(req.path, "/mnt/a b/IMG_1.NEF");
        assert_eq!(req.size, 1024);
        assert_eq!(req.version, 42);
        assert!(req.priority);
        assert_eq!(req.name(), "IMG_1.NEF");
    }

    #[test]
    fn a_bare_request_gets_the_grid_size_version_zero_and_no_priority() {
        let req = ThumbRequest::parse(Some("p=%2Fa.raf")).unwrap();
        assert_eq!((req.size, req.version, req.priority, req.as_shot), (GRID_PREVIEW_EDGE, 0, false, false));
    }

    #[test]
    fn an_as_shot_request_is_marked_and_nothing_else_is() {
        assert!(ThumbRequest::parse(Some("p=%2Fa.raf&asshot=1")).unwrap().as_shot);
        assert!(!ThumbRequest::parse(Some("p=%2Fa.raf&asshot=0")).unwrap().as_shot);
        assert!(!ThumbRequest::parse(Some("p=%2Fa.raf&notasshot=1")).unwrap().as_shot);
    }

    #[test]
    fn the_size_is_kept_within_what_a_thumbnail_may_be() {
        assert_eq!(ThumbRequest::parse(Some("p=a&size=10")).unwrap().size, 256);
        assert_eq!(ThumbRequest::parse(Some("p=a&size=99999")).unwrap().size, 2560);
        assert_eq!(ThumbRequest::parse(Some("p=a&size=abc")).unwrap().size, GRID_PREVIEW_EDGE);
    }

    #[test]
    fn a_parameter_is_matched_whole_not_by_prefix() {
        // `pp=`/`vv=`/`priority=0` must not be taken for `p=`/`v=`/`priority=1`.
        let req = ThumbRequest::parse(Some("pp=x&p=%2Fa.raf&vv=9&priority=0")).unwrap();
        assert_eq!((req.path.as_str(), req.version, req.priority), ("/a.raf", 0, false));
    }

    // ── the answer ──

    #[test]
    fn a_jpeg_reply_carries_its_type_and_cache_header() {
        let r = Reply::jpeg(vec![1, 2, 3]).into_http();
        assert_eq!(r.status(), 200);
        assert_eq!(r.headers()["Content-Type"], "image/jpeg");
        assert_eq!(r.headers()["Cache-Control"], "max-age=3600");
        assert_eq!(r.body(), &vec![1, 2, 3]);
    }

    #[test]
    fn failures_are_404_when_there_is_nothing_and_503_when_it_may_come_later() {
        assert_eq!(Reply::not_found().into_http().status(), 404);
        assert_eq!(Reply::unavailable(b"busy".to_vec()).into_http().status(), 503);
    }

    // ── the ladder ──

    /// A photo with only the rungs a test names; records which were tried, in order.
    #[derive(Default)]
    struct Fake {
        sidecar: Option<Result<Vec<u8>, String>>,
        embedded: Option<Vec<u8>>,
        companion: Option<Result<Vec<u8>, String>>,
        neutral: Option<Vec<u8>>,
        plain: Option<Vec<u8>>,
        tried: RefCell<Vec<&'static str>>,
    }

    impl Rungs for Fake {
        fn sidecar(&self) -> Option<Result<Vec<u8>, String>> {
            self.tried.borrow_mut().push("sidecar");
            self.sidecar.clone()
        }
        fn embedded(&self) -> Result<Vec<u8>, String> {
            self.tried.borrow_mut().push("embedded");
            self.embedded.clone().ok_or_else(|| "none".to_string())
        }
        fn companion(&self) -> Option<Result<Vec<u8>, String>> {
            self.tried.borrow_mut().push("companion");
            self.companion.clone()
        }
        fn neutral(&self) -> Result<Vec<u8>, String> {
            self.tried.borrow_mut().push("neutral");
            self.neutral.clone().ok_or_else(|| "cannot".to_string())
        }
        fn plain(&self) -> Option<Vec<u8>> {
            self.tried.borrow_mut().push("plain");
            self.plain.clone()
        }
    }

    fn b(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    #[test]
    fn the_developed_sidecar_wins_and_nothing_else_is_tried() {
        let f = Fake { sidecar: Some(Ok(b("developed"))), embedded: Some(b("embedded")), ..Default::default() };
        assert_eq!(climb(&f, false), Some(b("developed")));
        assert_eq!(*f.tried.borrow(), vec!["sidecar"]);
    }

    #[test]
    fn an_as_shot_request_never_looks_at_the_developed_sidecar() {
        // A photo with develop settings, asked for as the camera shot it: the sidecar is not even
        // consulted, and the climb goes on to the camera's own preview.
        let f = Fake { sidecar: Some(Ok(b("developed"))), embedded: Some(b("embedded")), ..Default::default() };
        assert_eq!(climb(&f, true), Some(b("embedded")));
        assert_eq!(*f.tried.borrow(), vec!["embedded"]);
    }

    #[test]
    fn a_sidecar_that_cannot_be_read_ends_the_climb() {
        // The developed picture exists; showing the camera's embedded one instead would show the
        // photo as it was before its edits.
        let f = Fake { sidecar: Some(Err("io".into())), embedded: Some(b("embedded")), ..Default::default() };
        assert_eq!(climb(&f, false), None);
        assert_eq!(*f.tried.borrow(), vec!["sidecar"]);
    }

    #[test]
    fn without_a_sidecar_the_embedded_preview_is_tried_first() {
        let f = Fake { embedded: Some(b("embedded")), companion: Some(Ok(b("companion"))), ..Default::default() };
        assert_eq!(climb(&f, false), Some(b("embedded")));
        assert_eq!(*f.tried.borrow(), vec!["sidecar", "embedded"]);
    }

    #[test]
    fn the_companion_jpeg_comes_after_the_embedded_preview_and_before_a_render() {
        let f = Fake { companion: Some(Ok(b("companion"))), neutral: Some(b("neutral")), ..Default::default() };
        assert_eq!(climb(&f, false), Some(b("companion")));
        assert_eq!(*f.tried.borrow(), vec!["sidecar", "embedded", "companion"]);
    }

    #[test]
    fn a_companion_that_cannot_be_read_ends_the_climb() {
        let f = Fake { companion: Some(Err("io".into())), neutral: Some(b("neutral")), ..Default::default() };
        assert_eq!(climb(&f, false), None);
        assert_eq!(*f.tried.borrow(), vec!["sidecar", "embedded", "companion"]);
    }

    #[test]
    fn a_raw_with_nothing_beside_it_is_rendered_neutrally() {
        let f = Fake { neutral: Some(b("neutral")), plain: Some(b("plain")), ..Default::default() };
        assert_eq!(climb(&f, false), Some(b("neutral")));
        assert_eq!(*f.tried.borrow(), vec!["sidecar", "embedded", "companion", "neutral"]);
    }

    #[test]
    fn a_file_the_raw_decoder_refuses_is_read_as_a_plain_image() {
        let f = Fake { plain: Some(b("plain")), ..Default::default() };
        assert_eq!(climb(&f, false), Some(b("plain")));
        assert_eq!(*f.tried.borrow(), vec!["sidecar", "embedded", "companion", "neutral", "plain"]);
    }

    #[test]
    fn when_every_rung_fails_there_is_no_picture() {
        let f = Fake::default();
        assert_eq!(climb(&f, false), None);
        assert_eq!(*f.tried.borrow(), vec!["sidecar", "embedded", "companion", "neutral", "plain"]);
    }
}
