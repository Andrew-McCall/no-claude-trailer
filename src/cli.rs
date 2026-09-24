//! Command line parsing.
//!
//! Hand-rolled, both to stay dependency free and so `--help` can be written as
//! prose with worked examples rather than generated from flag declarations.
//! The help text here is the documentation of record; the README repeats it.

use std::path::PathBuf;

use crate::error::Failure;
use crate::range::Selection;

/// What the user asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Report attribution without changing anything.
    Check(Selection),
    /// Rewrite commit messages to remove attribution.
    Clean(Clean),
    /// Put the commit-msg hook in place.
    Install(Install),
    /// Show what is set up here. `true` adds the next-step hint.
    Status(bool),
    /// Ask what is wanted, then do it.
    Setup,
    /// Take the commit-msg hook away.
    Uninstall(Uninstall),
    /// Strip a message file or stdin.
    Filter(Filter),
    /// Print help, for everything or for one command.
    Help(Option<String>),
    /// Print the version.
    Version,
}

/// Arguments for `install`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Install {
    /// Install for every repository by setting `core.hooksPath`.
    pub global: bool,
    /// Replace an existing hook, keeping a backup.
    pub force: bool,
    /// Agents to record in config; empty leaves config alone.
    pub agents: Vec<String>,
}

/// Arguments for `uninstall`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Uninstall {
    /// Undo a machine-wide install.
    pub global: bool,
}

/// Arguments for `clean`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Clean {
    /// Which commits to rewrite.
    pub selection: Selection,
    /// Show what would change without changing it.
    pub dry_run: bool,
}

/// Arguments for `filter`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// The message file, or stdin when absent.
    pub path: Option<PathBuf>,
    /// Rewrite the file in place instead of writing to stdout.
    pub in_place: bool,
}

/// Parses arguments, excluding the program name.
pub fn parse<I: IntoIterator<Item = String>>(arguments: I) -> Result<Command, Failure> {
    let arguments: Vec<String> = arguments.into_iter().collect();
    let Some((name, rest)) = arguments.split_first() else {
        // A bare invocation should say something useful, not print a wall of help.
        return Ok(Command::Status(true));
    };

    match name.as_str() {
        "-h" | "--help" | "help" => Ok(Command::Help(rest.first().cloned())),
        "-V" | "--version" | "version" => Ok(Command::Version),
        "check" => Ok(Command::Check(parse_selection("check", rest)?)),
        "clean" => parse_clean(rest),
        "install" => parse_install(rest),
        "status" => parse_no_arguments("status", rest).map(|()| Command::Status(false)),
        "setup" => parse_no_arguments("setup", rest).map(|()| Command::Setup),
        "uninstall" => parse_uninstall(rest),
        "filter" => parse_filter(rest),
        other => Err(unknown_command(other)),
    }
}

fn parse_filter(arguments: &[String]) -> Result<Command, Failure> {
    let mut filter = Filter::default();
    for argument in arguments {
        match argument.as_str() {
            "--in-place" | "-i" => filter.in_place = true,
            "-h" | "--help" => return Ok(Command::Help(Some("filter".to_string()))),
            flag if flag.starts_with('-') => return Err(unknown_flag("filter", flag)),
            path if filter.path.is_none() => filter.path = Some(PathBuf::from(path)),
            extra => {
                return Err(Failure::usage(format!(
                    "filter takes one message file, not two ({extra})"
                ))
                .fix("Run `git no-claude-trailer filter <file>`."));
            }
        }
    }
    if filter.in_place && filter.path.is_none() {
        return Err(Failure::usage("filter --in-place needs a message file to rewrite")
            .fix("Run `git no-claude-trailer filter --in-place <file>`, or drop --in-place to write to stdout."));
    }
    Ok(Command::Filter(filter))
}

fn parse_install(arguments: &[String]) -> Result<Command, Failure> {
    let mut install = Install::default();
    for argument in arguments {
        match argument.as_str() {
            "--global" => install.global = true,
            "--force" | "-f" => install.force = true,
            "-h" | "--help" => return Ok(Command::Help(Some("install".to_string()))),
            other => match other.strip_prefix("--agents=") {
                Some(list) => install.agents = agent_list(list),
                None => return Err(unknown_flag("install", other)),
            },
        }
    }
    Ok(Command::Install(install))
}

fn parse_uninstall(arguments: &[String]) -> Result<Command, Failure> {
    let mut uninstall = Uninstall::default();
    for argument in arguments {
        match argument.as_str() {
            "--global" => uninstall.global = true,
            "-h" | "--help" => return Ok(Command::Help(Some("uninstall".to_string()))),
            other => return Err(unknown_flag("uninstall", other)),
        }
    }
    Ok(Command::Uninstall(uninstall))
}

/// For commands that take nothing at all.
fn parse_no_arguments(command: &str, arguments: &[String]) -> Result<(), Failure> {
    match arguments.first() {
        None => Ok(()),
        Some(argument) if argument == "-h" || argument == "--help" => Ok(()),
        Some(argument) => Err(unknown_flag(command, argument)),
    }
}

/// Splits `--agents=a,b` into names, ignoring empties.
fn agent_list(list: &str) -> Vec<String> {
    list.split(',')
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

fn parse_clean(arguments: &[String]) -> Result<Command, Failure> {
    let mut dry_run = false;
    let mut rest = Vec::new();
    for argument in arguments {
        match argument.as_str() {
            "--dry-run" | "-n" => dry_run = true,
            "-h" | "--help" => return Ok(Command::Help(Some("clean".to_string()))),
            other => rest.push(other.to_string()),
        }
    }
    Ok(Command::Clean(Clean {
        selection: parse_selection("clean", &rest)?,
        dry_run,
    }))
}

/// Parses the `--range`/`--all` pair shared by the commands that walk history.
fn parse_selection(command: &str, arguments: &[String]) -> Result<Selection, Failure> {
    let mut selection = Selection::Unpushed;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        match argument {
            "--all" => set_selection(&mut selection, Selection::All, command)?,
            "--range" => {
                let revision = arguments.get(index + 1).ok_or_else(|| {
                    Failure::usage("--range needs a revision").fix(format!(
                        "Try `git no-claude-trailer {command} --range HEAD~3`."
                    ))
                })?;
                set_selection(
                    &mut selection,
                    Selection::Explicit(revision.clone()),
                    command,
                )?;
                index += 1;
            }
            flag if flag.starts_with('-') => return Err(unknown_flag(command, flag)),
            positional => {
                return Err(Failure::usage(format!(
                    "{command} does not take a bare revision ({positional})"
                ))
                .fix(format!(
                    "Name it explicitly: `git no-claude-trailer {command} --range {positional}`."
                )));
            }
        }
        index += 1;
    }
    Ok(selection)
}

/// Refuses two range flags rather than silently letting one win.
fn set_selection(current: &mut Selection, next: Selection, command: &str) -> Result<(), Failure> {
    if *current != Selection::Unpushed {
        return Err(
            Failure::usage("--all and --range choose different commits").fix(format!(
                "Pass one or the other to `git no-claude-trailer {command}`."
            )),
        );
    }
    *current = next;
    Ok(())
}

fn unknown_command(name: &str) -> Failure {
    Failure::usage(format!("there is no {name:?} command"))
        .fix("Run `git no-claude-trailer help` to see what there is.")
}

fn unknown_flag(command: &str, flag: &str) -> Failure {
    Failure::usage(format!("{command} does not take {flag}"))
        .fix(format!("Run `git no-claude-trailer help {command}`."))
}

/// The text printed by `help`.
pub fn help(topic: Option<&str>) -> String {
    match topic {
        Some("check") => CHECK_HELP.to_string(),
        Some("clean") => CLEAN_HELP.to_string(),
        Some("setup") => SETUP_HELP.to_string(),
        Some("install") => INSTALL_HELP.to_string(),
        Some("uninstall") => UNINSTALL_HELP.to_string(),
        Some("filter") => FILTER_HELP.to_string(),
        Some(unknown) => format!("There is no help for {unknown:?}.\n\n{OVERVIEW}"),
        None => OVERVIEW.to_string(),
    }
}

const OVERVIEW: &str = "\
no-claude-trailer — keep AI attribution out of your commit messages.

Usage: git no-claude-trailer <command> [options]

Commands:
  setup       Ask what you want, then set it up
  status      Show what is set up here
  install     Put the commit-msg hook in place
  uninstall   Take the commit-msg hook away
  check       Report attribution without changing anything
  clean       Rewrite commits to remove attribution
  filter      Strip attribution from a message file or stdin
  help        Show help for a command
  version     Show the version

Run `git no-claude-trailer help <command>` for detail.";

const FILTER_HELP: &str = "\
Strip attribution from a commit message.

Usage: git no-claude-trailer filter [<file>] [--in-place]

With no file, the message is read from stdin and written to stdout. This is
the command the installed commit-msg hook runs.

Options:
  -i, --in-place   Rewrite <file> in place instead of writing to stdout

Examples:
  Clean a message by hand:
    git no-claude-trailer filter .git/COMMIT_EDITMSG --in-place

  See what would change, without changing it:
    git no-claude-trailer filter .git/COMMIT_EDITMSG";

const CHECK_HELP: &str = "\
Report commits that carry AI attribution, and exit 1 if any do.

Usage: git no-claude-trailer check [--range <revision>|--all]

By default this looks only at commits you have not pushed, so it never
complains about history that is already published.

Options:
      --range <revision>   Start after this revision, or pass a full a..b range
      --all                Every commit reachable from HEAD

Examples:
  Gate a CI job on the branch being clean:
    git no-claude-trailer check --range origin/main

  Look at the last three commits:
    git no-claude-trailer check --range HEAD~3";

const CLEAN_HELP: &str = "\
Rewrite commit messages to remove AI attribution.

Usage: git no-claude-trailer clean [--range <revision>|--all] [--dry-run]

Commits are rebuilt with their trees, parents, authors and dates untouched;
only the message changes. Merges keep every parent, so nothing is flattened.
Before the branch moves, the old tip is saved under
refs/no-claude-trailer/backup, and the command prints how to restore it.

By default only unpushed commits are rewritten, so published history is never
rewritten unless you ask with --range or --all.

Options:
      --range <revision>   Start after this revision; the range must end at HEAD
      --all                Every commit reachable from HEAD
  -n, --dry-run            Show what would be removed, change nothing

Examples:
  See what would go, before anything moves:
    git no-claude-trailer clean --dry-run

  Clean everything since main:
    git no-claude-trailer clean --range main";

const INSTALL_HELP: &str = "\
Install the commit-msg hook, so attribution never lands in the first place.

Usage: git no-claude-trailer install [--global] [--force] [--agents=<list>]

The hook is a short shell script that calls this binary by absolute path. It
fails closed: if the binary goes missing your commit is refused, with a message
saying how to fix or remove it.

Options:
      --global          Install for every repository, by setting core.hooksPath
  -f, --force           Replace an existing hook, keeping a backup beside it
      --agents=<list>   Comma-separated agents to strip, recorded in git config

Examples:
  This repository only:
    git no-claude-trailer install

  Everywhere, stripping Claude and Copilot:
    git no-claude-trailer install --global --agents=claude,copilot";

const UNINSTALL_HELP: &str = "\
Remove the commit-msg hook.

Usage: git no-claude-trailer uninstall [--global]

Only a hook installed by no-claude-trailer is ever removed. If installing
moved an existing hook aside, uninstalling puts it back. A machine-wide
uninstall unsets core.hooksPath only when it was us who set it.

Options:
      --global   Undo a machine-wide install

Examples:
    git no-claude-trailer uninstall
    git no-claude-trailer uninstall --global";

const SETUP_HELP: &str = "\
Set up no-claude-trailer by answering a few questions.

Usage: git no-claude-trailer setup

Asks where the hook should run, which agents to strip, whether you want a
`git nct` alias, and whether to clean your unpushed commits now. Every answer
is collected first and shown back as a list of exact paths and config keys;
nothing is written until you confirm. Enter takes the default, and q stops.

This needs a terminal. In a script, use `install` with flags instead:
    git no-claude-trailer install --global --agents=claude,copilot";
