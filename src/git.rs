//! Talking to git.
//!
//! Every git interaction goes through this module, which shells out to the
//! `git` already on the user's PATH. That keeps the crate dependency free and
//! means git's own config, worktree and object handling are never reimplemented
//! here — only asked politely.

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::commit::Ident;
use crate::error::Failure;

/// Which config file a write lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// This repository's `.git/config`.
    Local,
    /// The user's global config.
    Global,
}

impl Scope {
    fn flag(self) -> &'static str {
        match self {
            Self::Local => "--local",
            Self::Global => "--global",
        }
    }
}

/// A handle for running git commands.
#[derive(Debug, Default)]
pub struct Git {
    /// Directory to run in, or the current directory when `None`.
    directory: Option<PathBuf>,
}

impl Git {
    /// Runs git in the current directory.
    pub fn here() -> Self {
        Self { directory: None }
    }

    /// Runs git in a specific directory.
    pub fn in_directory(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: Some(directory.into()),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        if let Some(directory) = &self.directory {
            command.arg("-C").arg(directory);
        }
        command
    }

    /// Runs git and returns trimmed stdout, failing with git's own stderr.
    pub fn run<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<String, Failure> {
        let output = self
            .command()
            .args(args)
            .output()
            .map_err(|error| self.spawn_failure(error))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr)
                .trim_end()
                .to_string();
            let shown = describe(args);
            return Err(Failure::git(format!("`git {shown}` failed: {stderr}")));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string())
    }

    /// Runs git, treating a non-zero exit as "no answer" rather than an error.
    ///
    /// Half of git's query commands report absence by failing, so this keeps
    /// the callers free of noise.
    pub fn query<S: AsRef<OsStr>>(&self, args: &[S]) -> Option<String> {
        let output = self.command().args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let answer = String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string();
        Some(answer)
    }

    fn spawn_failure(&self, error: std::io::Error) -> Failure {
        Failure::git(format!("could not run git: {error}"))
            .fix("Install git, or make sure it is on your PATH.")
    }

    /// Whether the current directory is inside a git repository.
    pub fn in_repository(&self) -> bool {
        self.query(&["rev-parse", "--git-dir"]).is_some()
    }

    /// Fails with an explanation when not inside a repository.
    pub fn require_repository(&self) -> Result<(), Failure> {
        if self.in_repository() {
            return Ok(());
        }
        Err(Failure::refused("this is not a git repository")
            .fix("Change into a repository first, or run `git init`."))
    }

    /// All values of a multi-valued config key, in config order.
    pub fn config_all(&self, key: &str) -> Vec<String> {
        self.query(&["config", "--get-all", key])
            .map(|values| values.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// The last value of a config key, if it is set to anything non-empty.
    pub fn config(&self, key: &str) -> Option<String> {
        self.query(&["config", "--get", key])
            .filter(|value| !value.is_empty())
    }

    pub fn config_set(&self, scope: Scope, key: &str, value: &str) -> Result<(), Failure> {
        self.run(&["config", scope.flag(), key, value]).map(|_| ())
    }

    pub fn config_add(&self, scope: Scope, key: &str, value: &str) -> Result<(), Failure> {
        self.run(&["config", scope.flag(), "--add", key, value])
            .map(|_| ())
    }

    /// Removes a key entirely, and is content if it was never set.
    pub fn config_unset_all(&self, scope: Scope, key: &str) -> Result<(), Failure> {
        self.query(&["config", scope.flag(), "--unset-all", key]);
        Ok(())
    }

    /// The comment prefix git will strip from a commit message.
    ///
    /// `core.commentString` (git 2.45 and later) wins over `core.commentChar`.
    /// The value `auto` means git picks a character per message, which cannot
    /// be predicted here, so the default `#` is used.
    pub fn comment_prefix(&self) -> String {
        let configured = self
            .config("core.commentString")
            .or_else(|| self.config("core.commentChar"))
            .unwrap_or_else(|| "#".to_string());
        if configured.eq_ignore_ascii_case("auto") {
            return "#".to_string();
        }
        configured
    }

    /// Resolves a revision to an object id.
    pub fn rev_parse(&self, revision: &str) -> Option<String> {
        self.query(&["rev-parse", "--verify", "--quiet", revision])
    }

    /// A path inside the git directory, such as `hooks`.
    pub fn git_path(&self, relative: &str) -> Result<PathBuf, Failure> {
        let path = self.run(&["rev-parse", "--git-path", relative])?;
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path);
        }
        // `--git-path` answers relative to the repository root.
        let root = self.run(&["rev-parse", "--show-toplevel"])?;
        Ok(Path::new(&root).join(path))
    }

    /// The current branch's full ref name, or `None` when HEAD is detached.
    pub fn current_branch_ref(&self) -> Option<String> {
        self.query(&["symbolic-ref", "--quiet", "HEAD"])
    }

    /// The name of a git operation that is part way through, if any.
    ///
    /// Rewriting history underneath a rebase or merge would strand it, so the
    /// commands that rewrite refuse while one is running.
    pub fn operation_in_progress(&self) -> Option<&'static str> {
        const STATES: &[(&str, &str)] = &[
            ("rebase-merge", "a rebase"),
            ("rebase-apply", "a rebase"),
            ("MERGE_HEAD", "a merge"),
            ("CHERRY_PICK_HEAD", "a cherry-pick"),
            ("REVERT_HEAD", "a revert"),
            ("BISECT_LOG", "a bisect"),
        ];
        STATES
            .iter()
            .find(|(path, _)| self.git_path(path).is_ok_and(|path| path.exists()))
            .map(|(_, name)| *name)
    }

    /// Whether the working tree and index are free of changes.
    pub fn is_clean(&self) -> Result<bool, Failure> {
        Ok(self.run(&["status", "--porcelain"])?.is_empty())
    }

    /// Commit ids in a range, oldest first.
    pub fn rev_list(&self, range: &str) -> Result<Vec<String>, Failure> {
        let listing = self.run(&["rev-list", "--reverse", "--topo-order", range])?;
        Ok(listing.lines().map(str::to_string).collect())
    }

    /// The raw bytes of a commit object.
    pub fn cat_file_commit(&self, id: &str) -> Result<String, Failure> {
        let output = self
            .command()
            .args(["cat-file", "commit", id])
            .output()
            .map_err(|error| self.spawn_failure(error))?;
        if !output.status.success() {
            return Err(Failure::git(format!("could not read commit {id}")));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Writes a new commit object, preserving identities and dates exactly.
    pub fn commit_tree(
        &self,
        tree: &str,
        parents: &[String],
        author: &Ident,
        committer: &Ident,
        message: &str,
        sign: bool,
    ) -> Result<String, Failure> {
        let mut command = self.command();
        command.arg("commit-tree").arg(tree);
        for parent in parents {
            command.arg("-p").arg(parent);
        }
        if sign {
            command.arg("-S");
        }
        command
            .env("GIT_AUTHOR_NAME", &author.name)
            .env("GIT_AUTHOR_EMAIL", &author.email)
            .env("GIT_AUTHOR_DATE", &author.date)
            .env("GIT_COMMITTER_NAME", &committer.name)
            .env("GIT_COMMITTER_EMAIL", &committer.email)
            .env("GIT_COMMITTER_DATE", &committer.date)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| self.spawn_failure(error))?;
        child
            .stdin
            .take()
            .expect("stdin was piped")
            .write_all(message.as_bytes())
            .map_err(|error| Failure::git(format!("could not write a commit message: {error}")))?;
        let output = child
            .wait_with_output()
            .map_err(|error| Failure::git(format!("commit-tree did not finish: {error}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr)
                .trim_end()
                .to_string();
            let mut failure = Failure::git(format!("could not write a new commit: {stderr}"));
            if sign {
                failure = failure.fix(
                    "Signing failed, so nothing was rewritten. \
                     Check your signing key with `git config --get user.signingkey`.",
                );
            }
            return Err(failure);
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string())
    }

    /// Moves a ref, refusing if it no longer points where we expect.
    pub fn update_ref(&self, name: &str, new: &str, old: &str) -> Result<(), Failure> {
        self.run(&[
            "update-ref",
            "-m",
            "no-claude-trailer: strip attribution",
            name,
            new,
            old,
        ])
        .map(|_| ())
    }

    /// Creates a ref without expecting a previous value.
    pub fn create_ref(&self, name: &str, target: &str) -> Result<(), Failure> {
        self.run(&["update-ref", name, target]).map(|_| ())
    }
}

fn describe<S: AsRef<OsStr>>(args: &[S]) -> String {
    args.iter()
        .map(|arg| arg.as_ref().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
