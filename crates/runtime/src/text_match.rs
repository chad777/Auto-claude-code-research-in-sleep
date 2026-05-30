//! Shared text-matching helpers used across crates.
//!
//! [`word_match`] is the single canonical word-boundary matcher. It was
//! previously duplicated verbatim three times — `openai_executor::word_match`,
//! `runtime::usage::has_word`, and `tools::reviewer_word_match` — all of which
//! now forward here so executor capability detection, reviewer routing, and the
//! pricing table share one definition. The boundary set (`-`, `_`, `/`, `:` plus
//! start/end of string) is load-bearing: it is what keeps `o3` from matching
//! `gpt-5.4-nano` or a mid-word `o32-mini`. Do not widen it.
//!
//! Note: `usage::provider_match` is intentionally NOT consolidated here — it has
//! different (more permissive, `starts_with` + `/:`-segment) semantics.

/// Word-boundary match. Returns `true` iff `needle` occurs in `haystack`
/// delimited on both sides by a word boundary, where a boundary is one of
/// `-`, `_`, `/`, `:` or the start/end of the string.
///
/// Empty `needle` never matches; a `needle` longer than `haystack` never
/// matches.
#[must_use]
pub fn word_match(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let nbytes = needle.as_bytes();
    if nbytes.is_empty() || bytes.len() < nbytes.len() {
        return false;
    }
    let is_boundary = |b: u8| matches!(b, b'-' | b'_' | b'/' | b':');
    let mut i = 0;
    while i + nbytes.len() <= bytes.len() {
        if &bytes[i..i + nbytes.len()] == nbytes {
            let before_ok = i == 0 || is_boundary(bytes[i - 1]);
            let after_idx = i + nbytes.len();
            let after_ok = after_idx == bytes.len() || is_boundary(bytes[after_idx]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::word_match;

    /// Canonical boundary semantics — start/end + each boundary char, plus the
    /// rejections (digit / letter neighbour, mid-word, empty needle). This
    /// mirrors the three former call-site tests so the canonical function is
    /// pinned directly.
    #[test]
    fn word_match_boundary_semantics() {
        // whole-string + each boundary char as leading delimiter
        assert!(word_match("o3", "o3"), "bare needle == whole string");
        assert!(word_match("a-o3", "o3"), "leading `-` is a boundary");
        assert!(word_match("a_o3", "o3"), "leading `_` is a boundary");
        assert!(word_match("a/o3", "o3"), "leading `/` is a boundary");
        assert!(word_match("a:o3", "o3"), "leading `:` is a boundary");
        // each boundary char as trailing delimiter
        assert!(word_match("o3-x", "o3"), "trailing `-` is a boundary");
        assert!(word_match("o3_x", "o3"), "trailing `_` is a boundary");
        assert!(word_match("o3/x", "o3"), "trailing `/` is a boundary");
        assert!(word_match("o3:x", "o3"), "trailing `:` is a boundary");
        // rejections
        assert!(!word_match("o32", "o3"), "trailing digit is not a boundary");
        assert!(!word_match("xo3", "o3"), "leading letter is not a boundary");
        assert!(!word_match("o3x", "o3"), "trailing letter is not a boundary");
        assert!(!word_match("o3.mini", "o3"), "`.` is not a boundary");
        assert!(!word_match("o3 mini", "o3"), "space is not a boundary");
        // needle longer than haystack / empty needle
        assert!(!word_match("o3", "o3-mini"), "needle longer than haystack");
        assert!(!word_match("o3", ""), "empty needle never matches");
        assert!(!word_match("anything", ""), "empty needle never matches");
    }
}
