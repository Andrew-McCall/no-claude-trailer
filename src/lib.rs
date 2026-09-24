//! Strip AI attribution trailers from git commit messages.
//!
//! ```
//! use no_claude_trailer::message::{strip, Options};
//! use no_claude_trailer::profiles::Rules;
//!
//! let rules = Rules::from_names(&["claude"]).unwrap();
//! let message = "Fix pagination\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n";
//! let cleaned = strip(message, &rules, &Options::commit_object()).unwrap();
//! assert_eq!(cleaned.message, "Fix pagination\n");
//! ```
//!
//! The crate is split so that each module has exactly one job:
//!
//! - [`scan`] — the only code in the crate that compares strings.
//! - [`profiles`] — the static table of agents and the needles that identify them.
//! - [`message`] — the rules for removing attribution from a message.
//! - [`commit`] — parsing raw commit objects so they can be rebuilt faithfully.
//! - [`git`] — every conversation with the `git` binary.
//! - [`config`] — reading preferences out of git config.
//! - [`cli`] — argument parsing and the help text of record.
//! - [`range`] — choosing which commits a command acts on.
//! - [`report`] — describing commits to a person.
//! - [`rewrite`] — planning and applying a history rewrite.
//! - [`hooks`] — the commit-msg hook: where it goes and what it says.
//! - [`state`] — what is set up here, shared by status and setup.
//! - [`wizard`] — the interactive setup flow.
//! - [`cmd`] — one module per command.
//! - [`ui`] — terminal colour, switched off when it is not wanted.
//! - [`error`] — failures phrased for the person reading them.
//! - [`clock`] — the calendar arithmetic a dependency would otherwise provide.

pub mod cli;
pub mod clock;
pub mod cmd;
pub mod commit;
pub mod config;
pub mod error;
pub mod git;
pub mod hooks;
pub mod message;
pub mod profiles;
pub mod range;
pub mod report;
pub mod rewrite;
pub mod scan;
pub mod state;
pub mod ui;
pub mod wizard;
