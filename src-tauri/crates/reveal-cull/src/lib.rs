//! AI culling: pick the best frames out of a large import batch.
//!
//! Two stages, both working off the camera's embedded JPEG preview (never a
//! full RAW develop — see `reveal_decode::extract_thumb_preview`, ~10-30 ms
//! per photo):
//!
//! 1. [`prefilter`] — a local pass that rejects technically bad frames
//!    (blur, blown/blocked exposure) and collapses near-duplicate bursts
//!    down to their sharpest few, bounding how many photos go on to stage 2.
//! 2. [`rank_all`] against a [`VisionRanker`] — a cloud vision pass that
//!    picks the "best" survivors by composition/expression, not just
//!    technical cleanliness. [`AnthropicRanker`] is the shipped default
//!    implementation; the trait is the seam for swapping providers later.

mod anthropic;
mod hash;
mod prefilter;
mod rank;

pub use anthropic::AnthropicRanker;
pub use prefilter::{prefilter, PhotoScore, PrefilterConfig};
pub use rank::{rank_all, RankCandidate, RankedResult, VisionRanker};

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum CullError {
    #[error("http: {0}")]
    Http(String),
    #[error("no vision API key configured")]
    MissingApiKey,
}

/// Re-extract and re-encode a photo's embedded preview as a size-capped
/// JPEG, ready to send to a vision API. Called only for stage-1 survivors
/// (not the whole import batch) — keeping full preview bytes in memory for
/// hundreds of photos at once would be wasteful; re-reading the ~80 that
/// made the cut is cheap by comparison.
pub fn embedded_preview_jpeg(path: &str, max_edge: u32) -> Option<Vec<u8>> {
    let preview = reveal_decode::extract_thumb_preview(Path::new(path)).ok()?;
    let img = image::load_from_memory(&preview.bytes).ok()?;
    let resized = if img.width() > max_edge || img.height() > max_edge {
        img.resize(max_edge, max_edge, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut buf = Vec::new();
    resized
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .ok()?;
    Some(buf)
}
