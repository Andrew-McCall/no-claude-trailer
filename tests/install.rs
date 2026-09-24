//! `install` and `uninstall` manage the commit-msg hook.

mod support;

use support::{TempRepo, text};

const DIRTY: &str = concat!(
    "Fix pagination\n",
    "\n",
    "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n",
);

fn hook(repo: &TempRepo) -> std::path::PathBuf {
    repo.path.join(".git/hooks/commit-msg")
}

#[test]
fn writes_an_executable_hook() {
    let repo = TempRepo::new("install-basic");
    let output = repo.run(&["install"]);
    assert!(output.status.success(), "{}", text(&output));
    let path = hook(&repo);
    assert!(path.exists(), "the hook should exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "the hook must be executable, got {mode:o}"
        );
    }
}

#[test]
fn the_hook_explains_itself() {
    let repo = TempRepo::new("install-self-documenting");
    repo.run(&["install"]);
    let script = std::fs::read_to_string(hook(&repo)).unwrap();
    assert!(
        script.contains("no-claude-trailer/v1"),
        "needs a marker: {script}"
    );
    assert!(
        script.contains("uninstall"),
        "should say how to remove it: {script}"
    );
    assert!(
        script.contains(support::binary()),
        "should call the binary by path: {script}"
    );
}

#[test]
fn the_installed_hook_strips_a_real_commit() {
    let repo = TempRepo::new("install-fires");
    assert!(repo.run(&["install"]).status.success());
    let output = repo.commit_with_hooks(DIRTY);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(repo.messages()[0], "Fix pagination");
}

#[test]
fn the_hook_blocks_the_commit_when_the_binary_is_missing() {
    let repo = TempRepo::new("install-fails-closed");
    repo.run(&["install"]);
    let path = hook(&repo);
    let script = std::fs::read_to_string(&path)
        .unwrap()
        .replace(support::binary(), "/nonexistent/git-no-claude-trailer");
    std::fs::write(&path, script).unwrap();

    let output = repo.commit_with_hooks(DIRTY);
    assert!(
        !output.status.success(),
        "a broken hook must block the commit"
    );
    let shown = text(&output);
    assert!(shown.contains("no-claude-trailer"), "{shown}");
    assert!(
        shown.contains("uninstall"),
        "the message must offer a way out: {shown}"
    );
}

#[test]
fn refuses_to_replace_someone_elses_hook() {
    let repo = TempRepo::new("install-existing");
    let path = hook(&repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\necho someone else\n").unwrap();

    let output = repo.run(&["install"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    let shown = text(&output);
    assert!(
        shown.contains("--force"),
        "the fix should mention --force: {shown}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "#!/bin/sh\necho someone else\n",
        "the existing hook must be untouched"
    );
}

#[test]
fn force_backs_up_the_existing_hook() {
    let repo = TempRepo::new("install-force");
    let path = hook(&repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\necho someone else\n").unwrap();

    let output = repo.run(&["install", "--force"]);
    assert!(output.status.success(), "{}", text(&output));
    let backup = repo
        .path
        .join(".git/hooks/commit-msg.no-claude-trailer.bak");
    assert_eq!(
        std::fs::read_to_string(backup).unwrap(),
        "#!/bin/sh\necho someone else\n"
    );
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("no-claude-trailer/v1")
    );
}

#[test]
fn force_refuses_to_overwrite_an_existing_backup() {
    let repo = TempRepo::new("install-force-twice");
    let path = hook(&repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\necho first\n").unwrap();
    std::fs::write(
        repo.path
            .join(".git/hooks/commit-msg.no-claude-trailer.bak"),
        "#!/bin/sh\necho older\n",
    )
    .unwrap();

    let output = repo.run(&["install", "--force"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(text(&output).contains("bak"), "{}", text(&output));
}

#[test]
fn installing_twice_is_harmless() {
    let repo = TempRepo::new("install-idempotent");
    assert!(repo.run(&["install"]).status.success());
    let output = repo.run(&["install"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).to_lowercase().contains("already"),
        "{}",
        text(&output)
    );
}

#[test]
fn records_the_chosen_agents() {
    let repo = TempRepo::new("install-agents");
    let output = repo.run(&["install", "--agents=claude,copilot"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(
        repo.git(&["config", "--get-all", "noclaudetrailer.agent"]),
        "claude\ncopilot"
    );
}

#[test]
fn rejects_an_unknown_agent_and_lists_the_known_ones() {
    let repo = TempRepo::new("install-bad-agent");
    let output = repo.run(&["install", "--agents=skynet"]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    let shown = text(&output);
    assert!(shown.contains("skynet"), "{shown}");
    assert!(
        shown.contains("copilot"),
        "should list what is available: {shown}"
    );
    assert!(!hook(&repo).exists(), "nothing should be installed");
}

#[test]
fn installs_into_a_configured_hooks_path() {
    let repo = TempRepo::new("install-hooks-path");
    let elsewhere = repo.path.join("my-hooks");
    std::fs::create_dir_all(&elsewhere).unwrap();
    repo.git(&["config", "core.hooksPath", elsewhere.to_str().unwrap()]);

    let output = repo.run(&["install"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        elsewhere.join("commit-msg").exists(),
        "should honour core.hooksPath"
    );
    assert!(!hook(&repo).exists());
}

#[test]
fn global_install_sets_the_hooks_path() {
    let repo = TempRepo::new("install-global");
    let output = repo.run(&["install", "--global"]);
    assert!(output.status.success(), "{}", text(&output));

    let configured = repo.git(&["config", "--global", "--get", "core.hooksPath"]);
    assert!(
        !configured.is_empty(),
        "core.hooksPath should be set globally"
    );
    assert!(
        std::path::Path::new(&configured)
            .join("commit-msg")
            .exists(),
        "{configured}"
    );
    assert_eq!(
        repo.git(&[
            "config",
            "--global",
            "--get",
            "noclaudetrailer.ownsHooksPath"
        ]),
        "true"
    );
    assert!(
        !hook(&repo).exists(),
        "a global install should not touch the repo"
    );
}

#[test]
fn global_install_refuses_to_hijack_someone_elses_hooks_path() {
    let repo = TempRepo::new("install-global-conflict");
    let elsewhere = repo.root().join("their-hooks");
    std::fs::create_dir_all(&elsewhere).unwrap();
    repo.git(&[
        "config",
        "--global",
        "core.hooksPath",
        elsewhere.to_str().unwrap(),
    ]);

    let output = repo.run(&["install", "--global"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(
        text(&output).contains("core.hooksPath"),
        "{}",
        text(&output)
    );
}

#[test]
fn uninstall_removes_our_hook() {
    let repo = TempRepo::new("uninstall-basic");
    repo.run(&["install"]);
    let output = repo.run(&["uninstall"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(!hook(&repo).exists(), "the hook should be gone");
}

#[test]
fn uninstall_restores_a_hook_it_replaced() {
    let repo = TempRepo::new("uninstall-restores");
    let path = hook(&repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\necho someone else\n").unwrap();
    repo.run(&["install", "--force"]);

    let output = repo.run(&["uninstall"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "#!/bin/sh\necho someone else\n"
    );
    assert!(
        !repo
            .path
            .join(".git/hooks/commit-msg.no-claude-trailer.bak")
            .exists()
    );
}

#[test]
fn uninstall_refuses_to_delete_a_hook_that_is_not_ours() {
    let repo = TempRepo::new("uninstall-foreign");
    let path = hook(&repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\necho someone else\n").unwrap();

    let output = repo.run(&["uninstall"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(path.exists(), "someone else's hook must survive");
}

#[test]
fn uninstall_says_so_when_there_is_nothing_installed() {
    let repo = TempRepo::new("uninstall-nothing");
    let output = repo.run(&["uninstall"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("no"), "{}", text(&output));
}

#[test]
fn global_uninstall_unsets_the_hooks_path_it_set() {
    let repo = TempRepo::new("uninstall-global");
    repo.run(&["install", "--global"]);
    let output = repo.run(&["uninstall", "--global"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(
        repo.git_output(&["config", "--global", "--get", "core.hooksPath"])
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn global_uninstall_leaves_a_hooks_path_it_did_not_set() {
    let repo = TempRepo::new("uninstall-global-foreign");
    let elsewhere = repo.root().join("their-hooks");
    std::fs::create_dir_all(elsewhere.join("commit-msg").parent().unwrap()).unwrap();
    repo.git(&[
        "config",
        "--global",
        "core.hooksPath",
        elsewhere.to_str().unwrap(),
    ]);
    // Put our hook there by hand, as though the user pointed us at it.
    repo.run(&["install", "--global", "--force"]);

    let output = repo.run(&["uninstall", "--global"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(
        repo.git(&["config", "--global", "--get", "core.hooksPath"]),
        elsewhere.to_str().unwrap(),
        "a hooks path we did not set should survive"
    );
}

#[test]
fn refuses_to_rewrite_while_a_rebase_is_in_progress() {
    let repo = TempRepo::new("clean-mid-rebase");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit(DIRTY);
    std::fs::create_dir_all(repo.path.join(".git/rebase-merge")).unwrap();

    let output = repo.run(&["clean"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(text(&output).contains("rebase"), "{}", text(&output));
}
