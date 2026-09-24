//! The commit-msg hook: where it goes, what it says, and how to take it away.
//!
//! The hook is a short shell script rather than a copy of the binary, so it can
//! be read and understood in place. It calls the binary by absolute path, so a
//! changed `PATH` cannot quietly stop it working, and falls back to a `PATH`
//! lookup if that path is gone.
//!
//! It fails closed: if the binary cannot be found the commit is refused, with a
//! message saying how to reinstall it or remove the hook. That is a deliberate
//! trade — a broken install blocks commits rather than letting attribution
//! through unnoticed.

use std::path::{Path, PathBuf};

use crate::clock;
use crate::config;
use crate::error::Failure;
use crate::git::{Git, Scope};

/// Identifies a hook as ours, so uninstall never deletes someone else's.
pub const MARKER: &str = "no-claude-trailer/v1";
/// The hook git runs for every commit message.
pub const HOOK_NAME: &str = "commit-msg";
/// Suffix for a hook we moved aside.
pub const BACKUP_SUFFIX: &str = ".no-claude-trailer.bak";

/// The hook directory for this repository, honouring `core.hooksPath`.
pub fn repo_directory(git: &Git) -> Result<PathBuf, Failure> {
    if let Some(configured) = git.config("core.hooksPath") {
        return Ok(expand_home(&configured));
    }
    git.git_path("hooks")
}

/// Where a machine-wide hook goes, and whether `core.hooksPath` needs setting.
///
/// An existing `core.hooksPath` is somebody's arrangement, so adding a hook to
/// it is something you have to ask for by name.
pub fn global_directory(git: &Git, force: bool) -> Result<(PathBuf, bool), Failure> {
    match git.query(&["config", "--global", "--get", "core.hooksPath"]) {
        Some(configured) if !configured.is_empty() => {
            if !force {
                return Err(Failure::refused(format!(
                    "core.hooksPath is already set to {configured}"
                ))
                .fix(format!(
                    "Installing would add a {HOOK_NAME} hook to that directory. \
                     Re-run with --force to do that, leaving core.hooksPath alone."
                )));
            }
            Ok((expand_home(&configured), false))
        }
        _ => Ok((default_global_directory(), true)),
    }
}

/// Where a machine-wide hook lives: `core.hooksPath` if set, else the default.
pub fn global_hooks_dir(git: &Git) -> PathBuf {
    git.query(&["config", "--global", "--get", "core.hooksPath"])
        .filter(|path| !path.is_empty())
        .map(|path| expand_home(&path))
        .unwrap_or_else(default_global_directory)
}

/// `$XDG_CONFIG_HOME/git/hooks`, or `~/.config/git/hooks`.
fn default_global_directory() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join("git/hooks");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/git/hooks")
}

/// Expands a leading `~/`, which git accepts in config but the OS does not.
fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(rest)
        }
        None => PathBuf::from(path),
    }
}

/// The hook script, written to explain itself to whoever finds it.
pub fn script(binary: &Path) -> String {
    format!(
        "#!/bin/sh\n\
         # {HOOK_NAME} hook installed by no-claude-trailer on {date}.\n\
         # Strips AI attribution from commit messages before they land.\n\
         #\n\
         # Agents stripped:  git config --get-all {agent_key}\n\
         # Remove this hook: git no-claude-trailer uninstall\n\
         # marker: {MARKER}\n\
         \n\
         binary='{binary}'\n\
         if [ ! -x \"$binary\" ]; then\n\
         \tbinary=\"$(command -v git-no-claude-trailer || true)\"\n\
         fi\n\
         if [ -z \"$binary\" ] || [ ! -x \"$binary\" ]; then\n\
         \techo 'no-claude-trailer: cannot strip attribution, the binary is missing:' >&2\n\
         \techo '  {binary}' >&2\n\
         \techo 'Your commit was blocked. Reinstall it, or remove this hook with' >&2\n\
         \techo '  git no-claude-trailer uninstall' >&2\n\
         \texit 1\n\
         fi\n\
         exec \"$binary\" filter --in-place \"$1\"\n",
        date = clock::today(),
        agent_key = config::AGENT_KEY,
        binary = binary.display(),
    )
}

/// Whether a hook script is one of ours.
pub fn is_ours(script: &str) -> bool {
    script.contains(MARKER)
}

/// Writes the hook and makes it executable.
pub fn write(path: &Path, script: &str) -> Result<(), Failure> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Failure::git(format!("could not create {}: {error}", parent.display()))
        })?;
    }
    std::fs::write(path, script)
        .map_err(|error| Failure::git(format!("could not write {}: {error}", path.display())))?;
    make_executable(path)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), Failure> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|error| {
        Failure::git(format!(
            "could not make {} executable: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), Failure> {
    // Windows has no executable bit; git runs hooks through its bundled shell.
    Ok(())
}

/// The path a replaced hook is kept at.
pub fn backup_path(hook: &Path) -> PathBuf {
    let mut name = hook.file_name().unwrap_or_default().to_os_string();
    name.push(BACKUP_SUFFIX);
    hook.with_file_name(name)
}

/// Records that we set `core.hooksPath`, so uninstall knows to undo it.
pub fn claim_hooks_path(git: &Git, directory: &Path) -> Result<(), Failure> {
    let directory = directory.to_string_lossy().to_string();
    git.config_set(Scope::Global, "core.hooksPath", &directory)?;
    git.config_set(Scope::Global, config::OWNS_HOOKS_PATH_KEY, "true")
}

/// Whether we are the ones who set `core.hooksPath`.
pub fn owns_hooks_path(git: &Git) -> bool {
    git.query(&["config", "--global", "--get", config::OWNS_HOOKS_PATH_KEY])
        .is_some_and(|value| value.trim() == "true")
}

/// Undoes [`claim_hooks_path`].
pub fn release_hooks_path(git: &Git) -> Result<(), Failure> {
    git.config_unset_all(Scope::Global, "core.hooksPath")?;
    git.config_unset_all(Scope::Global, config::OWNS_HOOKS_PATH_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_identifies_itself() {
        let script = script(Path::new("/usr/local/bin/git-no-claude-trailer"));
        assert!(is_ours(&script));
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("/usr/local/bin/git-no-claude-trailer"));
        assert!(script.contains("git no-claude-trailer uninstall"));
    }

    #[test]
    fn someone_elses_script_is_not_ours() {
        assert!(!is_ours("#!/bin/sh\nexec npx husky\n"));
    }

    #[test]
    fn a_backup_sits_beside_the_hook() {
        assert_eq!(
            backup_path(Path::new("/r/.git/hooks/commit-msg")),
            PathBuf::from("/r/.git/hooks/commit-msg.no-claude-trailer.bak")
        );
    }

    #[test]
    fn a_tilde_in_config_is_expanded() {
        // SAFETY: single-threaded test, and the value is restored below.
        let before = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", "/home/tester") };
        assert_eq!(expand_home("~/hooks"), PathBuf::from("/home/tester/hooks"));
        assert_eq!(expand_home("/etc/hooks"), PathBuf::from("/etc/hooks"));
        match before {
            Some(value) => unsafe { std::env::set_var("HOME", value) },
            None => unsafe { std::env::remove_var("HOME") },
        }
    }
}
