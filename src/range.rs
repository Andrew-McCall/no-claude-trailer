//! Choosing which commits a command acts on.
//!
//! The default is deliberately timid: only commits that have not been pushed.
//! Rewriting published history is something you have to ask for by name, with
//! `--range` or `--all`.

use crate::error::Failure;
use crate::git::Git;

/// Which commits the user asked for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Selection {
    /// Everything not yet pushed: `@{upstream}..HEAD`.
    #[default]
    Unpushed,
    /// Every commit reachable from HEAD.
    All,
    /// A revision to start after, or a full `a..b` range.
    Explicit(String),
}

/// A resolved range, with wording for the human reading the output.
#[derive(Debug, PartialEq, Eq)]
pub struct Range {
    /// What to hand to `git rev-list`.
    pub spec: String,
    /// How to describe the range in a sentence.
    pub description: String,
}

/// Turns a selection into a range, explaining anything that cannot be resolved.
pub fn resolve(git: &Git, selection: &Selection) -> Result<Range, Failure> {
    let range = match selection {
        Selection::Unpushed => {
            if git.rev_parse("@{upstream}").is_none() {
                return Err(Failure::refused(
                    "this branch has no upstream, so there is no way to tell which commits are unpushed",
                )
                .fix(
                    "Pass --range <commit> to choose where to start, or --all for the whole branch.",
                ));
            }
            Range {
                spec: "@{upstream}..HEAD".to_string(),
                description: "commits not yet pushed".to_string(),
            }
        }
        Selection::All => Range {
            spec: "HEAD".to_string(),
            description: "every commit on this branch".to_string(),
        },
        Selection::Explicit(revision) => {
            let spec = if revision.contains("..") {
                revision.clone()
            } else {
                format!("{revision}..HEAD")
            };
            Range {
                spec,
                description: format!("commits in {revision}"),
            }
        }
    };

    if git
        .query(&["rev-list", "--max-count=1", &range.spec])
        .is_none()
    {
        return Err(Failure::usage(format!(
            "cannot resolve {:?} as a range of commits",
            range.spec
        ))
        .fix("Pass a commit, tag or branch that exists, or a full `a..b` range."));
    }
    Ok(range)
}
