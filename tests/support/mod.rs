// Each integration test binary compiles this whole module but uses only the
// parts it needs, so unused helpers here are expected rather than dead.
#![allow(dead_code)]

//! A throwaway git repository for integration tests.
//!
//! Tests drive the real binary against real git, because that is the only way
//! to prove a commit-msg hook fires and that rewritten commits keep their
//! dates. Every invocation is hermetic: the machine's own git config, global
//! hooks and identity are replaced with ones inside the temporary directory,
//! so a developer's `core.hooksPath` cannot change the result of a test.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The binary under test, built by cargo before integration tests run.
pub fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_git-no-claude-trailer")
}

pub struct TempRepo {
    pub path: PathBuf,
    root: PathBuf,
    home: PathBuf,
    global_config: PathBuf,
}

impl TempRepo {
    /// Creates an initialised repository with a deterministic identity.
    pub fn new(label: &str) -> Self {
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/tmp")
            .join(format!("{label}-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("repo");
        let home = root.join("home");
        std::fs::create_dir_all(&path).expect("create repo dir");
        std::fs::create_dir_all(&home).expect("create home dir");
        let global_config = root.join("gitconfig-global");
        std::fs::write(&global_config, "").expect("create global config");

        let repo = Self {
            path,
            root: root.clone(),
            home,
            global_config,
        };
        repo.git(&["init", "--quiet", "--initial-branch=main"]);
        repo.git(&["config", "user.name", "Test Person"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }

    /// The temporary root holding the repo, its remote and its fake home.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The isolated global config file, so `--global` tests stay hermetic.
    pub fn global_config(&self) -> &Path {
        &self.global_config
    }

    /// The isolated home directory, where a global hooks dir would land.
    pub fn home(&self) -> &Path {
        &self.home
    }

    fn isolate(&self, command: &mut Command) {
        command
            .current_dir(&self.path)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            // Without a ceiling, git walks up out of the temporary directory
            // and finds this project's own repository.
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .env("GIT_AUTHOR_DATE", "1758100000 +0100")
            .env("GIT_COMMITTER_DATE", "1758100060 +0100")
            .env("GIT_AUTHOR_NAME", "Test Person")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test Person")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env_remove("NO_COLOR");
    }

    /// Runs git in the repository, returning trimmed stdout. Panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        let output = self.git_output(args);
        assert!(
            output.status.success(),
            "git {args:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string()
    }

    pub fn git_output(&self, args: &[&str]) -> Output {
        let mut command = Command::new("git");
        self.isolate(&mut command);
        command.args(args).output().expect("run git")
    }

    /// Runs the binary under test in the repository.
    pub fn run(&self, args: &[&str]) -> Output {
        let mut command = Command::new(binary());
        self.isolate(&mut command);
        command.args(args).output().expect("run binary")
    }

    /// Runs the binary with text on stdin.
    pub fn run_with_stdin(&self, args: &[&str], stdin: &str) -> Output {
        use std::io::Write;
        use std::process::Stdio;
        let mut command = Command::new(binary());
        self.isolate(&mut command);
        let mut child = command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn binary");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(stdin.as_bytes())
            .expect("write stdin");
        child.wait_with_output().expect("wait for binary")
    }

    /// Creates a commit with the given message and no file changes.
    pub fn commit(&self, message: &str) -> String {
        std::fs::write(self.path.join("message.tmp"), message).expect("write message");
        self.git(&[
            "commit",
            "--allow-empty",
            "--quiet",
            "--no-verify",
            "--file",
            "message.tmp",
        ]);
        std::fs::remove_file(self.path.join("message.tmp")).expect("remove message");
        self.git(&["rev-parse", "HEAD"])
    }

    /// Adds a bare remote, pushes `main` and sets it as upstream.
    ///
    /// The default range is `@{upstream}..HEAD`, so tests that exercise it need
    /// a published branch to measure against.
    pub fn with_upstream(&self) -> &Self {
        let remote = self.path.parent().unwrap().join("origin.git");
        std::fs::create_dir_all(&remote).expect("create remote dir");
        let remote = remote.to_str().expect("remote path").to_string();
        Command::new("git")
            .args(["init", "--bare", "--quiet", &remote])
            .output()
            .expect("init bare remote");
        self.git(&["remote", "add", "origin", &remote]);
        self.git(&["push", "--quiet", "--set-upstream", "origin", "main"]);
        self
    }

    /// Creates a commit with hooks enabled, so an installed hook can act.
    pub fn commit_with_hooks(&self, message: &str) -> Output {
        std::fs::write(self.path.join("message.tmp"), message).expect("write message");
        let output = self.git_output(&[
            "commit",
            "--allow-empty",
            "--quiet",
            "--file",
            "message.tmp",
        ]);
        let _ = std::fs::remove_file(self.path.join("message.tmp"));
        output
    }

    /// Every commit message on the current branch, newest first.
    pub fn messages(&self) -> Vec<String> {
        self.git(&["log", "--format=%B%x00"])
            .split('\0')
            .map(|entry| entry.trim_matches('\n').to_string())
            .filter(|entry| !entry.is_empty())
            .collect()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        if std::env::var_os("NO_CLAUDE_TRAILER_KEEP_TMP").is_none() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

/// Combined stdout and stderr, for asserting on human-facing output.
pub fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
