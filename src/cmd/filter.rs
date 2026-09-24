//! `filter` — strip attribution from a message file or stdin.
//!
//! This is the primitive the commit-msg hook runs, so its failure messages
//! matter: when it fails the user's commit is refused, and the message on
//! screen has to be enough to fix that without going looking.

use std::io::{Read, Write};

use crate::cli::Filter;
use crate::config;
use crate::error::Failure;
use crate::git::Git;
use crate::message::{self, Options, StripError};

pub fn run(arguments: &Filter) -> Result<(), Failure> {
    let git = Git::here();
    let rules = config::rules(&git)?;
    let options = Options::message_file(&git.comment_prefix());

    let input = match &arguments.path {
        Some(path) => std::fs::read_to_string(path).map_err(|error| {
            Failure::git(format!("could not read {}: {error}", path.display())).fix(
                "Check the path. A commit-msg hook is given this path by git, so if you \
                 are seeing this from a hook, reinstall it with `git no-claude-trailer install --force`.",
            )
        })?,
        None => read_stdin()?,
    };

    let stripped = message::strip(&input, &rules, &options).map_err(|error| match error {
        StripError::NothingLeft(_) => {
            Failure::refused("this message has nothing in it but AI attribution")
                .fix("Write a commit message describing the change, then commit again.")
        }
    })?;

    match (&arguments.path, arguments.in_place) {
        (Some(path), true) => {
            if stripped.changed() {
                std::fs::write(path, &stripped.message).map_err(|error| {
                    Failure::git(format!("could not write {}: {error}", path.display()))
                        .fix("Check that the file is writable.")
                })?;
            }
            Ok(())
        }
        _ => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(stripped.message.as_bytes())
                .map_err(|error| Failure::git(format!("could not write to stdout: {error}")))
        }
    }
}

fn read_stdin() -> Result<String, Failure> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| Failure::git(format!("could not read the message from stdin: {error}")))?;
    Ok(input)
}
