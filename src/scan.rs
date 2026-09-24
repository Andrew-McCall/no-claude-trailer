//! String matching primitives.
//!
//! This is the only module in the crate that compares strings, so every
//! matching rule in `no-claude-trailer` is built from the two functions here.
//! There is no regex engine: commit messages are small, the needles are known
//! at compile time, and a hand-written scan keeps the crate dependency free.

/// Returns true if `haystack` contains `needle`, ignoring ASCII case.
///
/// An empty needle never matches. Profiles and user config both feed this
/// function, and a needle that matched every line would strip whole messages.
pub fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    let hay = haystack.as_bytes();
    let pin = needle.as_bytes();
    if pin.is_empty() || pin.len() > hay.len() {
        return false;
    }
    let first = pin[0].to_ascii_lowercase();
    let last_start = hay.len() - pin.len();
    let mut at = 0;
    while at <= last_start {
        // Skip cheaply until the first byte could match, then compare the rest.
        if hay[at].to_ascii_lowercase() == first
            && hay[at..]
                .iter()
                .zip(pin)
                .all(|(h, p)| h.eq_ignore_ascii_case(p))
        {
            return true;
        }
        at += 1;
    }
    false
}

/// Splits a git trailer line into its key and value.
///
/// Returns `None` unless the line looks like `Key: value`, where the key is a
/// non-empty run of ASCII alphanumerics, `-` or `_`. Leading whitespace is
/// ignored and the value is trimmed, so `  Co-Authored-By:  Claude ` yields
/// `("Co-Authored-By", "Claude")`.
pub fn split_trailer(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_start();
    let colon = line.find(':')?;
    let key = &line[..colon];
    if key.is_empty() || !key.bytes().all(is_key_byte) {
        return None;
    }
    Some((key, line[colon + 1..].trim()))
}

/// The bytes git allows in a trailer key.
fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_ignore_case_matches_regardless_of_case() {
        assert!(contains_ignore_case("Co-Authored-By: CLAUDE", "claude"));
        assert!(contains_ignore_case(
            "generated with [Claude Code]",
            "[claude code]"
        ));
    }

    #[test]
    fn contains_ignore_case_matches_at_each_position() {
        assert!(contains_ignore_case("claude at the start", "claude"));
        assert!(contains_ignore_case("in the claude middle", "claude"));
        assert!(contains_ignore_case("at the end is claude", "claude"));
    }

    #[test]
    fn contains_ignore_case_rejects_absent_needle() {
        assert!(!contains_ignore_case("Co-Authored-By: Ada", "claude"));
    }

    #[test]
    fn contains_ignore_case_rejects_needle_longer_than_haystack() {
        assert!(!contains_ignore_case("cla", "claude"));
    }

    #[test]
    fn contains_ignore_case_never_matches_empty_needle() {
        assert!(!contains_ignore_case("any line at all", ""));
        assert!(!contains_ignore_case("", ""));
    }

    #[test]
    fn contains_ignore_case_tolerates_mixed_case_needles() {
        // Callers should pass lowercase needles, but user config cannot be
        // trusted to, so the comparison folds both sides.
        assert!(contains_ignore_case("co-authored-by: claude", "CLAUDE"));
    }

    #[test]
    fn contains_ignore_case_handles_multibyte_text() {
        assert!(contains_ignore_case("🤖 Generated with Claude", "claude"));
        assert!(!contains_ignore_case("日本語のメッセージ", "claude"));
    }

    #[test]
    fn split_trailer_splits_key_and_value() {
        assert_eq!(
            split_trailer("Co-Authored-By: Claude <noreply@anthropic.com>"),
            Some(("Co-Authored-By", "Claude <noreply@anthropic.com>"))
        );
    }

    #[test]
    fn split_trailer_trims_surrounding_whitespace() {
        assert_eq!(
            split_trailer("  Claude-Session:   https://claude.ai/code/x  "),
            Some(("Claude-Session", "https://claude.ai/code/x"))
        );
    }

    #[test]
    fn split_trailer_accepts_an_empty_value() {
        assert_eq!(
            split_trailer("Claude-Session:"),
            Some(("Claude-Session", ""))
        );
    }

    #[test]
    fn split_trailer_rejects_a_line_without_a_colon() {
        assert_eq!(split_trailer("Fix the pagination bug"), None);
    }

    #[test]
    fn split_trailer_rejects_a_key_containing_spaces() {
        // Otherwise a subject line like "Revert: the thing we did" would be
        // treated as a trailer whenever it happened to contain a colon.
        assert_eq!(split_trailer("Some words: value"), None);
    }

    #[test]
    fn split_trailer_rejects_an_empty_key() {
        assert_eq!(split_trailer(": value"), None);
        assert_eq!(split_trailer("   : value"), None);
    }
}
