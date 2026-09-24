//! Describing commits to a person.
//!
//! Both `check` and `clean` list commits with what was found in them, so the
//! listing lives here and neither command grows its own.

use crate::commit::Commit;
use crate::message::{self, Options, Strip, StripError};
use crate::profiles::{Hit, Rules};
use crate::ui::Style;

/// Longest subject shown before it is shortened.
const SUBJECT_WIDTH: usize = 48;

/// One commit that carries attribution.
#[derive(Debug)]
pub struct Finding {
    /// Full object id.
    pub id: String,
    /// The commit's subject line.
    pub subject: String,
    /// What was found in it.
    pub hits: Vec<Hit>,
    /// The cleaned message, or `None` when stripping would empty it.
    pub cleaned: Option<String>,
}

impl Finding {
    /// Examines one commit, returning `None` when it is already clean.
    pub fn examine(id: &str, commit: &Commit, rules: &Rules) -> Self {
        let subject = commit
            .message
            .lines()
            .next()
            .unwrap_or_default()
            .to_string();
        match message::strip(&commit.message, rules, &Options::commit_object()) {
            Ok(Strip { message, hits }) => Self {
                id: id.to_string(),
                subject,
                hits,
                cleaned: Some(message),
            },
            Err(StripError::NothingLeft(hits)) => Self {
                id: id.to_string(),
                subject,
                hits,
                cleaned: None,
            },
        }
    }

    /// Whether anything was found.
    pub fn is_dirty(&self) -> bool {
        !self.hits.is_empty()
    }

    /// The labels found, comma separated and without repeats.
    pub fn labels(&self) -> String {
        let mut labels: Vec<&str> = Vec::new();
        for hit in &self.hits {
            if !labels.contains(&hit.label.as_str()) {
                labels.push(&hit.label);
            }
        }
        labels.join(", ")
    }

    /// Abbreviated object id, as git would show it.
    pub fn short_id(&self) -> &str {
        &self.id[..7.min(self.id.len())]
    }
}

/// Renders findings as an aligned table.
pub fn table(findings: &[&Finding], style: &Style) -> String {
    let width = subject_width(findings);
    findings
        .iter()
        .map(|finding| {
            let subject = shorten(&finding.subject);
            let padding = width.saturating_sub(subject.chars().count());
            format!(
                "  {}  {subject}{:padding$}  {}",
                style.yellow(finding.short_id()),
                "",
                style.dim(&finding.labels()),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders findings as a table of `old → new`, for a rewrite that has happened.
///
/// The ids a rewrite replaces no longer exist, so a summary that showed only
/// those would name commits the user cannot look up.
pub fn rewritten_table(
    findings: &[&Finding],
    mapping: &[(String, String)],
    style: &Style,
) -> String {
    let width = subject_width(findings);
    findings
        .iter()
        .map(|finding| {
            let subject = shorten(&finding.subject);
            let padding = width.saturating_sub(subject.chars().count());
            let new = mapping
                .iter()
                .find(|(old, _)| old == &finding.id)
                .map(|(_, new)| new[..7.min(new.len())].to_string())
                .unwrap_or_else(|| "unchanged".to_string());
            format!(
                "  {} {} {}  {subject}{:padding$}  {}",
                style.dim(finding.short_id()),
                style.dim("→"),
                style.yellow(&new),
                "",
                style.dim(&finding.labels()),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The column width shared by both tables.
fn subject_width(findings: &[&Finding]) -> usize {
    findings
        .iter()
        .map(|finding| shorten(&finding.subject).chars().count())
        .max()
        .unwrap_or(0)
        .min(SUBJECT_WIDTH)
}

/// Shortens a subject to the table width, with an ellipsis.
fn shorten(subject: &str) -> String {
    if subject.chars().count() <= SUBJECT_WIDTH {
        return subject.to_string();
    }
    let kept: String = subject.chars().take(SUBJECT_WIDTH - 1).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_a_long_subject() {
        let long = "x".repeat(80);
        let shortened = shorten(&long);
        assert_eq!(shortened.chars().count(), SUBJECT_WIDTH);
        assert!(shortened.ends_with('…'));
    }

    #[test]
    fn keeps_a_short_subject_whole() {
        assert_eq!(shorten("Fix pagination"), "Fix pagination");
    }

    fn finding(id: &str, subject: &str) -> Finding {
        Finding {
            id: id.to_string(),
            subject: subject.to_string(),
            hits: vec![Hit {
                agent: "claude".into(),
                label: "Co-Authored-By".into(),
                line: "Co-Authored-By: Claude <x@y>".into(),
            }],
            cleaned: Some(String::new()),
        }
    }

    #[test]
    fn a_rewritten_table_names_both_ids() {
        let old = finding("3df8898aaaaaaaaaaaa", "Fix pagination");
        let mapping = vec![(
            "3df8898aaaaaaaaaaaa".to_string(),
            "63ed554bbbbbbbbbbbb".to_string(),
        )];
        let rendered = rewritten_table(&[&old], &mapping, &Style::plain());
        assert!(rendered.contains("3df8898"), "{rendered}");
        assert!(rendered.contains("63ed554"), "{rendered}");
    }

    #[test]
    fn a_rewritten_table_admits_when_a_commit_was_not_rebuilt() {
        let old = finding("3df8898aaaaaaaaaaaa", "Fix pagination");
        let rendered = rewritten_table(&[&old], &[], &Style::plain());
        assert!(rendered.contains("unchanged"), "{rendered}");
    }

    #[test]
    fn lists_each_label_once() {
        let finding = Finding {
            id: "a3f19c2f0c1d".to_string(),
            subject: "Fix pagination".to_string(),
            hits: vec![
                Hit {
                    agent: "claude".into(),
                    label: "Co-Authored-By".into(),
                    line: "a".into(),
                },
                Hit {
                    agent: "claude".into(),
                    label: "Co-Authored-By".into(),
                    line: "b".into(),
                },
                Hit {
                    agent: "claude".into(),
                    label: "Claude-Session".into(),
                    line: "c".into(),
                },
            ],
            cleaned: Some(String::new()),
        };
        assert_eq!(finding.labels(), "Co-Authored-By, Claude-Session");
        assert_eq!(finding.short_id(), "a3f19c2");
    }
}
