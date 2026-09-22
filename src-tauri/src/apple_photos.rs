//! PhotoKit is a read-only media source; edits never live inside Photos or its cache.
use super::photo_cache::{CacheLease, CacheStatus, Cleanup, PhotoCache, DEFAULT_LIMIT, GIB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{Emitter, Manager};

const PREFIX: &str = "apple-photos://";
static STORAGE: OnceLock<Storage> = OnceLock::new();
static DOWNLOAD: Mutex<()> = Mutex::new(());
static EDITS: Mutex<()> = Mutex::new(());

struct Storage {
    edits: PathBuf,
    cache: Arc<PhotoCache>,
    app: tauri::AppHandle,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub capture_at: Option<i64>,
    pub revision: f64,
}

pub fn init(app: &tauri::AppHandle) -> Result<(), String> {
    let handle = app.clone();
    let cache = Arc::new(PhotoCache::new(
        app.path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("apple-photos"),
        cache_limit(&super::load_preferences(app.clone()))?,
        move |error| {
            eprintln!("Apple Photos cache: {error}");
            let _ = handle.emit(
                "app-error",
                json!({"message": format!("Apple Photos cache: {error}")}),
            );
        },
    )?);
    let storage = Storage {
        edits: app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("apple-photos/edits"),
        cache: cache.clone(),
        app: app.clone(),
    };
    let edits_dir = storage.edits.clone();
    STORAGE
        .set(storage)
        .map_err(|_| "Apple Photos was already initialized".to_string())?;
    cache.schedule_trim();
    sweep_empty_edit_dirs(edits_dir);
    Ok(())
}

pub fn cache_limit(preferences: &Value) -> Result<u64, String> {
    match preferences.get("apple_photos_cache_limit_gib") {
        None => Ok(DEFAULT_LIMIT),
        Some(value) => value
            .as_u64()
            .filter(|value| (1..=64).contains(value))
            .map(|value| value * GIB)
            .ok_or_else(|| {
                "Apple Photos cache limit must be a whole number from 1 to 64 GiB".to_string()
            }),
    }
}

pub fn set_cache_limit(limit: u64) -> Result<(), String> {
    storage()?.cache.set_limit(limit);
    Ok(())
}

#[tauri::command]
pub async fn apple_photos_cache_status() -> Result<CacheStatus, String> {
    if !cfg!(target_os = "macos") {
        return Err("Apple Photos is available only on macOS".to_string());
    }
    tauri::async_runtime::spawn_blocking(|| storage()?.cache.status())
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apple_photos_cache_clear() -> Result<Cleanup, String> {
    if !cfg!(target_os = "macos") {
        return Err("Apple Photos is available only on macOS".to_string());
    }
    tauri::async_runtime::spawn_blocking(|| storage()?.cache.clear())
        .await
        .map_err(|e| e.to_string())?
}

fn storage() -> Result<&'static Storage, String> {
    STORAGE
        .get()
        .ok_or_else(|| "Apple Photos is not initialized".to_string())
}

pub fn is_asset(path: &str) -> bool {
    path.starts_with(PREFIX)
}

fn encode_id(id: &str) -> String {
    id.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn asset_key(path: &str) -> Result<&str, String> {
    let rest = path
        .strip_prefix(PREFIX)
        .ok_or("Not an Apple Photos asset")?;
    let (key, filename) = rest.split_once('/').ok_or("Invalid Apple Photos asset")?;
    if key.is_empty()
        || key.len() > 1024
        || key.len() % 2 != 0
        || !key.bytes().all(|b| b.is_ascii_hexdigit())
        || filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains(['/', '\\', '\0'])
    {
        return Err("Invalid Apple Photos asset identifier".to_string());
    }
    Ok(key)
}

fn identifier(path: &str) -> Result<String, String> {
    let key = asset_key(path)?;
    let bytes = (0..key.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&key[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn safe_name(name: &str) -> String {
    let name = name.replace(['/', '\\', '\0', ':'], "_");
    if name.is_empty() || name == "." || name == ".." {
        "Photo".to_string()
    } else {
        name
    }
}

/// Remove per-asset edit directories that hold nothing.
///
/// Reading metadata used to create one of these for every asset it touched,
/// so listing an album left one empty directory per photo in it — 718 of 721
/// on this machine. The read path no longer creates them, but the ones
/// already on disk are still there, and only an empty one is safe to remove:
/// a directory with anything in it holds a rating, a caption or a develop.
///
/// Off the startup path, and silent — this is housekeeping, and a failure to
/// tidy is not something to interrupt anyone about.
fn sweep_empty_edit_dirs(edits: PathBuf) {
    std::thread::spawn(move || {
        let Ok(entries) = std::fs::read_dir(&edits) else {
            return;
        };
        let mut removed = 0usize;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // `remove_dir` refuses a directory that is not empty, so it IS
            // the emptiness check — no race between looking and removing.
            if std::fs::remove_dir(&path).is_ok() {
                removed += 1;
            }
        }
        if removed > 0 {
            eprintln!("apple photos: removed {removed} empty edit directories");
        }
    });
}

pub fn metadata_path(path: &str) -> Result<PathBuf, String> {
    if !is_asset(path) {
        return Ok(PathBuf::from(path));
    }
    // Resolves a path; does NOT create it. This used to `create_dir_all`
    // unconditionally, so merely LISTING an album made one empty directory
    // per asset — 718 of the 721 found on disk held nothing at all. Writers
    // create the directory (see `reveal_meta::write` and
    // `write_sidecar_if_changed`); readers must not leave a trace.
    Ok(storage()?.edits.join(asset_key(path)?).join("photo"))
}

pub fn require_file(path: &str) -> Result<(), String> {
    if is_asset(path) {
        Err(
            "Apple Photos is read-only. Export this photo before using filesystem actions."
                .to_string(),
        )
    } else {
        Ok(())
    }
}

pub fn update_metadata(
    path: &str,
    update: impl FnOnce(&mut reveal_meta::Sidecar) -> Result<(), String>,
) -> Result<(), String> {
    let _guard = EDITS.lock().map_err(|e| e.to_string())?;
    let metadata = metadata_path(path)?;
    let mut sidecar = reveal_meta::read(&metadata)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    update(&mut sidecar)?;
    reveal_meta::write(&metadata, &sidecar).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn native(request: Value) -> Result<Value, String> {
    use std::ffi::{CStr, CString};
    unsafe extern "C" {
        fn reveal_photos_request(json: *const std::ffi::c_char) -> *mut std::ffi::c_char;
        fn reveal_photos_free(response: *mut std::ffi::c_char);
    }
    let input = CString::new(request.to_string()).map_err(|e| e.to_string())?;
    // The native bridge owns its autorelease pool. Calls must run on blocking
    // workers, never the AppKit thread that services PhotoKit callbacks.
    let response = unsafe {
        let pointer = reveal_photos_request(input.as_ptr());
        if pointer.is_null() {
            return Err("PhotoKit returned no response".to_string());
        }
        let result = serde_json::from_slice::<Value>(CStr::from_ptr(pointer).to_bytes());
        reveal_photos_free(pointer);
        result.map_err(|e| e.to_string())?
    };
    if let Some(error) = response.get("error").and_then(Value::as_str) {
        return Err(error.to_string());
    }
    Ok(response)
}

#[cfg(not(target_os = "macos"))]
fn native(_request: Value) -> Result<Value, String> {
    Err("Apple Photos is available only on macOS".to_string())
}

#[tauri::command]
pub async fn apple_photos_status(authorize: bool) -> Result<Value, String> {
    if !cfg!(target_os = "macos") {
        return Ok(json!({"supported": false}));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut response =
            native(json!({"operation": if authorize { "authorize" } else { "status" }}))?;
        response["supported"] = json!(true);
        Ok(response)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apple_photos_albums() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| native(json!({"operation": "albums"})))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apple_photos_list(
    album: Option<String>,
    offset: usize,
    descending: bool,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut page = native(json!({"operation": "list", "album": album.unwrap_or_default(), "offset": offset, "descending": descending}))?;
        let assets: Vec<Asset> = serde_json::from_value(page["frames"].clone()).map_err(|e| e.to_string())?;
        let mut frames = Vec::with_capacity(assets.len());
        for asset in assets {
            let name = safe_name(&asset.name);
            let path = format!("{PREFIX}{}/{name}", encode_id(&asset.id));
            let metadata = metadata_path(&path)?;
            let sidecar = reveal_meta::read(&metadata).map_err(|e| e.to_string())?;
            frames.push(json!({
                "path": path, "name": name, "capture_at": asset.capture_at,
                "rating": sidecar.and_then(|s| s.rating).unwrap_or(0),
                "previewVersion": super::served_preview_mtime(&metadata)
            }));
        }
        page["frames"] = json!(frames);
        Ok(page)
    }).await.map_err(|e| e.to_string())?
}

pub fn info(path: &str) -> Result<Asset, String> {
    serde_json::from_value(native(
        json!({"operation": "info", "id": identifier(path)?}),
    )?)
    .map_err(|e| e.to_string())
}

/// Must be called from a blocking worker. A single download at a time bounds
/// full-resolution memory and prevents simultaneous renders from racing the cache.
pub fn source(path: &str) -> Result<Source, String> {
    if !is_asset(path) {
        return Ok(Source {
            path: PathBuf::from(path),
            _lease: None,
        });
    }
    let _guard = DOWNLOAD.lock().map_err(|e| e.to_string())?;
    let asset = info(path)?;
    // A changed asset gets a different decode-cache key without deleting a
    // working file that another render might still be reading.
    let lease = storage()?.cache.lease(
        asset_key(path)?,
        &format!("source-{:016x}", asset.revision.to_bits()),
    )?;
    let directory = lease.path();
    let manifest = directory.join("source.json");
    if manifest.exists() {
        let saved: Value =
            serde_json::from_slice(&std::fs::read(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Invalid photo cache manifest: {e}"))?;
        if let Some(filename) = saved["filename"].as_str() {
            validate_cache_filename(filename)?;
            let cached = directory.join(filename);
            if saved["revision"].as_f64() == Some(asset.revision) && cached.is_file() {
                return Ok(Source {
                    path: cached,
                    _lease: Some(lease),
                });
            }
            if cached.exists() {
                std::fs::remove_file(cached).map_err(|e| e.to_string())?;
            }
        }
    }
    let app = &storage()?.app;
    let _ = app.emit(
        "apple-photos-transfer",
        json!({"path": path, "phase": "loading", "name": asset.name}),
    );
    let result = (|| {
        // Recover only known working filenames after a crash between download
        // completion and manifest commit. This never touches durable edits.
        if !manifest.exists() {
            for filename in std::iter::once("working.tiff".to_string()).chain(
                reveal_decode::RAW_EXTENSIONS
                    .iter()
                    .map(|ext| format!("original.{ext}")),
            ) {
                let candidate = directory.join(filename);
                if candidate.is_file() {
                    std::fs::remove_file(candidate).map_err(|e| e.to_string())?;
                }
            }
        }
        let mut response =
            native(json!({"operation": "source", "id": asset.id, "directory": directory}))?;
        let filename = response["filename"]
            .as_str()
            .ok_or("PhotoKit returned no working image")?
            .to_string();
        validate_cache_filename(&filename)?;
        response["revision"] = json!(asset.revision);
        let temporary = directory.join("source.json.tmp");
        std::fs::write(&temporary, response.to_string()).map_err(|e| e.to_string())?;
        std::fs::rename(temporary, manifest).map_err(|e| e.to_string())?;
        Ok(directory.join(filename))
    })();
    let _ = app.emit(
        "apple-photos-transfer",
        json!({
            "path": path, "phase": if result.is_ok() { "ready" } else { "error" },
            "error": result.as_ref().err(), "name": asset.name
        }),
    );
    result.map(|path| Source {
        path,
        _lease: Some(lease),
    })
}

pub struct Source {
    path: PathBuf,
    _lease: Option<CacheLease>,
}

impl std::ops::Deref for Source {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

fn validate_cache_filename(filename: &str) -> Result<(), String> {
    if filename == "working.tiff"
        || reveal_decode::RAW_EXTENSIONS
            .iter()
            .any(|ext| filename == format!("original.{ext}"))
    {
        Ok(())
    } else {
        Err("Invalid Apple Photos cache filename".to_string())
    }
}

pub fn thumbnail(path: &str, size: u32) -> Result<Vec<u8>, String> {
    let asset = info(path)?;
    let metadata = metadata_path(path)?;
    let developed = reveal_meta::read(&metadata)
        .map_err(|e| e.to_string())?
        .is_some_and(|sidecar| sidecar.engine_settings.is_some());
    if developed {
        if let Some(preview) = super::preview_sidecar_path(&metadata).filter(|p| p.is_file()) {
            return std::fs::read(preview).map_err(|e| e.to_string());
        }
    }
    let lease = storage()?.cache.lease(asset_key(path)?, "previews")?;
    let target = lease
        .path()
        .join(format!("{:016x}-{size}.jpg", asset.revision.to_bits()));
    if !target.is_file() {
        native(
            json!({"operation": "thumbnail", "id": asset.id, "size": size, "destination": target}),
        )?;
    }
    std::fs::read(target).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apple_photos_cancel() {
    #[cfg(target_os = "macos")]
    unsafe {
        unsafe extern "C" {
            fn reveal_photos_cancel();
        }
        reveal_photos_cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The sweep must only ever take empty directories. A populated one holds
    /// a rating, a caption or a develop recipe — the only copy of it, since
    /// Photos itself is read-only.
    #[test]
    fn the_sweep_removes_only_empty_directories() {
        let root = std::env::temp_dir()
            .join(format!("reveal-edits-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        for name in ["empty-a", "empty-b"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        let kept = root.join("has-an-edit");
        std::fs::create_dir_all(&kept).unwrap();
        std::fs::write(kept.join("photo.xmp"), b"<xmp/>").unwrap();
        let kept_preview = root.join("has-a-preview");
        std::fs::create_dir_all(&kept_preview).unwrap();
        std::fs::write(kept_preview.join("photo.preview.jpg"), b"jpeg").unwrap();

        super::sweep_empty_edit_dirs(root.clone());
        // The sweep runs on its own thread; wait for it to settle.
        for _ in 0..100 {
            if std::fs::read_dir(&root).unwrap().count() <= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let mut left: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec!["has-a-preview", "has-an-edit"],
            "only the directories holding something survive"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn asset_identity_round_trips_without_filename_collisions() {
        for id in ["ABC/L0/001", "another/L0/001", "photo-\u{e9}"] {
            let uri = format!("{PREFIX}{}/IMG_0001.HEIC", encode_id(id));
            assert_eq!(identifier(&uri).unwrap(), id);
        }
        assert_ne!(encode_id("ABC/L0/001"), encode_id("DEF/L0/001"));
    }

    #[test]
    fn rejects_traversal_and_malformed_identifiers() {
        for path in [
            "apple-photos://../file",
            "apple-photos://ab/../x",
            "apple-photos://ab/..",
            "apple-photos://zz/file",
            "apple-photos://a/file",
            "apple-photos:///file",
        ] {
            assert!(asset_key(path).is_err(), "{path}");
        }
        assert!(validate_cache_filename("../../outside").is_err());
        assert!(validate_cache_filename("original.dng").is_ok());
        assert!(validate_cache_filename("working.tiff").is_ok());
    }

    #[test]
    fn file_sources_remain_unchanged_and_virtual_assets_cannot_move() {
        let path = "/photos/IMG_0001.raf";
        assert_eq!(&*source(path).unwrap(), Path::new(path));
        assert_eq!(metadata_path(path).unwrap(), Path::new(path));
        assert!(require_file(path).is_ok());
        assert!(require_file("apple-photos://ab/photo").is_err());
        assert_eq!(safe_name("../bad:name"), ".._bad_name");
    }

    #[test]
    fn cache_preference_accepts_only_bounded_integer_limits() {
        assert_eq!(cache_limit(&json!({})).unwrap(), DEFAULT_LIMIT);
        for limit in [1, 4, 64] {
            assert_eq!(
                cache_limit(&json!({"apple_photos_cache_limit_gib": limit})).unwrap(),
                limit * GIB
            );
        }
        for limit in [
            json!(0),
            json!(65),
            json!(-1),
            json!(1.5),
            json!("4"),
            Value::Null,
        ] {
            assert!(cache_limit(&json!({"apple_photos_cache_limit_gib": limit})).is_err());
        }
    }

    #[test]
    fn duplicate_export_names_never_replace_another_photo() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("IMG_0001.jpg");
        let first = super::super::write_photo_export(
            "apple-photos://aa/IMG_0001.HEIC",
            &destination,
            b"one",
        )
        .unwrap();
        let second = super::super::write_photo_export(
            "apple-photos://bb/IMG_0001.HEIC",
            &destination,
            b"two",
        )
        .unwrap();
        assert_eq!(first, destination);
        assert_eq!(second.file_name().unwrap(), "IMG_0001-1.jpg");
        assert_eq!(std::fs::read(first).unwrap(), b"one");
        assert_eq!(std::fs::read(second).unwrap(), b"two");
    }

    #[test]
    fn tiff_working_images_reach_the_engine_decoder() {
        use reveal_decode::RawDecoder;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("working.tiff");
        let pixels = image::RgbImage::from_pixel(3, 2, image::Rgb([128u8, 255, 0]));
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn reveal_photos_convert(
                    source: *const std::ffi::c_char,
                    destination: *const std::ffi::c_char,
                ) -> i32;
            }
            let input = temp.path().join("synthetic.png");
            pixels.save(&input).unwrap();
            let input = std::ffi::CString::new(input.to_str().unwrap()).unwrap();
            let output = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
            assert_eq!(
                unsafe { reveal_photos_convert(input.as_ptr(), output.as_ptr()) },
                1
            );
        }
        #[cfg(not(target_os = "macos"))]
        pixels.save(&path).unwrap();
        let decoded = reveal_decode::DecoderRegistry
            .decode_linear(&path, false)
            .unwrap();
        assert_eq!((decoded.width, decoded.height), (3, 2));
        assert_eq!(decoded.data.len(), 18);
        assert!((decoded.data[0] - 0.21586).abs() < 0.00001);
        assert_eq!(decoded.data[1], 1.0);
        assert_eq!(decoded.data[2], 0.0);

        let engine = reveal_engine::Engine::new(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("data"),
            temp.path().join("luts"),
        )
        .unwrap();
        let recipe = reveal_engine::Recipe {
            engine: "rapid".to_string(),
            ..Default::default()
        };
        let (jpeg, width, height) = engine.export_jpeg(&path, &recipe, 2048, 0.0).unwrap();
        assert_eq!((width, height), (3, 2));
        assert_eq!(image::load_from_memory(&jpeg).unwrap().width(), 3);
    }

    #[test]
    fn metadata_updates_preserve_independent_fields() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("photo").to_string_lossy().into_owned();
        update_metadata(&path, |sidecar| {
            sidecar.rating = Some(4);
            sidecar.description = Some("A caption".to_string());
            Ok(())
        })
        .unwrap();
        update_metadata(&path, |sidecar| {
            sidecar.engine = Some("rapid".to_string());
            sidecar.engine_settings = Some(json!({"exposure_ev": 1}));
            Ok(())
        })
        .unwrap();
        let saved = reveal_meta::read(Path::new(&path)).unwrap().unwrap();
        assert_eq!(saved.rating, Some(4));
        assert_eq!(saved.description.as_deref(), Some("A caption"));
        assert_eq!(saved.engine.as_deref(), Some("rapid"));
        update_metadata(&path, |sidecar| {
            sidecar.engine = None;
            sidecar.engine_settings = None;
            Ok(())
        })
        .unwrap();
        let cleared = reveal_meta::read(Path::new(&path)).unwrap().unwrap();
        assert_eq!(cleared.rating, Some(4));
        assert!(cleared.engine_settings.is_none());
    }
}
