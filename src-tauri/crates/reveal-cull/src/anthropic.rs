//! The default `VisionRanker`: Anthropic's Messages API, multiple images per
//! request, forced tool-use so the response is a strict JSON array instead
//! of parsed prose.

use base64::Engine as _;
use serde_json::json;

use crate::rank::{RankCandidate, RankedResult, TagSuggester, VisionRanker};
use crate::CullError;

pub struct AnthropicRanker {
    api_key: String,
    model: String,
}

impl AnthropicRanker {
    pub fn new(api_key: String) -> Self {
        Self::with_model(api_key, "claude-sonnet-5".to_string())
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

const PROMPT: &str = "You are helping a photographer cull one shoot down to their best frames. \
Score each image from 0 to 10 on overall photographic quality: composition, expression/moment, and \
framing matter most; use sharpness/focus only as a tie-breaker between otherwise-similar shots. Call \
the score_photos tool with exactly one entry per image, using the image's position in this message as \
its index (0 for the first image, 1 for the second, and so on).";

impl VisionRanker for AnthropicRanker {
    fn rank_batch(&self, candidates: &[RankCandidate]) -> Result<Vec<RankedResult>, CullError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        if self.api_key.is_empty() {
            return Err(CullError::MissingApiKey);
        }

        let mut content = vec![json!({ "type": "text", "text": PROMPT })];
        for c in candidates {
            content.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/jpeg",
                    "data": base64::engine::general_purpose::STANDARD.encode(&c.jpeg_bytes),
                }
            }));
        }

        let tool = json!({
            "name": "score_photos",
            "description": "Report a 0-10 quality score for each image, in input order.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "scores": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "index": { "type": "integer" },
                                "score": { "type": "number" }
                            },
                            "required": ["index", "score"]
                        }
                    }
                },
                "required": ["scores"]
            }
        });

        let body = json!({
            "model": self.model,
            "max_tokens": 1024,
            "tools": [tool],
            "tool_choice": { "type": "tool", "name": "score_photos" },
            "messages": [{ "role": "user", "content": content }],
        });

        // ureq has no default timeout — without one, a stalled connection
        // (dropped packet with no reset, a proxy hiccup) hangs the request
        // forever with no error and no CPU use, indistinguishable from the
        // cull just being slow. Bounded generously: a 20-image forced
        // tool-use request can legitimately take tens of seconds.
        let response: serde_json::Value = ureq::post("https://api.anthropic.com/v1/messages")
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .timeout(std::time::Duration::from_secs(90))
            .send_json(body)
            .map_err(|e| CullError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| CullError::Http(e.to_string()))?;

        Ok(parse_scores(&response, candidates))
    }
}

const TAGS_PROMPT: &str = "Suggest 3 to 8 short keyword tags for this photo, the kind a photographer \
would use to find it later in a library search: subject, setting, mood, notable technique. Lowercase, \
one or two words each, no hashtags or punctuation. Call the suggest_tags tool with the list.";

impl TagSuggester for AnthropicRanker {
    fn suggest_tags(&self, jpeg_bytes: &[u8]) -> Result<Vec<String>, CullError> {
        if self.api_key.is_empty() {
            return Err(CullError::MissingApiKey);
        }

        let content = vec![
            json!({ "type": "text", "text": TAGS_PROMPT }),
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/jpeg",
                    "data": base64::engine::general_purpose::STANDARD.encode(jpeg_bytes),
                }
            }),
        ];

        let tool = json!({
            "name": "suggest_tags",
            "description": "Report the suggested keyword tags for this photo.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "tags": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["tags"]
            }
        });

        let body = json!({
            "model": self.model,
            "max_tokens": 256,
            "tools": [tool],
            "tool_choice": { "type": "tool", "name": "suggest_tags" },
            "messages": [{ "role": "user", "content": content }],
        });

        let response: serde_json::Value = ureq::post("https://api.anthropic.com/v1/messages")
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .timeout(std::time::Duration::from_secs(30))
            .send_json(body)
            .map_err(|e| CullError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| CullError::Http(e.to_string()))?;

        Ok(response
            .get("content")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
            .find(|block| block.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("tags"))
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect())
    }
}

fn parse_scores(response: &serde_json::Value, candidates: &[RankCandidate]) -> Vec<RankedResult> {
    let scores = response
        .get("content")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .find(|block| block.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        .and_then(|block| block.get("input"))
        .and_then(|input| input.get("scores"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    let mut results = Vec::with_capacity(candidates.len());
    for entry in scores {
        let Some(index) = entry.get("index").and_then(|v| v.as_u64()) else { continue };
        let Some(score) = entry.get("score").and_then(|v| v.as_f64()) else { continue };
        if let Some(candidate) = candidates.get(index as usize) {
            results.push(RankedResult { path: candidate.path.clone(), score });
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(path: &str) -> RankCandidate {
        RankCandidate { path: path.to_string(), jpeg_bytes: vec![0xFF, 0xD8] }
    }

    #[test]
    fn parses_a_well_formed_tool_use_response() {
        let candidates = vec![candidate("a.raf"), candidate("b.raf")];
        let response = json!({
            "content": [
                { "type": "text", "text": "..." },
                {
                    "type": "tool_use",
                    "name": "score_photos",
                    "input": { "scores": [ { "index": 0, "score": 7.5 }, { "index": 1, "score": 9.0 } ] }
                }
            ]
        });
        let results = parse_scores(&response, &candidates);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "a.raf");
        assert_eq!(results[0].score, 7.5);
        assert_eq!(results[1].path, "b.raf");
        assert_eq!(results[1].score, 9.0);
    }

    #[test]
    fn out_of_range_index_is_dropped_not_panicking() {
        let candidates = vec![candidate("a.raf")];
        let response = json!({
            "content": [
                { "type": "tool_use", "name": "score_photos", "input": { "scores": [ { "index": 5, "score": 3.0 } ] } }
            ]
        });
        assert!(parse_scores(&response, &candidates).is_empty());
    }

    #[test]
    fn missing_tool_use_block_returns_empty() {
        let candidates = vec![candidate("a.raf")];
        let response = json!({ "content": [ { "type": "text", "text": "sorry, I can't help with that" } ] });
        assert!(parse_scores(&response, &candidates).is_empty());
    }
}
