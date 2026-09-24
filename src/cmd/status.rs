//! `status` — what is set up here, and what to do next.
//!
//! This is also what a bare `git no-claude-trailer` prints, so it has to answer
//! the question a newcomer actually has: is this on, and for what?

use crate::error::Failure;
use crate::git::Git;
use crate::state::{Hook, State};
use crate::ui::Style;

pub fn run(with_hint: bool) -> Result<(), Failure> {
    let git = Git::here();
    let state = State::gather(&git);
    let style = Style::stdout();

    println!("{}", style.bold("no-claude-trailer"));

    match &state.repository {
        Some(path) => {
            let branch = state
                .branch
                .clone()
                .unwrap_or_else(|| "a detached HEAD".to_string());
            println!("  {:<10} {} on {branch}", "repo", path.display());
        }
        None => println!("  {:<10} not a git repository", "repo"),
    }

    match &state.binary {
        Some(path) => println!("  {:<10} {}", "binary", path.display()),
        None => println!("  {:<10} unknown", "binary"),
    }

    match &state.hook {
        Hook::Missing => println!("  {:<10} {}", "hook", style.yellow("not installed")),
        Hook::Ours(path) => println!(
            "  {:<10} {}",
            "hook",
            style.green(&path.display().to_string())
        ),
        Hook::OursGlobal(path) => println!(
            "  {:<10} {} {}",
            "hook",
            style.green(&path.display().to_string()),
            style.dim("(every repository)")
        ),
        Hook::Foreign(path) => println!(
            "  {:<10} {}",
            "hook",
            style.yellow(&format!("{} belongs to something else", path.display()))
        ),
    }

    println!("  {:<10} {}", "stripping", state.agents.join(", "));

    match state.unpushed {
        Some((total, 0)) => println!(
            "  {:<10} {total} unpushed, none carry attribution",
            "commits"
        ),
        Some((total, dirty)) => println!(
            "  {:<10} {total} unpushed, {}",
            "commits",
            style.yellow(&format!("{dirty} carry attribution"))
        ),
        None => println!(
            "  {:<10} {}",
            "commits",
            style.dim("no upstream to compare against")
        ),
    }

    if with_hint {
        println!("\n{}", style.dim(&hint(&state)));
    }
    Ok(())
}

/// The single most useful next step, given what we found.
fn hint(state: &State) -> String {
    if !state.is_installed() {
        return "Run `git no-claude-trailer setup` to get started.".to_string();
    }
    match state.unpushed {
        Some((_, dirty)) if dirty > 0 => {
            "Run `git no-claude-trailer clean` to tidy the commits above.".to_string()
        }
        _ => "Nothing to do. Run `git no-claude-trailer help` for the rest.".to_string(),
    }
}
