//! `check` — report attribution without changing anything.
//!
//! Exits 1 when it finds any, so it works as a CI gate as well as a way to
//! look before rewriting.

use crate::commit::Commit;
use crate::config;
use crate::error::{Failure, Kind};
use crate::git::Git;
use crate::range::{self, Selection};
use crate::report::{self, Finding};
use crate::ui::Style;

pub fn run(selection: &Selection) -> Result<(), Failure> {
    let git = Git::here();
    git.require_repository()?;
    let rules = config::rules(&git)?;
    let range = range::resolve(&git, selection)?;
    let style = Style::stdout();

    let ids = git.rev_list(&range.spec)?;
    let mut findings = Vec::new();
    for id in &ids {
        let raw = git.cat_file_commit(id)?;
        let commit = Commit::parse(&raw)
            .map_err(|error| Failure::git(format!("{error}")).fix("This commit may be corrupt."))?;
        findings.push(Finding::examine(id, &commit, &rules));
    }

    let dirty: Vec<&Finding> = findings
        .iter()
        .filter(|finding| finding.is_dirty())
        .collect();
    if dirty.is_empty() {
        println!(
            "{} — no AI attribution found.",
            style.green(&format!("{} checked", commits(ids.len()))),
        );
        return Ok(());
    }

    println!(
        "{} of {} carry AI attribution:\n{}",
        dirty.len(),
        commits(ids.len()),
        report::table(&dirty, &style),
    );
    // The listing above already gave the numbers; this is just the remedy.
    Err(Failure::new(Kind::Found, "AI attribution found.")
        .fix("Remove it with `git no-claude-trailer clean`."))
}

/// Pluralises a commit count, because "1 commits" reads badly.
fn commits(count: usize) -> String {
    if count == 1 {
        "1 commit".to_string()
    } else {
        format!("{count} commits")
    }
}
