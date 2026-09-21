//! Stage 2 — the cloud pass: hand the stage-1 survivors to a vision model
//! and ask it which are actually the best shots. `VisionRanker` is the seam
//! that keeps the concrete HTTP provider swappable without touching the
//! prefilter or the orchestration in the Tauri command.

use crate::CullError;

pub struct RankCandidate {
    pub path: String,
    /// JPEG bytes — the same small embedded-preview thumbnail stage 1
    /// already decoded, so this stage never re-decodes a RAW or spends GPU
    /// time, and keeps the bytes sent over the wire small.
    pub jpeg_bytes: Vec<u8>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RankedResult {
    pub path: String,
    pub score: f64,
}

pub trait VisionRanker {
    /// Score one batch of images (already sized to fit one request) and
    /// return a result per input image, in any order.
    fn rank_batch(&self, candidates: &[RankCandidate]) -> Result<Vec<RankedResult>, CullError>;
}

/// One-off keyword-tag suggestion for a single photo. Separate from
/// `VisionRanker` (different shape: one image in, a string list out, not a
/// per-image score) but the same "provider swappable behind a trait" idea —
/// each provider module implements both on its own client struct.
pub trait TagSuggester {
    fn suggest_tags(&self, jpeg_bytes: &[u8]) -> Result<Vec<String>, CullError>;
}

/// Chunk `candidates` into request-sized batches, rank each batch, merge and
/// sort globally, and return the best `target`. The provider only ever sees
/// one batch at a time — this is a per-batch score, not a precise
/// cross-batch leaderboard — which is the right tradeoff for "cut ~80 down
/// to 24", not a photo contest.
pub fn rank_all(
    ranker: &dyn VisionRanker,
    candidates: Vec<RankCandidate>,
    batch_size: usize,
    target: usize,
) -> Result<Vec<RankedResult>, CullError> {
    let mut all_results: Vec<RankedResult> = Vec::with_capacity(candidates.len());
    for batch in candidates.chunks(batch_size.max(1)) {
        all_results.extend(ranker.rank_batch(batch)?);
    }
    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_results.truncate(target);
    Ok(all_results)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake ranker that scores every image by its byte length — enough to
    /// exercise the batching/merge/truncate logic without any network call.
    struct FakeRanker;
    impl VisionRanker for FakeRanker {
        fn rank_batch(&self, candidates: &[RankCandidate]) -> Result<Vec<RankedResult>, CullError> {
            Ok(candidates
                .iter()
                .map(|c| RankedResult { path: c.path.clone(), score: c.jpeg_bytes.len() as f64 })
                .collect())
        }
    }

    fn candidate(path: &str, size: usize) -> RankCandidate {
        RankCandidate { path: path.to_string(), jpeg_bytes: vec![0u8; size] }
    }

    #[test]
    fn merges_across_batches_and_returns_global_top_n() {
        let candidates = vec![
            candidate("a", 10),
            candidate("b", 90),
            candidate("c", 50),
            candidate("d", 20),
            candidate("e", 70),
        ];
        // batch_size of 2 forces 3 separate rank_batch calls, so this also
        // proves the merge step compares scores ACROSS batches, not just
        // within one.
        let top = rank_all(&FakeRanker, candidates, 2, 3).unwrap();
        assert_eq!(top.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), vec!["b", "e", "c"]);
    }

    #[test]
    fn target_larger_than_candidates_returns_everything() {
        let candidates = vec![candidate("a", 1), candidate("b", 2)];
        let top = rank_all(&FakeRanker, candidates, 10, 24).unwrap();
        assert_eq!(top.len(), 2);
    }

    #[test]
    fn empty_candidates_returns_empty() {
        let top = rank_all(&FakeRanker, Vec::new(), 20, 24).unwrap();
        assert!(top.is_empty());
    }
}
