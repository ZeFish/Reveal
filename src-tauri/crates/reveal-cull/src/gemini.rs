//! Google Gemini implementation of the same `VisionRanker`/`TagSuggester`
//! seams `anthropic.rs` fills — a second concrete provider, picked by
//! `AiProvider` from prefs. Uses Gemini's function-calling ("tools") the
//! same way the Anthropic client forces tool-use: a single declared
//! function, `mode: "ANY"`, so the response is structured JSON instead of
//! prose to parse.

use base64::Engine as _;
use serde_json::json;

use crate::rank::{RankCandidate, RankedResult, TagSuggester, VisionRanker};
use crate::CullError;

pub struct GeminiRanker {
    api_key: String,
    model: String,
}

impl GeminiRanker {
    pub fn new(api_key: String) -> Self {
        Self::with_model(api_key, "gemini-2.5-flash".to_string())
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }

    fn endpoint(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        )
    }
}

const RANK_PROMPT: &str = "You are helping a photographer cull one shoot down to their best frames. \
Score each image from 0 to 10 on overall photographic quality: composition, expression/moment, and \
framing matter most; use sharpness/focus only as a tie-breaker between otherwise-similar shots. Call \
the score_photos function with exactly one entry per image, using the image's position in this message \
as its index (0 for the first image, 1 for the second, and so on).";

const TAGS_PROMPT: &str = "Suggest 3 to 8 short keyword tags for this photo, the kind a photographer \
would use to find it later in a library search: subject, setting, mood, notable technique. Lowercase, \
one or two words each, no hashtags or punctuation. Call the suggest_tags function with the list.";

fn image_part(jpeg_bytes: &[u8]) -> serde_json::Value {
    json!({
        "inline_data": {
            "mime_type": "image/jpeg",
            "data": base64::engine::general_purpose::STANDARD.encode(jpeg_bytes),
        }
    })
}

/// Pulls the first `functionCall.args` out of a Gemini `generateContent`
/// response — the JSON shape forced-tool-use ends up in regardless of which
/// function was declared.
fn function_call_args(response: &serde_json::Value) -> Option<&serde_json::Value> {
    response
        .get("candidates")?
        .as_array()?
        .first()?
        .get("content")?
        .get("parts")?
        .as_array()?
        .iter()
        .find_map(|part| part.get("functionCall"))?
        .get("args")
}

impl VisionRanker for GeminiRanker {
    fn rank_batch(&self, candidates: &[RankCandidate]) -> Result<Vec<RankedResult>, CullError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        if self.api_key.is_empty() {
            return Err(CullError::MissingApiKey);
        }

        let mut parts = vec![json!({ "text": RANK_PROMPT })];
        parts.extend(candidates.iter().map(|c| image_part(&c.jpeg_bytes)));

        let function = json!({
            "name": "score_photos",
            "description": "Report a 0-10 quality score for each image, in input order.",
            "parameters": {
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
            "contents": [{ "parts": parts }],
            "tools": [{ "function_declarations": [function] }],
            "tool_config": {
                "function_calling_config": { "mode": "ANY", "allowed_function_names": ["score_photos"] }
            },
        });

        let response: serde_json::Value = ureq::post(&self.endpoint())
            .set("content-type", "application/json")
            .timeout(std::time::Duration::from_secs(90))
            .send_json(body)
            .map_err(|e| CullError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| CullError::Http(e.to_string()))?;

        let scores = function_call_args(&response)
            .and_then(|a| a.get("scores"))
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
        Ok(results)
    }
}

impl TagSuggester for GeminiRanker {
    fn suggest_tags(&self, jpeg_bytes: &[u8]) -> Result<Vec<String>, CullError> {
        if self.api_key.is_empty() {
            return Err(CullError::MissingApiKey);
        }

        let parts = vec![json!({ "text": TAGS_PROMPT }), image_part(jpeg_bytes)];

        let function = json!({
            "name": "suggest_tags",
            "description": "Report the suggested keyword tags for this photo.",
            "parameters": {
                "type": "object",
                "properties": {
                    "tags": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["tags"]
            }
        });

        let body = json!({
            "contents": [{ "parts": parts }],
            "tools": [{ "function_declarations": [function] }],
            "tool_config": {
                "function_calling_config": { "mode": "ANY", "allowed_function_names": ["suggest_tags"] }
            },
        });

        let response: serde_json::Value = ureq::post(&self.endpoint())
            .set("content-type", "application/json")
            .timeout(std::time::Duration::from_secs(30))
            .send_json(body)
            .map_err(|e| CullError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| CullError::Http(e.to_string()))?;

        Ok(function_call_args(&response)
            .and_then(|a| a.get("tags"))
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect())
    }
}
