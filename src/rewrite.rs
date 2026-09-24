//! Planning and applying a history rewrite.
//!
//! Planning is read-only and complete: every commit is examined and every
//! refusal raised before a single object is written. Applying then writes all
//! the new commits and moves the branch **last**, so an interrupted or refused
//! run leaves the repository exactly as it was.

use crate::clock;
use crate::commit::Commit;
use crate::error::Failure;
use crate::git::Git;
use crate::profiles::Rules;
use crate::range::Range;
use crate::report::Finding;

/// Where backup refs live, so a rewrite is always reversible.
pub const BACKUP_PREFIX: &str = "refs/no-claude-trailer/backup";

/// One commit in the plan.
pub struct Step {
    pub id: String,
    pub commit: Commit,
    pub finding: Finding,
    /// Whether this commit has to be rebuilt, either because it is dirty or
    /// because an ancestor was rebuilt and its parent id will change.
    pub rebuild: bool,
}

/// A complete, checked rewrite that has not yet touched anything.
pub struct Plan {
    pub steps: Vec<Step>,
    /// The branch ref to move, or `None` when HEAD is detached.
    pub branch: Option<String>,
    pub old_head: String,
    /// Other refs that point at commits being rebuilt and will be left behind.
    pub stranded_refs: Vec<String>,
}

/// What a rewrite did.
pub struct Applied {
    pub new_head: String,
    pub backup_ref: String,
    /// Old id to new id, for every commit rebuilt.
    pub mapping: Vec<(String, String)>,
    pub resigned: usize,
}

impl Applied {
    /// How many commits were rebuilt.
    pub fn rewritten(&self) -> usize {
        self.mapping.len()
    }
}

impl Plan {
    /// Examines every commit in the range and raises anything that would stop us.
    pub fn build(git: &Git, rules: &Rules, range: &Range) -> Result<Self, Failure> {
        require_tip_is_head(git, range)?;
        let old_head = git
            .rev_parse("HEAD")
            .ok_or_else(|| Failure::refused("this repository has no commits yet"))?;

        let mut steps: Vec<Step> = Vec::new();
        for id in git.rev_list(&range.spec)? {
            let raw = git.cat_file_commit(&id)?;
            let commit = Commit::parse(&raw).map_err(|error| {
                Failure::git(error.to_string())
                    .fix("Nothing has been changed. This commit may be corrupt.")
            })?;
            let finding = Finding::examine(&id, &commit, rules);
            let parent_rebuilt = commit
                .parents
                .iter()
                .any(|parent| steps.iter().any(|step| step.rebuild && &step.id == parent));
            let rebuild = finding.is_dirty() || parent_rebuilt;

            if rebuild && finding.is_dirty() {
                refuse_if_unrepresentable(&commit, &finding)?;
            }
            steps.push(Step {
                id,
                commit,
                finding,
                rebuild,
            });
        }

        let stranded_refs = find_stranded_refs(git, &steps);
        Ok(Self {
            steps,
            branch: git.current_branch_ref(),
            old_head,
            stranded_refs,
        })
    }

    /// The commits that carry attribution.
    pub fn dirty(&self) -> Vec<&Finding> {
        self.steps
            .iter()
            .filter(|step| step.finding.is_dirty())
            .map(|step| &step.finding)
            .collect()
    }

    /// Whether there is anything to do.
    pub fn is_empty(&self) -> bool {
        !self.steps.iter().any(|step| step.rebuild)
    }

    /// How many commits would be rebuilt, including untouched descendants.
    pub fn rebuild_count(&self) -> usize {
        self.steps.iter().filter(|step| step.rebuild).count()
    }

    /// The branch name for display, or a note that HEAD is detached.
    pub fn branch_name(&self) -> String {
        match &self.branch {
            Some(reference) => reference.trim_start_matches("refs/heads/").to_string(),
            None => "a detached HEAD".to_string(),
        }
    }

    /// Writes the new commits, then moves the ref. Nothing moves until the end.
    pub fn apply(&self, git: &Git) -> Result<Applied, Failure> {
        let mut mapping: Vec<(String, String)> = Vec::new();
        let mut resigned = 0;

        for step in &self.steps {
            if !step.rebuild {
                continue;
            }
            let message = match &step.finding.cleaned {
                Some(cleaned) => cleaned.clone(),
                // A commit only reaching here unchanged is a descendant being
                // re-parented, so its message is kept as it is.
                None => step.commit.message.clone(),
            };
            let parents: Vec<String> = step
                .commit
                .parents
                .iter()
                .map(|parent| remap(&mapping, parent))
                .collect();
            if step.commit.signed {
                resigned += 1;
            }
            let new = git.commit_tree(
                &step.commit.tree,
                &parents,
                &step.commit.author,
                &step.commit.committer,
                &message,
                step.commit.signed,
            )?;
            mapping.push((step.id.clone(), new));
        }

        let new_head = remap(&mapping, &self.old_head);
        let backup_ref = format!(
            "{BACKUP_PREFIX}/{}-{}",
            self.backup_label(),
            clock::stamp_now()
        );
        git.create_ref(&backup_ref, &self.old_head)?;

        let reference = self.branch.clone().unwrap_or_else(|| "HEAD".to_string());
        git.update_ref(&reference, &new_head, &self.old_head)?;

        Ok(Applied {
            new_head,
            backup_ref,
            mapping,
            resigned,
        })
    }

    /// A filesystem-safe name for the backup ref.
    fn backup_label(&self) -> String {
        match &self.branch {
            Some(reference) => reference
                .trim_start_matches("refs/heads/")
                .replace('/', "-"),
            None => "detached".to_string(),
        }
    }
}

/// Looks up a rewritten id, falling back to the original.
fn remap(mapping: &[(String, String)], id: &str) -> String {
    mapping
        .iter()
        .find(|(old, _)| old == id)
        .map(|(_, new)| new.clone())
        .unwrap_or_else(|| id.to_string())
}

/// Refuses the two cases `commit-tree` cannot reproduce faithfully.
fn refuse_if_unrepresentable(commit: &Commit, finding: &Finding) -> Result<(), Failure> {
    let short = finding.short_id();
    if finding.cleaned.is_none() {
        return Err(Failure::refused(format!(
            "commit {short} would have nothing left in its message"
        ))
        .fix(format!(
            "Nothing has been changed. Give {short} a real message first, \
             with `git rebase -i --reword {short}`."
        )));
    }
    if let Some(encoding) = &commit.encoding
        && !encoding.eq_ignore_ascii_case("utf-8")
        && !encoding.eq_ignore_ascii_case("utf8")
    {
        return Err(Failure::refused(format!(
                "commit {short} declares the encoding {encoding}, which cannot be preserved when rebuilding it"
            ))
            .fix(
                "Nothing has been changed. Re-encode the message to UTF-8, \
                 or strip this commit by hand.",
            ));
    }
    Ok(())
}

/// Refuses a range whose tip is not HEAD, since only HEAD's branch is moved.
fn require_tip_is_head(git: &Git, range: &Range) -> Result<(), Failure> {
    let Some((_, tip)) = range.spec.split_once("..") else {
        return Ok(());
    };
    let head = git.rev_parse("HEAD");
    if head.is_some() && git.rev_parse(tip) == head {
        return Ok(());
    }
    Err(Failure::refused(format!(
        "clean rewrites the branch you are on, so the range has to end at HEAD, not {tip}"
    ))
    .fix("Check out the commit you want to rewrite up to, or pass --range <start> and let it end at HEAD."))
}

/// Refs that point at commits being rebuilt, and so will be left behind.
fn find_stranded_refs(git: &Git, steps: &[Step]) -> Vec<String> {
    let current = git.current_branch_ref();
    let mut stranded = Vec::new();
    for step in steps.iter().filter(|step| step.rebuild) {
        let listing = git
            .query(&[
                "for-each-ref",
                "--format=%(refname)",
                "--points-at",
                &step.id,
            ])
            .unwrap_or_default();
        for reference in listing.lines() {
            let is_current = current.as_deref() == Some(reference);
            let is_ours = reference.starts_with(BACKUP_PREFIX);
            if !is_current && !is_ours && !stranded.contains(&reference.to_string()) {
                stranded.push(reference.to_string());
            }
        }
    }
    stranded
}
