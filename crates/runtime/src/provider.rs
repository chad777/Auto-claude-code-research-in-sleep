//! Provider family classification.
//!
//! [`ProviderFamily`] is a PURE CLASSIFIER over the `executor_provider`
//! string. It does NOT read or write env, does NOT pick an endpoint, and is
//! NOT wired into any routing, pricing, or env-writing decision. It exists so
//! the three executor families can be named by a type; the actual dispatch
//! (P8, a later phase) will consume it, at which point its mapping must stay an
//! exact mirror of the string comparisons that drive routing today.
//!
//! ## Mapping (exact-match — no trim, no lowercase)
//!
//! The `executor_provider` config field only ever serializes to one of a small
//! set of literals (see `aris-cli/src/config.rs`): `"anthropic"`,
//! `"anthropic-compat"`, `"openai"`, `"custom"`. Every OpenAI-compatible menu
//! provider (OpenAI / Gemini / GLM / MiniMax / Kimi / Xiaomi / Qwen / Doubao)
//! collapses to the literal `"openai"` at the config layer, and only `"openai"`
//! / `"custom"` cause `apply_to_env` to set `EXECUTOR_PROVIDER=openai` (the
//! exact value `resolve_openai_executor_config` matches on). This classifier
//! mirrors that:
//!
//! | `executor_provider` | family |
//! |---|---|
//! | `"anthropic"` | [`ProviderFamily::AnthropicNative`] |
//! | `"anthropic-compat"` | [`ProviderFamily::AnthropicCompat`] |
//! | `"openai"`, `"custom"` | [`ProviderFamily::OpenAiCompat`] |
//! | anything else | [`ProviderFamily::Unknown`] |
//!
//! `Unknown` is deliberate: an unrecognized string triggers no
//! `EXECUTOR_PROVIDER=openai` write today, so at runtime it falls through to
//! the Anthropic path. Reporting it as `Unknown` (rather than guessing
//! `OpenAiCompat`) keeps a future P8 dispatch from misclassifying an unknown
//! string into the OpenAI branch and silently regressing.

// Provider brand names (OpenAI, MiniMax, GLM, …) appear in the prose docs;
// they are not Rust items, so silence the doc-backtick lint for this module.
#![allow(clippy::doc_markdown)]

/// The three executor families ARIS routes to, plus an explicit `Unknown` for
/// strings that match none of them. See the module docs for the mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderFamily {
    /// Anthropic native API (Messages endpoint, `x-api-key` / OAuth).
    AnthropicNative,
    /// Anthropic-compatible third party (e.g. DeepSeek's `/anthropic`
    /// endpoint, Bearer auth via `ANTHROPIC_AUTH_TOKEN`).
    AnthropicCompat,
    /// OpenAI-compatible `/v1/chat/completions` (OpenAI / Gemini / GLM /
    /// MiniMax / Kimi / Xiaomi / Qwen / Doubao / custom).
    OpenAiCompat,
    /// Unrecognized `executor_provider` string. Triggers no known routing
    /// today (falls through to the Anthropic path at runtime).
    Unknown,
}

impl ProviderFamily {
    /// Classify an `executor_provider` config string. Exact-match only — the
    /// input is compared verbatim, mirroring the string comparisons that drive
    /// routing today (no trimming, no case folding).
    #[must_use]
    pub fn from_executor_provider(s: &str) -> ProviderFamily {
        match s {
            "anthropic" => ProviderFamily::AnthropicNative,
            "anthropic-compat" => ProviderFamily::AnthropicCompat,
            "openai" | "custom" => ProviderFamily::OpenAiCompat,
            _ => ProviderFamily::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderFamily;

    #[test]
    fn classifies_anthropic_native() {
        assert_eq!(
            ProviderFamily::from_executor_provider("anthropic"),
            ProviderFamily::AnthropicNative
        );
    }

    #[test]
    fn classifies_anthropic_compat() {
        assert_eq!(
            ProviderFamily::from_executor_provider("anthropic-compat"),
            ProviderFamily::AnthropicCompat
        );
    }

    #[test]
    fn classifies_openai_and_custom_as_openai_compat() {
        assert_eq!(
            ProviderFamily::from_executor_provider("openai"),
            ProviderFamily::OpenAiCompat
        );
        // `custom` is the value that, with `openai`, drives
        // `apply_to_env` to set EXECUTOR_PROVIDER=openai.
        assert_eq!(
            ProviderFamily::from_executor_provider("custom"),
            ProviderFamily::OpenAiCompat
        );
    }

    #[test]
    fn menu_providers_collapse_to_openai_string() {
        // Every OpenAI-compatible menu provider serializes to the literal
        // "openai" at the config layer, so the classifier only ever sees
        // "openai" for these — pinned here so a future config refactor that
        // leaks a raw provider name (e.g. "glm") is caught as Unknown, not
        // silently misrouted.
        for raw in ["glm", "minimax-openai", "kimi", "gemini", "doubao", "qwen", "mimo"] {
            assert_eq!(
                ProviderFamily::from_executor_provider(raw),
                ProviderFamily::Unknown,
                "raw menu name `{raw}` is NOT a serialized executor_provider value; \
                 config collapses these to \"openai\" — seeing the raw name means Unknown"
            );
        }
    }

    #[test]
    fn unknown_string_is_unknown() {
        assert_eq!(
            ProviderFamily::from_executor_provider("totally-made-up"),
            ProviderFamily::Unknown
        );
        assert_eq!(
            ProviderFamily::from_executor_provider(""),
            ProviderFamily::Unknown
        );
    }

    #[test]
    fn classification_is_exact_match_no_trim_no_case_fold() {
        // Whitespace and case variants are NOT normalized — mirrors the
        // verbatim string comparisons in the routing code.
        assert_eq!(
            ProviderFamily::from_executor_provider(" anthropic"),
            ProviderFamily::Unknown
        );
        assert_eq!(
            ProviderFamily::from_executor_provider("anthropic "),
            ProviderFamily::Unknown
        );
        assert_eq!(
            ProviderFamily::from_executor_provider("Anthropic"),
            ProviderFamily::Unknown
        );
        assert_eq!(
            ProviderFamily::from_executor_provider("OPENAI"),
            ProviderFamily::Unknown
        );
    }
}
