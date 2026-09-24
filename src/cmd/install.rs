//! `install` — put the commit-msg hook in place.
//!
//! Agents are validated before anything is written, so a typo in `--agents`
//! never leaves a half-installed repository behind.

use std::path::PathBuf;

use crate::cli::Install;
use crate::config;
use crate::error::Failure;
use crate::git::{Git, Scope};
use crate::hooks;
use crate::profiles::Rules;
use crate::ui::Style;

pub fn run(arguments: &Install) -> Result<(), Failure> {
    let git = Git::here();
    let style = Style::stdout();

    validate_agents(&arguments.agents)?;
    if !arguments.global {
        git.require_repository()?;
    }

    let (directory, claim_path) = if arguments.global {
        let (directory, claim) = hooks::global_directory(&git, arguments.force)?;
        (directory, claim)
    } else {
        (hooks::repo_directory(&git)?, false)
    };
    let hook = directory.join(hooks::HOOK_NAME);
    let binary = current_binary()?;
    let script = hooks::script(&binary);

    match existing_hook(&hook)? {
        Existing::Ours if hook_points_at(&hook, &binary)? => {
            println!("{} {}", style.green("Already installed at"), hook.display());
            return finish(&git, arguments, &directory, claim_path, &style);
        }
        Existing::Ours => {
            hooks::write(&hook, &script)?;
            println!(
                "{} {}",
                style.green("Refreshed the hook at"),
                hook.display()
            );
        }
        Existing::Foreign if !arguments.force => {
            return Err(Failure::refused(format!(
                "there is already a {} hook at {}",
                hooks::HOOK_NAME,
                hook.display()
            ))
            .fix(format!(
                "Re-run with --force to move it to {} and install ours.",
                hooks::backup_path(&hook).display()
            )));
        }
        Existing::Foreign => {
            let backup = hooks::backup_path(&hook);
            if backup.exists() {
                return Err(Failure::refused(format!(
                    "{} already exists, so the current hook cannot be backed up",
                    backup.display()
                ))
                .fix("Move or delete that file, then install again."));
            }
            std::fs::rename(&hook, &backup).map_err(|error| {
                Failure::git(format!("could not move the existing hook aside: {error}"))
            })?;
            hooks::write(&hook, &script)?;
            println!(
                "{} {}\n  {}",
                style.green("Installed at"),
                hook.display(),
                style.dim(&format!("The previous hook is at {}", backup.display()))
            );
        }
        Existing::None => {
            hooks::write(&hook, &script)?;
            println!("{} {}", style.green("Installed at"), hook.display());
        }
    }

    finish(&git, arguments, &directory, claim_path, &style)
}

/// Writes config and prints what the hook will do.
fn finish(
    git: &Git,
    arguments: &Install,
    directory: &std::path::Path,
    claim_path: bool,
    style: &Style,
) -> Result<(), Failure> {
    let scope = if arguments.global {
        Scope::Global
    } else {
        Scope::Local
    };
    if !arguments.agents.is_empty() {
        git.config_unset_all(scope, config::AGENT_KEY)?;
        for agent in &arguments.agents {
            git.config_add(scope, config::AGENT_KEY, agent)?;
        }
    }
    if claim_path {
        hooks::claim_hooks_path(git, directory)?;
        println!(
            "  {}",
            style.dim(&format!(
                "core.hooksPath now points at {}",
                directory.display()
            ))
        );
    }
    println!(
        "  {}",
        style.dim(&format!(
            "Stripping: {}",
            config::agent_names(git).join(", ")
        ))
    );
    Ok(())
}

/// What is already at the hook path.
enum Existing {
    None,
    Ours,
    Foreign,
}

fn existing_hook(hook: &std::path::Path) -> Result<Existing, Failure> {
    if !hook.exists() {
        return Ok(Existing::None);
    }
    let contents = std::fs::read_to_string(hook).map_err(|error| {
        Failure::git(format!("could not read {}: {error}", hook.display()))
            .fix("Check that the file is readable, or move it aside by hand.")
    })?;
    Ok(if hooks::is_ours(&contents) {
        Existing::Ours
    } else {
        Existing::Foreign
    })
}

/// Whether our installed hook already calls this exact binary.
fn hook_points_at(hook: &std::path::Path, binary: &std::path::Path) -> Result<bool, Failure> {
    let contents = std::fs::read_to_string(hook)
        .map_err(|error| Failure::git(format!("could not read {}: {error}", hook.display())))?;
    Ok(contents.contains(&binary.display().to_string()))
}

/// Rejects unknown agent names before anything is written.
fn validate_agents(agents: &[String]) -> Result<(), Failure> {
    Rules::from_names(agents).map(|_| ()).map_err(|unknown| {
        Failure::usage(format!("unknown agent {:?}", unknown.0))
            .fix(format!("Known agents: {}.", config::known_agents()))
    })
}

/// The absolute path of the running binary, which the hook will call.
fn current_binary() -> Result<PathBuf, Failure> {
    std::env::current_exe().map_err(|error| {
        Failure::git(format!(
            "could not work out where this binary lives: {error}"
        ))
        .fix("Install it somewhere stable, such as with `cargo install --path .`.")
    })
}
