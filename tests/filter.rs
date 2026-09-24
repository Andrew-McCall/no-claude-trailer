//! `filter` is the primitive the commit-msg hook runs.

mod support;

use support::{TempRepo, text};

const DIRTY: &str = concat!(
    "Fix pagination\n",
    "\n",
    "The cursor was off by one.\n",
    "\n",
    "🤖 Generated with [Claude Code](https://claude.com/claude-code)\n",
    "\n",
    "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n",
);

const CLEAN: &str = "Fix pagination\n\nThe cursor was off by one.\n";

#[test]
fn strips_a_message_on_stdin() {
    let repo = TempRepo::new("filter-stdin");
    let output = repo.run_with_stdin(&["filter"], DIRTY);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(String::from_utf8_lossy(&output.stdout), CLEAN);
}

#[test]
fn leaves_a_clean_message_untouched() {
    let repo = TempRepo::new("filter-clean");
    let output = repo.run_with_stdin(&["filter"], CLEAN);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(String::from_utf8_lossy(&output.stdout), CLEAN);
}

#[test]
fn strips_a_message_file_in_place() {
    let repo = TempRepo::new("filter-in-place");
    let file = repo.path.join("COMMIT_EDITMSG");
    std::fs::write(&file, DIRTY).unwrap();
    let output = repo.run(&["filter", "--in-place", file.to_str().unwrap()]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), CLEAN);
}

#[test]
fn reads_a_message_file_without_writing_it() {
    let repo = TempRepo::new("filter-read-file");
    let file = repo.path.join("COMMIT_EDITMSG");
    std::fs::write(&file, DIRTY).unwrap();
    let output = repo.run(&["filter", file.to_str().unwrap()]);
    assert_eq!(String::from_utf8_lossy(&output.stdout), CLEAN);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        DIRTY,
        "file must be untouched"
    );
}

#[test]
fn refuses_a_message_that_is_only_attribution() {
    let repo = TempRepo::new("filter-empty");
    let output = repo.run_with_stdin(
        &["filter"],
        "\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n",
    );
    assert!(!output.status.success());
    let message = text(&output);
    assert!(
        message.contains("nothing"),
        "error should explain the refusal: {message}"
    );
}

#[test]
fn fails_loudly_when_the_message_file_is_missing() {
    let repo = TempRepo::new("filter-missing");
    let output = repo.run(&["filter", "--in-place", "does-not-exist"]);
    assert!(!output.status.success());
    assert!(text(&output).contains("does-not-exist"));
}

#[test]
fn honours_configured_extra_agents() {
    let repo = TempRepo::new("filter-agents");
    repo.git(&["config", "--add", "noclaudetrailer.agent", "claude"]);
    repo.git(&["config", "--add", "noclaudetrailer.agent", "copilot"]);
    let output = repo.run_with_stdin(
        &["filter"],
        "Subject\n\nCo-Authored-By: Copilot <copilot@github.com>\n",
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Subject\n");
}

#[test]
fn ignores_copilot_when_only_claude_is_enabled() {
    let repo = TempRepo::new("filter-default-agents");
    let message = "Subject\n\nCo-Authored-By: Copilot <copilot@github.com>\n";
    let output = repo.run_with_stdin(&["filter"], message);
    assert_eq!(String::from_utf8_lossy(&output.stdout), message);
}

#[test]
fn honours_a_configured_extra_needle() {
    let repo = TempRepo::new("filter-needle");
    repo.git(&[
        "config",
        "--add",
        "noclaudetrailer.needle",
        "my-internal-agent",
    ]);
    let output = repo.run_with_stdin(&["filter"], "Subject\n\nBuilt-By: My-Internal-Agent\n");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Subject\n");
}

#[test]
fn honours_a_configured_extra_trailer_key() {
    let repo = TempRepo::new("filter-key");
    repo.git(&[
        "config",
        "--add",
        "noclaudetrailer.trailerkey",
        "X-Agent-Run",
    ]);
    let output = repo.run_with_stdin(&["filter"], "Subject\n\nX-Agent-Run: 42\n");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Subject\n");
}

#[test]
fn respects_a_custom_comment_prefix_from_git_config() {
    let repo = TempRepo::new("filter-comment-char");
    repo.git(&["config", "core.commentChar", ";"]);
    let message = "Subject\n; Generated with [Claude Code](https://claude.com/claude-code)\n";
    let output = repo.run_with_stdin(&["filter"], message);
    assert_eq!(String::from_utf8_lossy(&output.stdout), message);
}
