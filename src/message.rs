//! Removing attribution from a commit message.
//!
//! Four rules keep this honest, and each one is a bug if it goes missing:
//!
//! 1. **The subject line is never removed.** A commit titled
//!    `Add Co-Authored-By parsing` has to survive.
//! 2. **Nothing at or after the scissors line is touched.** With
//!    `commit.verbose` the message file holds the whole diff, which can
//!    legitimately contain the text of a trailer.
//! 3. **Blank lines our removals created are collapsed**, so a stripped
//!    message does not end in a gap. Blank runs we did not create are left
//!    exactly as the author wrote them.
//! 4. **An attribution-only message is an error**, never an empty commit.

use crate::profiles::{Hit, Rules};

/// How to read the message: as a hook's message file, or as a commit object.
#[derive(Debug, Clone)]
pub struct Options {
    /// Prefix marking comment lines, which are ignored along with everything
    /// from the scissors line onward. `None` for commit objects, which have no
    /// comments — there, a line starting with `#` is ordinary text.
    pub comment_prefix: Option<String>,
}

impl Options {
    /// For a raw commit object read out of the object database.
    pub fn commit_object() -> Self {
        Self {
            comment_prefix: None,
        }
    }

    /// For a `commit-msg` hook's message file, using git's comment prefix.
    pub fn message_file(comment_prefix: &str) -> Self {
        Self {
            comment_prefix: Some(comment_prefix.to_string()),
        }
    }
}

/// The outcome of stripping one message.
#[derive(Debug)]
pub struct Strip {
    /// The message with attribution removed.
    pub message: String,
    /// What was removed, in the order it appeared.
    pub hits: Vec<Hit>,
}

impl Strip {
    /// Whether anything was actually removed.
    pub fn changed(&self) -> bool {
        !self.hits.is_empty()
    }
}

/// Stripping refused to produce a message.
#[derive(Debug, PartialEq, Eq)]
pub enum StripError {
    /// Removing attribution would leave nothing but whitespace. Carries what
    /// was found, so a caller that only reports can still say what it saw.
    NothingLeft(Vec<Hit>),
}

/// Removes every line matching `rules`, then tidies what the removals left.
pub fn strip(message: &str, rules: &Rules, options: &Options) -> Result<Strip, StripError> {
    if message.is_empty() {
        return Ok(Strip {
            message: String::new(),
            hits: Vec::new(),
        });
    }

    let terminator = if message.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let trailing = message.ends_with(terminator);
    let body = if trailing {
        &message[..message.len() - terminator.len()]
    } else {
        message
    };
    let lines: Vec<&str> = body.split(terminator).collect();

    let editable = editable_end(&lines, options);
    let (removed, hits) = find_removals(&lines, editable, rules, options);

    let mut kept = keep_lines(&lines, editable, &removed);
    if !hits.is_empty() {
        // Our own removals must not leave the message ending in blank lines.
        while kept.last().is_some_and(|line| line.trim().is_empty()) {
            kept.pop();
        }
    }
    let tail_is_empty = editable == lines.len();
    if !hits.is_empty() && kept.is_empty() && tail_is_empty {
        return Err(StripError::NothingLeft(hits));
    }
    kept.extend_from_slice(&lines[editable..]);

    let mut out = kept.join(terminator);
    if trailing && !out.is_empty() {
        out.push_str(terminator);
    }
    Ok(Strip { message: out, hits })
}

/// The index where the editable region ends: the scissors line, or the end.
fn editable_end(lines: &[&str], options: &Options) -> usize {
    let Some(prefix) = &options.comment_prefix else {
        return lines.len();
    };
    // The `>8` marker survives translation of the surrounding text, so it is a
    // sturdier signal than matching git's English wording.
    lines
        .iter()
        .position(|line| line.starts_with(prefix.as_str()) && line.contains(">8"))
        .unwrap_or(lines.len())
}

/// Marks which lines to remove and records why, skipping the subject and comments.
fn find_removals(
    lines: &[&str],
    editable: usize,
    rules: &Rules,
    options: &Options,
) -> (Vec<bool>, Vec<Hit>) {
    let mut removed = vec![false; lines.len()];
    let mut hits = Vec::new();
    // Start at 1: the subject line is never a candidate.
    for index in 1..editable {
        let line = lines[index];
        if let Some(prefix) = &options.comment_prefix
            && line.starts_with(prefix.as_str())
        {
            continue;
        }
        if let Some(hit) = rules.match_line(line) {
            removed[index] = true;
            hits.push(hit);
        }
    }
    (removed, hits)
}

/// Copies the kept lines, dropping only the blank lines our removals orphaned.
fn keep_lines<'a>(lines: &[&'a str], editable: usize, removed: &[bool]) -> Vec<&'a str> {
    let mut kept: Vec<&str> = Vec::with_capacity(editable);
    let mut after_removal = false;
    for index in 0..editable {
        if removed[index] {
            after_removal = true;
            continue;
        }
        let blank = lines[index].trim().is_empty();
        let would_double = kept.last().is_none_or(|line| line.trim().is_empty());
        if blank && after_removal && would_double {
            continue;
        }
        if !blank {
            after_removal = false;
        }
        kept.push(lines[index]);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude() -> Rules {
        Rules::from_names(&["claude"]).unwrap()
    }

    fn strip_file(message: &str) -> Strip {
        strip(message, &claude(), &Options::message_file("#")).expect("should strip")
    }

    fn strip_object(message: &str) -> Strip {
        strip(message, &claude(), &Options::commit_object()).expect("should strip")
    }

    #[test]
    fn removes_a_coauthor_trailer() {
        let out =
            strip_object("Fix pagination\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n");
        assert_eq!(out.message, "Fix pagination\n");
        assert!(out.changed());
    }

    #[test]
    fn keeps_a_message_with_no_attribution_byte_for_byte() {
        let original = "Fix pagination\n\nThe cursor was off by one.\n\nCo-Authored-By: Ada <ada@example.com>\n";
        let out = strip_object(original);
        assert_eq!(out.message, original);
        assert!(!out.changed());
    }

    #[test]
    fn never_removes_the_subject_line() {
        let out = strip_object("Add Co-Authored-By: Claude parsing\n\nReal body.\n");
        assert_eq!(
            out.message,
            "Add Co-Authored-By: Claude parsing\n\nReal body.\n"
        );
        assert!(!out.changed());
    }

    #[test]
    fn removes_the_full_claude_code_footer() {
        let out = strip_object(concat!(
            "Fix pagination\n",
            "\n",
            "The cursor was off by one.\n",
            "\n",
            "🤖 Generated with [Claude Code](https://claude.com/claude-code)\n",
            "\n",
            "https://claude.ai/code/session_011cHRTmeNC5X7\n",
            "\n",
            "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n",
            "Claude-Session: https://claude.ai/code/session_011cHRTmeNC5X7\n",
        ));
        assert_eq!(
            out.message,
            "Fix pagination\n\nThe cursor was off by one.\n"
        );
        assert_eq!(out.hits.len(), 4);
    }

    #[test]
    fn keeps_human_trailers_beside_removed_ones() {
        let out = strip_object(concat!(
            "Fix pagination\n",
            "\n",
            "Co-Authored-By: Claude <noreply@anthropic.com>\n",
            "Reviewed-by: Ada <ada@example.com>\n",
            "Co-Authored-By: Grace <grace@example.com>\n",
        ));
        assert_eq!(
            out.message,
            "Fix pagination\n\nReviewed-by: Ada <ada@example.com>\nCo-Authored-By: Grace <grace@example.com>\n"
        );
    }

    #[test]
    fn leaves_blank_runs_it_did_not_create() {
        let original = "Subject\n\n\n\nBody with deliberate gaps.\n";
        assert_eq!(strip_object(original).message, original);
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let out = strip_object("Fix pagination\r\n\r\nCo-Authored-By: Claude <x@y>\r\n");
        assert_eq!(out.message, "Fix pagination\r\n");
    }

    #[test]
    fn preserves_a_missing_trailing_newline() {
        let out = strip_object("Fix pagination\n\nCo-Authored-By: Claude <x@y>");
        assert_eq!(out.message, "Fix pagination");
    }

    #[test]
    fn ignores_everything_after_the_scissors_line() {
        // `commit.verbose` puts the diff in the message file, and the diff can
        // contain the very text we strip.
        let message = concat!(
            "Fix pagination\n",
            "\n",
            "Co-Authored-By: Claude <noreply@anthropic.com>\n",
            "# ------------------------ >8 ------------------------\n",
            "# Do not modify or remove the line above.\n",
            "diff --git a/README.md b/README.md\n",
            "+Co-Authored-By: Claude <noreply@anthropic.com>\n",
        );
        let out = strip_file(message);
        assert_eq!(
            out.message,
            concat!(
                "Fix pagination\n",
                "# ------------------------ >8 ------------------------\n",
                "# Do not modify or remove the line above.\n",
                "diff --git a/README.md b/README.md\n",
                "+Co-Authored-By: Claude <noreply@anthropic.com>\n",
            )
        );
        assert_eq!(out.hits.len(), 1);
    }

    #[test]
    fn ignores_comment_lines_in_a_message_file() {
        let message =
            "Fix pagination\n# Generated with [Claude Code](https://claude.com/claude-code)\n";
        let out = strip_file(message);
        assert_eq!(out.message, message);
        assert!(!out.changed());
    }

    #[test]
    fn honours_a_custom_comment_prefix() {
        let message =
            "Fix pagination\n; Generated with [Claude Code](https://claude.com/claude-code)\n";
        let out = strip(message, &claude(), &Options::message_file(";")).unwrap();
        assert_eq!(out.message, message);
    }

    #[test]
    fn strips_hash_prefixed_lines_in_a_commit_object() {
        // A commit object has no comments, so `#` is ordinary text.
        let out = strip_object(
            "Fix pagination\n\n# Generated with [Claude Code](https://claude.com/claude-code)\n",
        );
        assert_eq!(out.message, "Fix pagination\n");
        assert!(out.changed());
    }

    #[test]
    fn refuses_an_attribution_only_message() {
        let result = strip(
            "Co-Authored-By: Claude <noreply@anthropic.com>\n",
            &claude(),
            &Options::commit_object(),
        );
        // The subject is protected, so this specific message survives whole.
        assert_eq!(
            result.unwrap().message,
            "Co-Authored-By: Claude <noreply@anthropic.com>\n"
        );
    }

    #[test]
    fn refuses_a_message_whose_body_was_all_attribution_and_subject_blank() {
        let result = strip(
            "\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n",
            &claude(),
            &Options::commit_object(),
        );
        assert!(matches!(result.unwrap_err(), StripError::NothingLeft(hits) if hits.len() == 1));
    }

    #[test]
    fn reports_hits_in_order_with_their_agent() {
        let out = strip_object(concat!(
            "Fix pagination\n",
            "\n",
            "🤖 Generated with [Claude Code](https://claude.com/claude-code)\n",
            "Co-Authored-By: Claude <noreply@anthropic.com>\n",
        ));
        let labels: Vec<&str> = out.hits.iter().map(|hit| hit.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["Generated with [Claude Code]", "Co-Authored-By"]
        );
        assert!(out.hits.iter().all(|hit| hit.agent == "claude"));
    }

    #[test]
    fn handles_an_empty_message() {
        assert_eq!(strip_object("").message, "");
    }

    #[test]
    fn collapses_only_the_gap_left_behind() {
        let out = strip_object("Subject\n\nBody.\n\nCo-Authored-By: Claude <x@y>\n\nMore body.\n");
        assert_eq!(out.message, "Subject\n\nBody.\n\nMore body.\n");
    }
}
