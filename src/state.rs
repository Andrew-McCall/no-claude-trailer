//! What is set up here.
//!
//! Gathered once and shared by `status` and the setup wizard, so the two can
//! never disagree about what they are looking at.

use std::path::PathBuf;

use crate::commit::Commit;
use crate::config;
use crate::error::Failure;
use crate::git::Git;
use crate::hooks;
use crate::profiles::Rules;
use crate::range::{self, Selection};
use crate::report::Finding;

/// Where a commit-msg hook was found, and whose it is.
#[derive(Debug, PartialEq, Eq)]
pub enum Hook {
    /// Nothing installed.
    Missing,
    /// Ours, in this repository.
    Ours(PathBuf),
    /// Ours, installed machine-wide through `core.hooksPath`.
    OursGlobal(PathBuf),
    /// Somebody else's hook, which we will not touch.
    Foreign(PathBuf),
}

/// A snapshot of the current situation.
#[derive(Debug)]
pub struct State {
    /// The repository root, or `None` when we are not in one.
    pub repository: Option<PathBuf>,
    /// The current branch, or `None` when detached or outside a repository.
    pub branch: Option<String>,
    /// Where this binary lives, which is what a hook would call.
    pub binary: Option<PathBuf>,
    pub hook: Hook,
    /// Where a repository-scoped hook would go, when we are in a repository.
    pub repo_hooks_dir: Option<PathBuf>,
    /// Where a machine-wide hook would go.
    pub global_hooks_dir: PathBuf,
    /// Agents in force, from config or the default.
    pub agents: Vec<String>,
    /// Unpushed commit count and how many carry attribution, when measurable.
    pub unpushed: Option<(usize, usize)>,
}

impl State {
    /// Looks at the repository, the config and the filesystem.
    pub fn gather(git: &Git) -> Self {
        let repository = git
            .query(&["rev-parse", "--show-toplevel"])
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        let branch = git
            .current_branch_ref()
            .map(|reference| reference.trim_start_matches("refs/heads/").to_string());
        Self {
            hook: find_hook(git, repository.is_some()),
            repo_hooks_dir: repository
                .as_ref()
                .and_then(|_| hooks::repo_directory(git).ok()),
            global_hooks_dir: hooks::global_hooks_dir(git),
            agents: config::agent_names(git),
            unpushed: count_unpushed(git).ok().flatten(),
            binary: std::env::current_exe().ok(),
            repository,
            branch,
        }
    }

    /// Whether a hook of ours is in place.
    pub fn is_installed(&self) -> bool {
        matches!(self.hook, Hook::Ours(_) | Hook::OursGlobal(_))
    }
}

/// Looks for a hook globally first, since `core.hooksPath` wins over the repo.
fn find_hook(git: &Git, in_repository: bool) -> Hook {
    let global = git
        .query(&["config", "--global", "--get", "core.hooksPath"])
        .filter(|path| !path.is_empty());
    if let Some(directory) = global {
        let path = PathBuf::from(directory).join(hooks::HOOK_NAME);
        return classify(&path, true);
    }
    if !in_repository {
        return Hook::Missing;
    }
    match hooks::repo_directory(git) {
        Ok(directory) => classify(&directory.join(hooks::HOOK_NAME), false),
        Err(_) => Hook::Missing,
    }
}

fn classify(path: &std::path::Path, global: bool) -> Hook {
    match std::fs::read_to_string(path) {
        Ok(contents) if hooks::is_ours(&contents) => {
            if global {
                Hook::OursGlobal(path.to_path_buf())
            } else {
                Hook::Ours(path.to_path_buf())
            }
        }
        Ok(_) => Hook::Foreign(path.to_path_buf()),
        Err(_) => Hook::Missing,
    }
}

/// Counts unpushed commits and how many of them carry attribution.
fn count_unpushed(git: &Git) -> Result<Option<(usize, usize)>, Failure> {
    if !git.in_repository() {
        return Ok(None);
    }
    let Ok(range) = range::resolve(git, &Selection::Unpushed) else {
        return Ok(None);
    };
    let rules = config::rules(git)?;
    let ids = git.rev_list(&range.spec)?;
    let mut dirty = 0;
    for id in &ids {
        let raw = git.cat_file_commit(id)?;
        if let Ok(commit) = Commit::parse(&raw)
            && Finding::examine(id, &commit, &rules).is_dirty()
        {
            dirty += 1;
        }
    }
    Ok(Some((ids.len(), dirty)))
}

/// Resolves rules without failing, for display only.
pub fn rules_or_default(git: &Git) -> Rules {
    config::rules(git).unwrap_or_else(|_| Rules::from_names::<&str>(&[]).expect("empty is valid"))
}
