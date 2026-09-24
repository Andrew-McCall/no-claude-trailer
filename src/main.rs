//! The `git-no-claude-trailer` binary.
//!
//! Named so that git finds it as the subcommand `git no-claude-trailer`.
//! Everything here is dispatch and exit codes; the work lives in the library.

use std::process::ExitCode;

use no_claude_trailer::cli::{self, Command};
use no_claude_trailer::cmd;
use no_claude_trailer::error::{Failure, Kind};
use no_claude_trailer::ui::Style;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            let style = Style::stderr();
            // `check` has already printed its listing, so a finding is a
            // summary rather than an error shouting in red.
            if failure.kind == Kind::Found {
                eprintln!("{failure}");
            } else {
                eprintln!("{}", style.red(&failure.to_string()));
            }
            ExitCode::from(failure.kind.exit_code() as u8)
        }
    }
}

fn run() -> Result<(), Failure> {
    match cli::parse(std::env::args().skip(1))? {
        Command::Check(selection) => cmd::check::run(&selection),
        Command::Clean(arguments) => cmd::clean::run(&arguments),
        Command::Install(arguments) => cmd::install::run(&arguments),
        Command::Status(with_hint) => cmd::status::run(with_hint),
        Command::Setup => cmd::setup::run(),
        Command::Uninstall(arguments) => cmd::uninstall::run(&arguments),
        Command::Filter(arguments) => cmd::filter::run(&arguments),
        Command::Help(topic) => {
            println!("{}", cli::help(topic.as_deref()));
            Ok(())
        }
        Command::Version => {
            println!("no-claude-trailer {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
