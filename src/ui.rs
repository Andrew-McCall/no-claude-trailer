//! Terminal output.
//!
//! Colour is hand-rolled ANSI, and it switches itself off whenever the output
//! is not a terminal, `NO_COLOR` is set, or the terminal says it is dumb — so
//! piping any command into a file gives clean text.

use std::io::IsTerminal;

/// Whether to emit ANSI escapes.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    color: bool,
}

impl Style {
    /// Colour for stdout, following the terminal and `NO_COLOR`.
    pub fn stdout() -> Self {
        Self {
            color: color_allowed() && std::io::stdout().is_terminal(),
        }
    }

    /// Colour for stderr, following the terminal and `NO_COLOR`.
    pub fn stderr() -> Self {
        Self {
            color: color_allowed() && std::io::stderr().is_terminal(),
        }
    }

    /// Plain text, for tests and for piping.
    pub fn plain() -> Self {
        Self { color: false }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    pub fn red(&self, text: &str) -> String {
        self.paint("31", text)
    }

    pub fn green(&self, text: &str) -> String {
        self.paint("32", text)
    }

    pub fn yellow(&self, text: &str) -> String {
        self.paint("33", text)
    }
}

/// Whether the environment permits colour at all.
pub fn color_allowed() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    match std::env::var("TERM") {
        Ok(term) => term != "dumb",
        Err(_) => true,
    }
}
