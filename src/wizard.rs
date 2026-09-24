//! The interactive setup flow.
//!
//! The questions are asked against a reader and a writer rather than the
//! terminal directly, so the whole flow is exercised by unit tests instead of
//! only by hand. Two rules hold throughout:
//!
//! - **Nothing is written until the final confirmation.** Every answer is
//!   collected first, then shown back as a list of exact paths and config keys.
//! - **Enter always means the default**, and `q` or end-of-input always means
//!   stop without having changed anything.

use std::io::{BufRead, Write};

use crate::error::Failure;
use crate::hooks;
use crate::profiles::BUILT_IN;
use crate::state::State;

/// Where the user wants the hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// This repository only.
    Repo,
    /// Every repository, through `core.hooksPath`.
    Global,
    /// Record the agents but install no hook.
    ConfigOnly,
}

/// Everything the wizard collected.
#[derive(Debug, PartialEq, Eq)]
pub struct Answers {
    pub place: Place,
    pub agents: Vec<String>,
    /// Add a `git nct` alias.
    pub alias: bool,
    /// Clean unpushed commits straight away.
    pub clean_now: bool,
}

/// Asks the questions. `Ok(None)` means the user stopped, having changed nothing.
pub fn ask<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    state: &State,
) -> Result<Option<Answers>, Failure> {
    let mut session = Session { input, output };
    session.show_state(state)?;

    let Some(place) = session.ask_place()? else {
        return session.aborted();
    };
    let Some(agents) = session.ask_agents(&state.agents)? else {
        return session.aborted();
    };
    let Some(alias) = session.ask("Add a `git nct` alias?", false)? else {
        return session.aborted();
    };

    let dirty = state.unpushed.map(|(_, dirty)| dirty).unwrap_or(0);
    let clean_now = if dirty > 0 && state.repository.is_some() {
        match session.ask(
            &format!(
                "Clean the {} unpushed commits that carry attribution now?",
                dirty
            ),
            true,
        )? {
            Some(answer) => answer,
            None => return session.aborted(),
        }
    } else {
        false
    };

    let answers = Answers {
        place,
        agents,
        alias,
        clean_now,
    };
    session.show_plan(&answers, state, dirty)?;
    match session.ask("Apply?", true)? {
        Some(true) => Ok(Some(answers)),
        _ => session.aborted(),
    }
}

/// One run of the wizard, over a reader and a writer.
struct Session<'a, R, W> {
    input: &'a mut R,
    output: &'a mut W,
}

impl<R: BufRead, W: Write> Session<'_, R, W> {
    fn line(&mut self, text: &str) -> Result<(), Failure> {
        writeln!(self.output, "{text}").map_err(write_failed)
    }

    fn aborted(&mut self) -> Result<Option<Answers>, Failure> {
        self.line("\nStopped. Nothing has been changed.")?;
        Ok(None)
    }

    /// Prints what we found, so the answers are given in context.
    fn show_state(&mut self, state: &State) -> Result<(), Failure> {
        self.line("no-claude-trailer setup\n")?;
        self.line("  Current state")?;
        match &state.repository {
            Some(path) => {
                let branch = state
                    .branch
                    .clone()
                    .unwrap_or_else(|| "detached HEAD".to_string());
                self.line(&format!("    repo      {} on {branch}", path.display()))?;
            }
            None => self.line("    repo      not a git repository")?,
        }
        match &state.hook {
            crate::state::Hook::Missing => self.line("    hook      not installed")?,
            other => self.line(&format!("    hook      {other:?}"))?,
        }
        self.line(&format!("    stripping {}", state.agents.join(", ")))?;
        match state.unpushed {
            Some((total, dirty)) => self.line(&format!(
                "    commits   {total} unpushed, {dirty} carry attribution"
            ))?,
            None => self.line("    commits   no upstream to compare against")?,
        }
        self.line("")
    }

    /// Reads one answer, trimmed. `None` means stop.
    fn read(&mut self, question: &str, default_shown: &str) -> Result<Option<String>, Failure> {
        write!(self.output, "  {question}\n  [{default_shown}] > ").map_err(write_failed)?;
        self.output.flush().map_err(write_failed)?;
        let mut answer = String::new();
        let read = self
            .input
            .read_line(&mut answer)
            .map_err(|error| Failure::git(format!("could not read your answer: {error}")))?;
        if read == 0 {
            return Ok(None); // end of input
        }
        let answer = answer.trim().to_string();
        if answer.eq_ignore_ascii_case("q") || answer.eq_ignore_ascii_case("quit") {
            return Ok(None);
        }
        Ok(Some(answer))
    }

    /// Asks a yes/no question until the answer makes sense.
    fn ask(&mut self, question: &str, default: bool) -> Result<Option<bool>, Failure> {
        let shown = if default { "Y/n" } else { "y/N" };
        loop {
            let Some(answer) = self.read(question, shown)? else {
                return Ok(None);
            };
            if answer.is_empty() {
                return Ok(Some(default));
            }
            match answer.to_ascii_lowercase().as_str() {
                "y" | "yes" => return Ok(Some(true)),
                "n" | "no" => return Ok(Some(false)),
                _ => self.line("  Please answer y or n.")?,
            }
        }
    }

    fn ask_place(&mut self) -> Result<Option<Place>, Failure> {
        self.line("  Where should the hook run?")?;
        self.line("    1  this repository only")?;
        self.line("    2  every repository on this machine  (sets core.hooksPath)")?;
        self.line("    3  record the agents, install no hook")?;
        loop {
            let Some(answer) = self.read("Choose 1, 2 or 3.", "1")? else {
                return Ok(None);
            };
            match answer.as_str() {
                "" | "1" => return Ok(Some(Place::Repo)),
                "2" => return Ok(Some(Place::Global)),
                "3" => return Ok(Some(Place::ConfigOnly)),
                _ => self.line("  Please choose 1, 2 or 3.")?,
            }
        }
    }

    /// Offers the built-in profiles by number, with the current set as default.
    fn ask_agents(&mut self, current: &[String]) -> Result<Option<Vec<String>>, Failure> {
        self.line("  Which agents should be stripped?")?;
        for (index, profile) in BUILT_IN.iter().enumerate() {
            let mark = if current.iter().any(|name| name == profile.name) {
                "*"
            } else {
                " "
            };
            self.line(&format!(
                "    {}{mark} {:<8} {}",
                index + 1,
                profile.name,
                profile.description
            ))?;
        }
        let shown = current.join(",");
        loop {
            let Some(answer) = self.read("Numbers separated by commas, or 'a' for all.", &shown)?
            else {
                return Ok(None);
            };
            if answer.is_empty() {
                return Ok(Some(current.to_vec()));
            }
            if answer.eq_ignore_ascii_case("a") || answer.eq_ignore_ascii_case("all") {
                return Ok(Some(BUILT_IN.iter().map(|p| p.name.to_string()).collect()));
            }
            match select(&answer) {
                Some(agents) => return Ok(Some(agents)),
                None => self.line(&format!(
                    "  Please give numbers from 1 to {}, or 'a'.",
                    BUILT_IN.len()
                ))?,
            }
        }
    }

    /// Shows exactly what will be written, before anything is.
    fn show_plan(&mut self, answers: &Answers, state: &State, dirty: usize) -> Result<(), Failure> {
        self.line("\n  About to write")?;
        match answers.place {
            Place::Repo => {
                let directory = state
                    .repo_hooks_dir
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(".git/hooks"));
                self.line(&format!(
                    "    {}",
                    directory.join(hooks::HOOK_NAME).display()
                ))?;
            }
            Place::Global => {
                let hook = state.global_hooks_dir.join(hooks::HOOK_NAME);
                self.line(&format!("    {}", hook.display()))?;
                self.line(&format!(
                    "    core.hooksPath = {}  (global)",
                    state.global_hooks_dir.display()
                ))?;
            }
            Place::ConfigOnly => self.line("    no hook")?,
        }
        self.line(&format!(
            "    {} = {}",
            crate::config::AGENT_KEY,
            answers.agents.join(", ")
        ))?;
        if answers.alias {
            self.line("    alias.nct  (global)")?;
        }
        if answers.clean_now {
            self.line(&format!("    rewrite {dirty} commits, with a backup ref"))?;
        }
        self.line("")
    }
}

/// Parses `1,3,5` into agent names, rejecting anything out of range.
fn select(answer: &str) -> Option<Vec<String>> {
    let mut agents = Vec::new();
    for part in answer.split(',') {
        let index: usize = part.trim().parse().ok()?;
        let profile = BUILT_IN.get(index.checked_sub(1)?)?;
        let name = profile.name.to_string();
        if !agents.contains(&name) {
            agents.push(name);
        }
    }
    (!agents.is_empty()).then_some(agents)
}

fn write_failed(error: std::io::Error) -> Failure {
    Failure::git(format!("could not write to the terminal: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;

    /// A state with one dirty unpushed commit, in a repository.
    fn state() -> State {
        State {
            repository: Some(PathBuf::from("/tmp/repo")),
            branch: Some("main".to_string()),
            binary: Some(PathBuf::from("/usr/local/bin/git-no-claude-trailer")),
            hook: crate::state::Hook::Missing,
            repo_hooks_dir: Some(PathBuf::from("/tmp/repo/.git/hooks")),
            global_hooks_dir: PathBuf::from("/home/tester/.config/git/hooks"),
            agents: vec!["claude".to_string()],
            unpushed: Some((3, 1)),
        }
    }

    fn run(keystrokes: &str) -> (Option<Answers>, String) {
        let mut input = Cursor::new(keystrokes.as_bytes().to_vec());
        let mut output: Vec<u8> = Vec::new();
        let answers = ask(&mut input, &mut output, &state()).expect("wizard should not fail");
        (answers, String::from_utf8(output).expect("utf-8 output"))
    }

    #[test]
    fn accepting_every_default_installs_into_this_repository() {
        let (answers, _) = run("\n\n\n\n\n");
        assert_eq!(
            answers,
            Some(Answers {
                place: Place::Repo,
                agents: vec!["claude".to_string()],
                alias: false,
                clean_now: true,
            })
        );
    }

    #[test]
    fn choosing_two_installs_machine_wide() {
        let (answers, _) = run("2\n\n\n\n\n");
        assert_eq!(answers.unwrap().place, Place::Global);
    }

    #[test]
    fn choosing_three_installs_no_hook() {
        let (answers, _) = run("3\n\n\n\n\n");
        assert_eq!(answers.unwrap().place, Place::ConfigOnly);
    }

    #[test]
    fn a_selects_every_agent() {
        let (answers, _) = run("1\na\n\n\n\n");
        assert_eq!(answers.unwrap().agents.len(), BUILT_IN.len());
    }

    #[test]
    fn numbers_select_specific_agents() {
        let (answers, _) = run("1\n1,3\n\n\n\n");
        assert_eq!(
            answers.unwrap().agents,
            vec!["claude".to_string(), "cursor".to_string()]
        );
    }

    #[test]
    fn repeated_numbers_are_collapsed() {
        let (answers, _) = run("1\n1,1,1\n\n\n\n");
        assert_eq!(answers.unwrap().agents, vec!["claude".to_string()]);
    }

    #[test]
    fn an_impossible_scope_is_asked_again() {
        let (answers, shown) = run("9\n1\n\n\n\n\n");
        assert_eq!(answers.unwrap().place, Place::Repo);
        assert!(shown.contains("Please choose 1, 2 or 3."), "{shown}");
    }

    #[test]
    fn an_out_of_range_agent_is_asked_again() {
        let (answers, shown) = run("1\n99\n1\n\n\n\n");
        assert_eq!(answers.unwrap().agents, vec!["claude".to_string()]);
        assert!(shown.contains("Please give numbers"), "{shown}");
    }

    #[test]
    fn nonsense_for_a_yes_no_question_is_asked_again() {
        let (answers, shown) = run("1\n\nmaybe\ny\n\n\n");
        assert!(answers.unwrap().alias);
        assert!(shown.contains("Please answer y or n."), "{shown}");
    }

    #[test]
    fn q_stops_without_answers() {
        // Input continues after the `q`, so passing this requires `q` itself to
        // stop the wizard rather than the reader simply running dry.
        let (answers, shown) = run("q\n1\n\n\n\n\n");
        assert_eq!(answers, None);
        assert!(shown.contains("Nothing has been changed."), "{shown}");
        assert!(
            !shown.contains("Which agents"),
            "it should stop before the next question: {shown}"
        );
    }

    #[test]
    fn end_of_input_stops_without_answers() {
        let (answers, _) = run("");
        assert_eq!(answers, None);
    }

    #[test]
    fn declining_the_final_confirmation_stops() {
        let (answers, shown) = run("1\n\n\n\nn\n");
        assert_eq!(answers, None);
        assert!(shown.contains("Nothing has been changed."), "{shown}");
    }

    #[test]
    fn the_state_is_shown_before_any_question() {
        let (_, shown) = run("q\n");
        assert!(shown.contains("Current state"), "{shown}");
        assert!(shown.contains("/tmp/repo"), "{shown}");
        assert!(shown.contains("not installed"), "{shown}");
        assert!(shown.contains("3 unpushed, 1 carry attribution"), "{shown}");
    }

    #[test]
    fn the_exact_changes_are_shown_before_confirming() {
        let (_, shown) = run("2\n1,2\ny\ny\nn\n");
        assert!(shown.contains("About to write"), "{shown}");
        assert!(
            shown.contains("/home/tester/.config/git/hooks/commit-msg"),
            "{shown}"
        );
        assert!(shown.contains("core.hooksPath"), "{shown}");
        assert!(
            shown.contains("noclaudetrailer.agent = claude, copilot"),
            "{shown}"
        );
        assert!(shown.contains("alias.nct"), "{shown}");
        assert!(shown.contains("rewrite 1 commits"), "{shown}");
    }

    #[test]
    fn a_clean_repository_is_not_offered_a_rewrite() {
        let mut clean = state();
        clean.unpushed = Some((3, 0));
        let mut input = Cursor::new(b"\n\n\n\n".to_vec());
        let mut output: Vec<u8> = Vec::new();
        let answers = ask(&mut input, &mut output, &clean).unwrap().unwrap();
        assert!(!answers.clean_now);
        let shown = String::from_utf8(output).unwrap();
        assert!(!shown.contains("Clean the"), "{shown}");
    }

    #[test]
    fn every_agent_is_listed_with_its_description() {
        let (_, shown) = run("1\nq\n");
        for profile in BUILT_IN {
            assert!(
                shown.contains(profile.name),
                "{} missing from {shown}",
                profile.name
            );
        }
    }
}
