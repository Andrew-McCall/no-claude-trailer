//! `uninstall` — take the hook away, and put back whatever it replaced.

use crate::cli::Uninstall;
use crate::error::Failure;
use crate::git::Git;
use crate::hooks;
use crate::ui::Style;

pub fn run(arguments: &Uninstall) -> Result<(), Failure> {
    let git = Git::here();
    let style = Style::stdout();

    let directory = if arguments.global {
        // Never refuses here: uninstalling only ever removes our own hook.
        hooks::global_directory(&git, true)?.0
    } else {
        git.require_repository()?;
        hooks::repo_directory(&git)?
    };
    let hook = directory.join(hooks::HOOK_NAME);

    if !hook.exists() {
        println!(
            "{} {}",
            style.dim("There is no hook to remove at"),
            hook.display()
        );
        return release_if_ours(&git, arguments, &style);
    }

    let contents = std::fs::read_to_string(&hook)
        .map_err(|error| Failure::git(format!("could not read {}: {error}", hook.display())))?;
    if !hooks::is_ours(&contents) {
        return Err(Failure::refused(format!(
            "the {} hook at {} was not installed by no-claude-trailer",
            hooks::HOOK_NAME,
            hook.display()
        ))
        .fix("Leaving it alone. Remove it by hand if you are sure."));
    }

    std::fs::remove_file(&hook)
        .map_err(|error| Failure::git(format!("could not remove {}: {error}", hook.display())))?;
    println!("{} {}", style.green("Removed"), hook.display());

    let backup = hooks::backup_path(&hook);
    if backup.exists() {
        std::fs::rename(&backup, &hook).map_err(|error| {
            Failure::git(format!("could not restore {}: {error}", backup.display())).fix(format!(
                "The previous hook is still at {}.",
                backup.display()
            ))
        })?;
        println!(
            "  {}",
            style.dim("Restored the hook that was there before.")
        );
    }

    release_if_ours(&git, arguments, &style)
}

/// Undoes `core.hooksPath` only when we are the ones who set it.
fn release_if_ours(git: &Git, arguments: &Uninstall, style: &Style) -> Result<(), Failure> {
    if !arguments.global || !hooks::owns_hooks_path(git) {
        return Ok(());
    }
    hooks::release_hooks_path(git)?;
    println!(
        "  {}",
        style.dim("core.hooksPath unset, since we were the ones who set it.")
    );
    Ok(())
}
