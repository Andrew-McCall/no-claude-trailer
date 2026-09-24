//! `status` answers "what is set up here?", and is what a bare invocation shows.

mod support;

use support::{TempRepo, text};

const DIRTY: &str = "Fix pagination\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n";

#[test]
fn reports_that_no_hook_is_installed() {
    let repo = TempRepo::new("status-none");
    let output = repo.run(&["status"]);
    assert!(output.status.success(), "{}", text(&output));
    let shown = text(&output);
    assert!(shown.contains("not installed"), "{shown}");
    assert!(
        shown.contains("claude"),
        "should name the agents in force: {shown}"
    );
}

#[test]
fn reports_the_installed_hook() {
    let repo = TempRepo::new("status-installed");
    repo.run(&["install"]);
    let shown = text(&repo.run(&["status"]));
    assert!(shown.contains("commit-msg"), "{shown}");
    assert!(!shown.contains("not installed"), "{shown}");
}

#[test]
fn counts_unpushed_commits_that_carry_attribution() {
    let repo = TempRepo::new("status-dirty");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit(DIRTY);
    repo.commit("Clean commit\n");
    let shown = text(&repo.run(&["status"]));
    assert!(
        shown.contains('2'),
        "should count the unpushed commits: {shown}"
    );
    assert!(shown.contains('1'), "should count the dirty ones: {shown}");
}

#[test]
fn says_when_there_is_no_upstream_to_measure_against() {
    let repo = TempRepo::new("status-no-upstream");
    repo.commit("First commit\n");
    let shown = text(&repo.run(&["status"]));
    assert!(shown.contains("upstream"), "{shown}");
}

#[test]
fn lists_the_configured_agents() {
    let repo = TempRepo::new("status-agents");
    repo.git(&["config", "--add", "noclaudetrailer.agent", "claude"]);
    repo.git(&["config", "--add", "noclaudetrailer.agent", "cursor"]);
    let shown = text(&repo.run(&["status"]));
    assert!(shown.contains("cursor"), "{shown}");
}

#[test]
fn works_outside_a_repository() {
    let repo = TempRepo::new("status-outside");
    let outside = repo.root().join("not-a-repo");
    std::fs::create_dir_all(&outside).unwrap();
    let output = std::process::Command::new(support::binary())
        .arg("status")
        .current_dir(&outside)
        .env("GIT_CONFIG_GLOBAL", repo.global_config())
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CEILING_DIRECTORIES", repo.root())
        .env("HOME", repo.home())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("not a git repository"),
        "{}",
        text(&output)
    );
}

#[test]
fn a_bare_invocation_shows_status_and_a_hint() {
    let repo = TempRepo::new("status-bare");
    let output = repo.run(&[]);
    assert!(output.status.success(), "{}", text(&output));
    let shown = text(&output);
    assert!(shown.contains("not installed"), "{shown}");
    assert!(
        shown.contains("setup"),
        "should point at the next step: {shown}"
    );
}

#[test]
fn setup_refuses_without_a_terminal_and_names_the_flags() {
    let repo = TempRepo::new("setup-no-tty");
    let output = repo.run_with_stdin(&["setup"], "");
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    let shown = text(&output);
    assert!(
        shown.contains("install"),
        "should name the non-interactive command: {shown}"
    );
    assert!(shown.contains("--agents"), "{shown}");
}
