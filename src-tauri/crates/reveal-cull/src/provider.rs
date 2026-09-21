//! Which cloud vision provider a call goes to — read once from prefs
//! (`ai_provider`, default `anthropic`) and used to pick the concrete
//! `VisionRanker`/`TagSuggester` impl. Kept as its own tiny module rather
//! than a bool so a third provider is a match arm, not a second flag.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiProvider {
    Anthropic,
    Gemini,
}

impl AiProvider {
    pub fn from_str(s: &str) -> Self {
        match s {
            "gemini" => AiProvider::Gemini,
            _ => AiProvider::Anthropic,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AiProvider::Anthropic => "anthropic",
            AiProvider::Gemini => "gemini",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            AiProvider::Anthropic => "claude-sonnet-5",
            AiProvider::Gemini => "gemini-2.5-flash",
        }
    }
}
