//! Stage 1 — the local pass: reject technically bad frames (blur, blown/
//! blocked exposure) and collapse near-duplicate bursts down to their
//! sharpest few, so the cloud ranking stage only ever sees a bounded,
//! technically-sound candidate pool. This stage never decides "best" — only
//! "not obviously bad" — that judgment call stays with stage 2.

use std::path::Path;

use rayon::prelude::*;

use crate::hash::{dhash, exposure_penalty, hamming, resize_gray, sharpness_variance};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PhotoScore {
    pub path: String,
    pub sharpness: f64,
    pub exposure_penalty: f64,
    pub technical_score: f64,
    pub phash: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct PrefilterConfig {
    /// Sharpness (Laplacian variance) floor below which a frame is rejected
    /// as out-of-focus/motion-blurred.
    pub min_sharpness: f64,
    /// Clipped-pixel fraction ceiling above which a frame is rejected as
    /// blown out or blocked up.
    pub max_exposure_penalty: f64,
    /// dHash Hamming-distance threshold under which two frames are treated
    /// as the same burst/near-duplicate.
    pub burst_hamming_threshold: u32,
    /// How many frames to keep per burst cluster — kept above 1 so stage 2
    /// still gets to pick "best in this burst" instead of this stage
    /// guessing that from sharpness/exposure alone.
    pub keep_per_burst: usize,
    /// Hard ceiling on survivors handed to the cloud stage, bounding its
    /// cost/latency regardless of how large the import was.
    pub max_survivors: usize,
}

impl Default for PrefilterConfig {
    fn default() -> Self {
        Self {
            min_sharpness: 40.0,
            max_exposure_penalty: 0.6,
            burst_hamming_threshold: 6,
            keep_per_burst: 3,
            max_survivors: 80,
        }
    }
}

const SCORE_EDGE: u32 = 480;

/// Score one photo's embedded camera preview. `None` (not an error) when the
/// preview can't be read/decoded — an unreadable frame is simply dropped
/// from the candidate pool rather than failing the whole cull run.
fn score_one(path: &str) -> Option<PhotoScore> {
    let preview = reveal_decode::extract_thumb_preview(Path::new(path)).ok()?;
    let img = image::load_from_memory(&preview.bytes).ok()?;
    let gray = resize_gray(&img.to_luma8(), SCORE_EDGE);
    let sharpness = sharpness_variance(&gray);
    let exposure = exposure_penalty(&gray);
    let phash = dhash(&gray);
    // Higher is better. Sharpness spans orders of magnitude across a mixed
    // batch, so it's log-compressed; exposure is a 0-1 fraction, weighted up
    // so a badly clipped frame can't out-rank a clean one on sharpness alone.
    let technical_score = sharpness.max(0.0).ln_1p() - exposure * 10.0;
    Some(PhotoScore {
        path: path.to_string(),
        sharpness,
        exposure_penalty: exposure,
        technical_score,
        phash,
    })
}

/// Caps how many photos the local pass decodes at once. Using the full
/// global rayon pool here would decode every candidate simultaneously —
/// fine on a local SSD, but on a NAS-mounted library it saturates the same
/// libraw/network path the grid's own on-screen thumbnails are using at the
/// same moment, and some of those lose the race and 404 (observed as
/// broken-image cells while a cull is running).
const MAX_CONCURRENT_DECODES: usize = 4;

/// Score every candidate in parallel (bounded, see `MAX_CONCURRENT_DECODES`),
/// reject technically bad frames, collapse near-duplicate bursts to their
/// sharpest few, and cap the survivor count. Returns survivors sorted
/// best-first.
pub fn prefilter(paths: &[String], cfg: &PrefilterConfig) -> Vec<PhotoScore> {
    let score_all = || {
        paths
            .par_iter()
            .filter_map(|p| score_one(p))
            .filter(|s| s.sharpness >= cfg.min_sharpness && s.exposure_penalty <= cfg.max_exposure_penalty)
            .collect()
    };
    let scored: Vec<PhotoScore> = match rayon::ThreadPoolBuilder::new()
        .num_threads(MAX_CONCURRENT_DECODES)
        .build()
    {
        Ok(pool) => pool.install(score_all),
        Err(_) => score_all(),
    };
    cluster_and_cap(scored, cfg)
}

/// Greedy burst clustering over technically-sound scores: sort best-first,
/// join a photo to the first existing cluster within the Hamming threshold
/// (capped at `keep_per_burst` members), or start a new cluster of its own.
/// Pulled out from `prefilter()` so the clustering logic itself is
/// unit-tested directly against hand-built scores, without needing real
/// RAW files on disk.
fn cluster_and_cap(mut scored: Vec<PhotoScore>, cfg: &PrefilterConfig) -> Vec<PhotoScore> {
    sort_desc(&mut scored);

    let mut clusters: Vec<Vec<PhotoScore>> = Vec::new();
    'outer: for photo in scored {
        for cluster in clusters.iter_mut() {
            if hamming(cluster[0].phash, photo.phash) <= cfg.burst_hamming_threshold {
                if cluster.len() < cfg.keep_per_burst {
                    cluster.push(photo);
                }
                continue 'outer;
            }
        }
        clusters.push(vec![photo]);
    }

    let mut survivors: Vec<PhotoScore> = clusters.into_iter().flatten().collect();
    sort_desc(&mut survivors);
    survivors.truncate(cfg.max_survivors);
    survivors
}

fn sort_desc(scores: &mut [PhotoScore]) {
    scores.sort_by(|a, b| {
        b.technical_score
            .partial_cmp(&a.technical_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(path: &str, sharpness: f64, exposure_penalty: f64, phash: u64) -> PhotoScore {
        PhotoScore {
            path: path.to_string(),
            sharpness,
            exposure_penalty,
            technical_score: sharpness.max(0.0).ln_1p() - exposure_penalty * 10.0,
            phash,
        }
    }

    #[test]
    fn burst_cluster_keeps_only_top_n_sharpest() {
        let cfg = PrefilterConfig { keep_per_burst: 2, burst_hamming_threshold: 4, ..Default::default() };
        // Five near-identical frames (same phash) at different sharpness.
        let burst = vec![
            score("a.raf", 100.0, 0.0, 0b0),
            score("b.raf", 300.0, 0.0, 0b0),
            score("c.raf", 50.0, 0.0, 0b0),
            score("d.raf", 200.0, 0.0, 0b0),
            score("e.raf", 10.0, 0.0, 0b0),
        ];
        let survivors = cluster_and_cap(burst, &cfg);
        assert_eq!(survivors.len(), 2, "only keep_per_burst frames should survive one burst");
        assert_eq!(survivors[0].path, "b.raf");
        assert_eq!(survivors[1].path, "d.raf");
    }

    #[test]
    fn distinct_scenes_are_not_collapsed_together() {
        let cfg = PrefilterConfig { burst_hamming_threshold: 4, ..Default::default() };
        let photos = vec![
            score("scene1.raf", 100.0, 0.0, 0x0000_0000_0000_0000),
            score("scene2.raf", 100.0, 0.0, 0xFFFF_FFFF_FFFF_FFFF),
        ];
        let survivors = cluster_and_cap(photos, &cfg);
        assert_eq!(survivors.len(), 2, "unrelated scenes must both survive, not cluster together");
    }

    #[test]
    fn survivor_count_is_capped() {
        let cfg = PrefilterConfig { max_survivors: 3, burst_hamming_threshold: 0, ..Default::default() };
        let photos: Vec<PhotoScore> = (0..10)
            .map(|i| score(&format!("{i}.raf"), 100.0 + i as f64, 0.0, i as u64))
            .collect();
        let survivors = cluster_and_cap(photos, &cfg);
        assert_eq!(survivors.len(), 3);
    }
}
