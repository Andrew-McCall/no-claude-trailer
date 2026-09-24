//! Failures, phrased for the person reading them.
//!
//! Every failure carries two things: what happened, and the command that fixes
//! it. Exit codes are stable so scripts can branch on them, but a human never
//! has to look one up — the message says what to do.

use std::fmt;

/// What kind of failure this is, which decides the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Attribution was found by `check`. Exit code 1.
    Found,
    /// The command line was wrong. Exit code 2.
    Usage,
    /// Refused on purpose: dirty tree, existing hook, no TTY. Exit code 3.
    Refused,
    /// git or the filesystem failed. Exit code 4.
    Git,
}

impl Kind {
    /// The process exit code for this kind of failure.
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Found => 1,
            Self::Usage => 2,
            Self::Refused => 3,
            Self::Git => 4,
        }
    }
}

/// Something went wrong, described the way a colleague would describe it.
#[derive(Debug)]
pub struct Failure {
    pub kind: Kind,
    /// What happened, as a sentence.
    pub what: String,
    /// The command or edit that resolves it, when there is one.
    pub fix: Option<String>,
}

impl Failure {
    pub fn new(kind: Kind, what: impl Into<String>) -> Self {
        Self {
            kind,
            what: what.into(),
            fix: None,
        }
    }

    /// Adds the "try this" line shown under the error.
    pub fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    pub fn usage(what: impl Into<String>) -> Self {
        Self::new(Kind::Usage, what)
    }

    pub fn refused(what: impl Into<String>) -> Self {
        Self::new(Kind::Refused, what)
    }

    pub fn git(what: impl Into<String>) -> Self {
        Self::new(Kind::Git, what)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "no-claude-trailer: {}", self.what)?;
        if let Some(fix) = &self.fix {
            write!(formatter, "\n  {fix}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Failure {}
