//! `clean` — rewrite commit messages to remove attribution.
//!
//! The order of operations is the safety story: refuse early, plan completely,
//! write objects, then move the branch last with a backup ref in place.

use crate::cli::Clean;
use crate::config;
use crate::error::Failure;
use crate::git::Git;
use crate::range;
use crate::report;
use crate::rewrite::Plan;
use crate::ui::Style;

pub fn run(arguments: &Clean) -> Result<(), Failure> {
    let git = Git::here();
    git.require_repository()?;

    if let Some(operation) = git.operation_in_progress() {
        return Err(Failure::refused(format!("{operation} is in progress"))
            .fix("Finish or abort it first, then run clean again."));
    }
    if !git.is_clean()? {
        return Err(Failure::refused("the working tree has uncommitted changes")
            .fix("Commit them, or set them aside with `git stash`, then run clean again."));
    }

    let rules = config::rules(&git)?;
    let range = range::resolve(&git, &arguments.selection)?;
    let plan = Plan::build(&git, &rules, &range)?;
    let style = Style::stdout();

    if plan.is_empty() {
        println!(
            "{} — nothing to clean.",
            style.green(&format!("Checked {}", range.description))
        );
        return Ok(());
    }

    if arguments.dry_run {
        print_dry_run(&plan, &style);
        return Ok(());
    }

    let applied = plan.apply(&git)?;
    println!(
        "{} on {}",
        style.green(&format!("Rewrote {}", commits(applied.rewritten()))),
        plan.branch_name()
    );
    println!(
        "{}",
        report::rewritten_table(&plan.dirty(), &applied.mapping, &style)
    );
    if applied.resigned > 0 {
        println!(
            "{}",
            style.yellow(&format!(
                "Re-signed {} with your signing key.",
                commits(applied.resigned)
            ))
        );
    }
    for reference in &plan.stranded_refs {
        println!(
            "{}",
            style.yellow(&format!("{reference} still points at the old commits."))
        );
    }
    println!(
        "Backup at {}\n  {}",
        style.dim(&applied.backup_ref),
        style.dim(&format!(
            "Undo with `git reset --hard {}`",
            applied.backup_ref
        ))
    );
    Ok(())
}

fn print_dry_run(plan: &Plan, style: &Style) {
    println!(
        "{} on {}",
        style.bold(&format!("Would rewrite {}", commits(plan.rebuild_count()))),
        plan.branch_name()
    );
    for step in plan.steps.iter().filter(|step| step.finding.is_dirty()) {
        println!(
            "  {}  {}",
            style.yellow(step.finding.short_id()),
            step.finding.subject
        );
        for hit in &step.finding.hits {
            println!("    {}", style.red(&format!("- {}", hit.line)));
        }
    }
    for reference in &plan.stranded_refs {
        println!(
            "{}",
            style.yellow(&format!(
                "{reference} would be left pointing at the old commits."
            ))
        );
    }
    println!("{}", style.dim("Nothing has been changed."));
}

/// Pluralises a commit count, because "1 commits" reads badly.
fn commits(count: usize) -> String {
    if count == 1 {
        "1 commit".to_string()
    } else {
        format!("{count} commits")
    }
}
