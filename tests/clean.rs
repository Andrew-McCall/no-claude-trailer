//! `clean` rewrites commit messages in place, preserving everything else.

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

const CLEANED: &str = "Fix pagination\n\nThe cursor was off by one.";

#[test]
fn rewrites_unpushed_commits() {
    let repo = TempRepo::new("clean-basic");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit(DIRTY);
    let output = repo.run(&["clean"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(repo.messages()[0], CLEANED);
}

#[test]
fn leaves_published_commits_alone_by_default() {
    let repo = TempRepo::new("clean-published");
    repo.commit(DIRTY);
    repo.with_upstream();
    let before = repo.git(&["rev-parse", "HEAD"]);
    let output = repo.run(&["clean"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), before);
    assert!(text(&output).contains("nothing"), "{}", text(&output));
}

#[test]
fn rewrites_published_history_when_asked() {
    let repo = TempRepo::new("clean-all");
    repo.commit(DIRTY);
    repo.with_upstream();
    assert!(repo.run(&["clean", "--all"]).status.success());
    assert_eq!(repo.messages()[0], CLEANED);
}

#[test]
fn preserves_author_and_committer_dates_exactly() {
    let repo = TempRepo::new("clean-dates");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit(DIRTY);
    let before = repo.git(&["log", "-1", "--format=%an|%ae|%at|%ai|%cn|%ce|%ct|%ci"]);
    assert!(repo.run(&["clean"]).status.success());
    let after = repo.git(&["log", "-1", "--format=%an|%ae|%at|%ai|%cn|%ce|%ct|%ci"]);
    assert_eq!(before, after, "identities and dates must survive a rewrite");
}

#[test]
fn preserves_the_tree() {
    let repo = TempRepo::new("clean-tree");
    repo.commit("First commit\n");
    repo.with_upstream();
    std::fs::write(repo.path.join("file.txt"), "contents\n").unwrap();
    repo.git(&["add", "file.txt"]);
    std::fs::write(repo.path.join("msg"), DIRTY).unwrap();
    repo.git(&["commit", "--quiet", "--no-verify", "--file", "msg"]);
    std::fs::remove_file(repo.path.join("msg")).unwrap();
    let before = repo.git(&["rev-parse", "HEAD^{tree}"]);
    assert!(repo.run(&["clean"]).status.success());
    assert_eq!(repo.git(&["rev-parse", "HEAD^{tree}"]), before);
    assert_eq!(
        std::fs::read_to_string(repo.path.join("file.txt")).unwrap(),
        "contents\n"
    );
}

#[test]
fn preserves_merge_topology() {
    let repo = TempRepo::new("clean-merge");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.git(&["checkout", "--quiet", "-b", "side"]);
    repo.commit("Side work\n");
    repo.git(&["checkout", "--quiet", "main"]);
    repo.commit("Main work\n");
    std::fs::write(repo.path.join("msg"), DIRTY).unwrap();
    repo.git(&[
        "merge",
        "--quiet",
        "--no-ff",
        "--no-verify",
        "-F",
        "msg",
        "side",
    ]);
    std::fs::remove_file(repo.path.join("msg")).unwrap();
    let parents_before = repo.git(&["log", "-1", "--format=%p"]).split(' ').count();
    assert_eq!(parents_before, 2, "the test needs a real merge commit");

    let output = repo.run(&["clean"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(repo.messages()[0], CLEANED);
    assert_eq!(
        repo.git(&["log", "-1", "--format=%p"]).split(' ').count(),
        2
    );
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "4");
}

#[test]
fn leaves_clean_commits_with_their_original_id() {
    let repo = TempRepo::new("clean-untouched");
    repo.commit("First commit\n");
    repo.with_upstream();
    let untouched = repo.commit("Second commit\n");
    repo.commit(DIRTY);
    assert!(repo.run(&["clean"]).status.success());
    assert_eq!(
        repo.git(&["rev-parse", "HEAD~1"]),
        untouched,
        "a clean commit with clean ancestors keeps its id"
    );
}

#[test]
fn saves_a_backup_ref_that_restores_the_branch() {
    let repo = TempRepo::new("clean-backup");
    repo.commit("First commit\n");
    repo.with_upstream();
    let before = repo.commit(DIRTY);
    let output = repo.run(&["clean"]);
    let shown = text(&output);
    assert!(shown.contains("refs/no-claude-trailer/backup/"), "{shown}");

    let backup = repo.git(&[
        "for-each-ref",
        "--format=%(refname)",
        "refs/no-claude-trailer/backup",
    ]);
    assert!(!backup.is_empty(), "a backup ref should exist");
    assert_eq!(repo.git(&["rev-parse", &backup]), before);

    // The undo path the output advertises has to actually work.
    repo.git(&["reset", "--hard", "--quiet", &backup]);
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), before);
}

#[test]
fn the_summary_names_the_commits_that_now_exist() {
    let repo = TempRepo::new("clean-new-ids");
    repo.commit("First commit\n");
    repo.with_upstream();
    let old = repo.commit(DIRTY);
    let output = repo.run(&["clean"]);
    assert!(output.status.success(), "{}", text(&output));

    let new = repo.git(&["rev-parse", "HEAD"]);
    let shown = text(&output);
    assert!(
        shown.contains(&new[..7]),
        "the summary should name the commit that now exists ({}): {shown}",
        &new[..7]
    );
    assert!(
        shown.contains(&old[..7]),
        "and the one it replaced ({}): {shown}",
        &old[..7]
    );
}

#[test]
fn dry_run_changes_nothing_and_shows_the_difference() {
    let repo = TempRepo::new("clean-dry-run");
    repo.commit("First commit\n");
    repo.with_upstream();
    let before = repo.commit(DIRTY);
    let output = repo.run(&["clean", "--dry-run"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(
        repo.git(&["rev-parse", "HEAD"]),
        before,
        "dry run must not move anything"
    );
    assert!(
        repo.git(&[
            "for-each-ref",
            "--format=%(refname)",
            "refs/no-claude-trailer/backup"
        ])
        .is_empty(),
        "dry run must not create a backup ref"
    );
    let shown = text(&output);
    assert!(
        shown.contains("Co-Authored-By: Claude"),
        "the removed lines should be shown: {shown}"
    );
    assert!(
        shown.to_lowercase().contains("would"),
        "the wording should stay conditional: {shown}"
    );
}

#[test]
fn refuses_when_the_working_tree_is_dirty() {
    let repo = TempRepo::new("clean-dirty-tree");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit(DIRTY);
    std::fs::write(repo.path.join("scratch.txt"), "work in progress\n").unwrap();
    repo.git(&["add", "scratch.txt"]);
    let output = repo.run(&["clean"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    let shown = text(&output);
    assert!(shown.contains("uncommitted"), "{shown}");
    assert!(
        shown.contains("stash"),
        "the fix should offer a way out: {shown}"
    );
}

#[test]
fn reports_when_there_is_nothing_to_do() {
    let repo = TempRepo::new("clean-nothing");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit("Second commit\n");
    let output = repo.run(&["clean"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("nothing"), "{}", text(&output));
}

#[test]
fn refuses_a_range_that_does_not_end_at_head() {
    let repo = TempRepo::new("clean-range-tip");
    let first = repo.commit("First commit\n");
    repo.commit(DIRTY);
    repo.commit("Third commit\n");
    let output = repo.run(&["clean", "--range", &format!("{first}..HEAD~1")]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(text(&output).contains("HEAD"), "{}", text(&output));
}

#[test]
fn refuses_a_commit_whose_message_is_only_attribution() {
    let repo = TempRepo::new("clean-empty-message");
    repo.commit("First commit\n");
    repo.with_upstream();
    let before = repo.git(&["rev-parse", "HEAD"]);
    std::fs::write(
        repo.path.join("msg"),
        "\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n",
    )
    .unwrap();
    repo.git(&[
        "commit",
        "--allow-empty",
        "--quiet",
        "--no-verify",
        "--allow-empty-message",
        "--cleanup=verbatim",
        "--file",
        "msg",
    ]);
    std::fs::remove_file(repo.path.join("msg")).unwrap();
    let dirty_head = repo.git(&["rev-parse", "HEAD"]);

    let output = repo.run(&["clean"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert_eq!(
        repo.git(&["rev-parse", "HEAD"]),
        dirty_head,
        "nothing should move"
    );
    assert_ne!(dirty_head, before);
    assert!(text(&output).contains("nothing"), "{}", text(&output));
}

#[test]
fn warns_about_other_branches_pointing_into_the_range() {
    let repo = TempRepo::new("clean-other-refs");
    repo.commit("First commit\n");
    repo.with_upstream();
    repo.commit(DIRTY);
    repo.git(&["branch", "keepsake"]);
    let output = repo.run(&["clean"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("keepsake"), "{}", text(&output));
}

#[test]
fn works_on_a_detached_head_and_says_so() {
    let repo = TempRepo::new("clean-detached");
    repo.commit("First commit\n");
    repo.commit(DIRTY);
    repo.git(&["checkout", "--quiet", "--detach"]);
    let output = repo.run(&["clean", "--all"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(repo.messages()[0], CLEANED);
    assert!(text(&output).contains("detached"), "{}", text(&output));
}

#[test]
fn refuses_a_commit_with_a_non_utf8_encoding_header() {
    let repo = TempRepo::new("clean-encoding");
    repo.commit("First commit\n");
    repo.with_upstream();
    std::fs::write(repo.path.join("msg"), DIRTY).unwrap();
    repo.git(&[
        "-c",
        "i18n.commitEncoding=ISO-8859-1",
        "commit",
        "--allow-empty",
        "--quiet",
        "--no-verify",
        "--file",
        "msg",
    ]);
    std::fs::remove_file(repo.path.join("msg")).unwrap();
    let before = repo.git(&["rev-parse", "HEAD"]);

    let output = repo.run(&["clean"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert_eq!(
        repo.git(&["rev-parse", "HEAD"]),
        before,
        "nothing should move"
    );
    assert!(text(&output).contains("encoding"), "{}", text(&output));
}
