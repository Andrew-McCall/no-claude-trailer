//! `setup` — the guided way in.
//!
//! The wizard collects every answer before anything is written, and this
//! module is what turns those answers into actions. Without a terminal it
//! refuses and names the flags that do the same job, rather than applying
//! defaults to somebody's machine unasked.

use std::io::IsTerminal;

use crate::cli::{Clean, Install};
use crate::cmd;
use crate::config;
use crate::error::Failure;
use crate::git::{Git, Scope};
use crate::range::Selection;
use crate::state::State;
use crate::ui::Style;
use crate::wizard::{self, Answers, Place};

pub fn run() -> Result<(), Failure> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Err(
            Failure::usage("setup asks questions, so it needs a terminal").fix(
                "Do it non-interactively instead, for example \
                 `git no-claude-trailer install --global --agents=claude,copilot`.",
            ),
        );
    }

    let git = Git::here();
    let state = State::gather(&git);
    let mut input = stdin.lock();
    let mut output = std::io::stdout();

    let Some(answers) = wizard::ask(&mut input, &mut output, &state)? else {
        return Ok(()); // the wizard has already said it changed nothing
    };
    apply(&git, &answers)
}

fn apply(git: &Git, answers: &Answers) -> Result<(), Failure> {
    let style = Style::stdout();
    match answers.place {
        Place::Repo | Place::Global => cmd::install::run(&Install {
            global: answers.place == Place::Global,
            force: false,
            agents: answers.agents.clone(),
        })?,
        Place::ConfigOnly => record_agents(git, &answers.agents)?,
    }

    if answers.alias {
        // `git nct` resolves to this same subcommand.
        git.config_set(Scope::Global, "alias.nct", "no-claude-trailer")?;
        println!("  {}", style.dim("`git nct` now runs no-claude-trailer."));
    }

    if answers.clean_now {
        cmd::clean::run(&Clean {
            selection: Selection::Unpushed,
            dry_run: false,
        })?;
    }

    println!(
        "\n{}",
        style.dim("Undo any of this with `git no-claude-trailer uninstall`.")
    );
    Ok(())
}

/// Writes the agent list without installing a hook.
fn record_agents(git: &Git, agents: &[String]) -> Result<(), Failure> {
    let scope = if git.in_repository() {
        Scope::Local
    } else {
        Scope::Global
    };
    git.config_unset_all(scope, config::AGENT_KEY)?;
    for agent in agents {
        git.config_add(scope, config::AGENT_KEY, agent)?;
    }
    println!("Recorded: {}", agents.join(", "));
    Ok(())
}
