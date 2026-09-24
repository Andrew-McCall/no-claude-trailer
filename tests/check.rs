//! `check` reports attribution without changing anything, for use as a gate.

mod support;

use support::{TempRepo, text};

const DIRTY: &str = concat!(
    "Fix pagination\n",
    "\n",
    "🤖 Generated with [Claude Code](https://claude.com/claude-code)\n",
    "\n",
    "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n",
);

#[test]
fn succeeds_when_nothing_is_attributed() {
    let repo = TempRepo::new("check-clean");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit("Second commit\n");
    let output = repo.run(&["check"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(
        text(&output).contains("no AI attribution"),
        "{}",
        text(&output)
    );
}

#[test]
fn exits_one_and_lists_offending_commits() {
    let repo = TempRepo::new("check-dirty");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit(DIRTY);
    repo.commit("Clean commit\n");
    let output = repo.run(&["check"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    let shown = text(&output);
    assert!(shown.contains("Fix pagination"), "{shown}");
    assert!(shown.contains("Co-Authored-By"), "{shown}");
    assert!(shown.contains("Generated with [Claude Code]"), "{shown}");
    assert!(
        !shown.contains("Clean commit"),
        "clean commits should not be listed: {shown}"
    );
}

#[test]
fn only_looks_at_unpushed_commits_by_default() {
    let repo = TempRepo::new("check-unpushed");
    repo.commit(DIRTY);
    repo.with_upstream();
    let output = repo.run(&["check"]);
    assert!(
        output.status.success(),
        "a published commit is not our business by default: {}",
        text(&output)
    );
}

#[test]
fn checks_whole_history_with_all() {
    let repo = TempRepo::new("check-all");
    repo.commit(DIRTY);
    repo.with_upstream();
    let output = repo.run(&["check", "--all"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
}

#[test]
fn accepts_an_explicit_range() {
    let repo = TempRepo::new("check-range");
    let first = repo.commit("First commit\n");
    repo.commit(DIRTY);
    let output = repo.run(&["check", "--range", &first]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
}

#[test]
fn accepts_a_two_dot_range() {
    let repo = TempRepo::new("check-two-dot");
    let first = repo.commit("First commit\n");
    repo.commit(DIRTY);
    let output = repo.run(&["check", "--range", &format!("{first}..HEAD")]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
}

#[test]
fn explains_itself_when_there_is_no_upstream() {
    let repo = TempRepo::new("check-no-upstream");
    repo.commit("First commit\n");
    let output = repo.run(&["check"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    let shown = text(&output);
    assert!(shown.contains("upstream"), "{shown}");
    assert!(
        shown.contains("--all"),
        "the fix should name the way forward: {shown}"
    );
}

#[test]
fn refuses_outside_a_repository() {
    let repo = TempRepo::new("check-no-repo");
    let outside = repo.path.parent().unwrap().join("not-a-repo");
    std::fs::create_dir_all(&outside).unwrap();
    let output = std::process::Command::new(support::binary())
        .arg("check")
        .current_dir(&outside)
        .env("GIT_CONFIG_GLOBAL", repo.global_config())
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CEILING_DIRECTORIES", repo.root())
        .env("HOME", repo.home())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(
        text(&output).contains("not a git repository"),
        "{}",
        text(&output)
    );
}

#[test]
fn rejects_an_unresolvable_range() {
    let repo = TempRepo::new("check-bad-range");
    repo.commit("First commit\n");
    let output = repo.run(&["check", "--range", "no-such-revision"]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    assert!(
        text(&output).contains("no-such-revision"),
        "{}",
        text(&output)
    );
}

#[test]
fn rejects_all_and_range_together() {
    let repo = TempRepo::new("check-both-ranges");
    repo.commit("First commit\n");
    let output = repo.run(&["check", "--all", "--range", "HEAD~1"]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
}
