//! The table of AI agents and the needles that identify their attribution.
//!
//! Adding support for another agent means adding one [`Profile`] entry to
//! [`BUILT_IN`] and nothing else — no logic in this crate branches on an agent
//! name. Users extend the same table at runtime through git config, so a new
//! agent never requires a new release.

use crate::scan::{contains_ignore_case, split_trailer};

/// Trailer keys whose *value* is checked against [`Profile::coauthor_needles`].
///
/// `Signed-off-by` is deliberately absent: it carries a legal meaning under the
/// Developer Certificate of Origin and is never ours to remove.
const COAUTHOR_KEYS: &[&str] = &["co-authored-by", "coauthored-by", "assisted-by"];

/// One agent's attribution fingerprint. All needles must be lowercase.
#[derive(Debug)]
pub struct Profile {
    /// The name used in config and on the command line, e.g. `claude`.
    pub name: &'static str,
    /// Shown by `status` and the setup wizard.
    pub description: &'static str,
    /// Trailer keys removed outright, whatever their value.
    pub trailer_keys: &'static [&'static str],
    /// Needles matched against the value of a co-author style trailer.
    pub coauthor_needles: &'static [&'static str],
    /// Needles matched anywhere in a line, for prose attribution, each paired
    /// with the label `check` prints when it matches.
    pub line_needles: &'static [(&'static str, &'static str)],
}

/// Every agent this crate knows about. `claude` is the only default.
pub const BUILT_IN: &[Profile] = &[
    Profile {
        name: "claude",
        description: "Claude Code (Anthropic)",
        trailer_keys: &["claude-session"],
        coauthor_needles: &["claude", "anthropic"],
        line_needles: &[
            (
                "generated with [claude code]",
                "Generated with [Claude Code]",
            ),
            ("claude.com/claude-code", "Claude Code link"),
            ("claude.ai/code/session", "Claude session link"),
        ],
    },
    Profile {
        name: "copilot",
        description: "GitHub Copilot",
        trailer_keys: &[],
        coauthor_needles: &["copilot"],
        line_needles: &[("github copilot", "GitHub Copilot mention")],
    },
    Profile {
        name: "cursor",
        description: "Cursor",
        trailer_keys: &[],
        coauthor_needles: &["cursor"],
        line_needles: &[
            ("generated with cursor", "Generated with Cursor"),
            ("cursor.com", "Cursor link"),
            ("cursor.sh", "Cursor link"),
        ],
    },
    Profile {
        name: "codex",
        description: "OpenAI Codex / ChatGPT",
        trailer_keys: &[],
        coauthor_needles: &["codex", "openai", "chatgpt"],
        line_needles: &[
            ("generated with codex", "Generated with Codex"),
            ("generated with chatgpt", "Generated with ChatGPT"),
        ],
    },
    Profile {
        name: "gemini",
        description: "Gemini / Jules (Google)",
        trailer_keys: &[],
        coauthor_needles: &["gemini", "google-labs-jules"],
        line_needles: &[
            ("generated with gemini", "Generated with Gemini"),
            ("gemini-cli", "Gemini CLI mention"),
        ],
    },
    Profile {
        name: "devin",
        description: "Devin (Cognition)",
        trailer_keys: &[],
        coauthor_needles: &["devin", "cognition"],
        line_needles: &[("devin.ai", "Devin link")],
    },
    Profile {
        name: "aider",
        description: "Aider",
        trailer_keys: &[],
        coauthor_needles: &["aider"],
        line_needles: &[
            ("generated with aider", "Generated with Aider"),
            ("aider.chat", "Aider link"),
        ],
    },
    Profile {
        name: "bot",
        description: "Any bot co-author (catch-all)",
        trailer_keys: &[],
        // Narrow on purpose: a bare "bot" needle would also strip a human
        // called Talbot.
        coauthor_needles: &["[bot]", "bot@users.noreply.github.com"],
        line_needles: &[],
    },
];

/// The agents enabled when the user has expressed no preference.
pub const DEFAULT_AGENTS: &[&str] = &["claude"];

/// Why a line was considered attribution, for reporting to a human.
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    /// The profile that matched, or `config` for a user-supplied rule.
    pub agent: String,
    /// A short label naming what matched, e.g. `Co-Authored-By`.
    pub label: String,
    /// The line itself, so a dry run can show exactly what would be removed.
    pub line: String,
}

/// A resolved set of matching rules: chosen profiles plus user additions.
#[derive(Debug)]
pub struct Rules {
    agents: Vec<&'static Profile>,
    extra_needles: Vec<String>,
    extra_keys: Vec<String>,
}

/// An agent name that is not in [`BUILT_IN`].
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownAgent(pub String);

impl Rules {
    /// Resolves agent names against [`BUILT_IN`], preserving the caller's order.
    pub fn from_names<S: AsRef<str>>(names: &[S]) -> Result<Self, UnknownAgent> {
        let mut agents = Vec::with_capacity(names.len());
        for name in names {
            let name = name.as_ref().trim();
            let profile = BUILT_IN
                .iter()
                .find(|profile| profile.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| UnknownAgent(name.to_string()))?;
            agents.push(profile);
        }
        Ok(Self {
            agents,
            extra_needles: Vec::new(),
            extra_keys: Vec::new(),
        })
    }

    /// Adds a literal needle matched anywhere in a line, from user config.
    ///
    /// Blank needles are dropped rather than matching every line.
    pub fn add_needle(&mut self, needle: &str) {
        let needle = needle.trim();
        if !needle.is_empty() {
            self.extra_needles.push(needle.to_lowercase());
        }
    }

    /// Adds a trailer key removed outright, from user config.
    pub fn add_trailer_key(&mut self, key: &str) {
        let key = key.trim();
        if !key.is_empty() {
            self.extra_keys.push(key.to_string());
        }
    }

    /// The enabled agent names, in resolution order.
    pub fn agent_names(&self) -> Vec<&'static str> {
        self.agents.iter().map(|profile| profile.name).collect()
    }

    /// Tests one line, returning why it matched.
    ///
    /// Rules are tried in order of confidence: a trailer key we own outright,
    /// then a co-author trailer naming an agent, then prose attribution.
    pub fn match_line(&self, line: &str) -> Option<Hit> {
        let mut hit = if let Some((key, value)) = split_trailer(line) {
            self.match_trailer(key, value)
                .or_else(|| self.match_prose(line))
        } else {
            self.match_prose(line)
        }?;
        hit.line = line.to_string();
        Some(hit)
    }

    /// Rules that require the line to be a `Key: value` trailer.
    fn match_trailer(&self, key: &str, value: &str) -> Option<Hit> {
        for profile in &self.agents {
            if profile
                .trailer_keys
                .iter()
                .any(|owned| key.eq_ignore_ascii_case(owned))
            {
                return Some(Hit::new(profile.name, key));
            }
        }
        if self
            .extra_keys
            .iter()
            .any(|owned| key.eq_ignore_ascii_case(owned))
        {
            return Some(Hit::new("config", key));
        }
        if !COAUTHOR_KEYS
            .iter()
            .any(|coauthor| key.eq_ignore_ascii_case(coauthor))
        {
            return None;
        }
        for profile in &self.agents {
            if profile
                .coauthor_needles
                .iter()
                .any(|needle| contains_ignore_case(value, needle))
            {
                return Some(Hit::new(profile.name, key));
            }
        }
        None
    }

    /// Rules that match anywhere in the line, trailer or not.
    fn match_prose(&self, line: &str) -> Option<Hit> {
        for profile in &self.agents {
            for (needle, label) in profile.line_needles {
                if contains_ignore_case(line, needle) {
                    return Some(Hit::new(profile.name, label));
                }
            }
        }
        for needle in &self.extra_needles {
            if contains_ignore_case(line, needle) {
                return Some(Hit::new("config", needle));
            }
        }
        None
    }
}

impl Hit {
    /// The line is filled in by [`Rules::match_line`], which alone knows it.
    fn new(agent: &str, label: &str) -> Self {
        Self {
            agent: agent.to_string(),
            label: label.to_string(),
            line: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude() -> Rules {
        Rules::from_names(&["claude"]).unwrap()
    }

    #[test]
    fn default_agents_are_claude_only() {
        assert_eq!(DEFAULT_AGENTS, &["claude"]);
    }

    #[test]
    fn built_in_names_are_unique() {
        let mut names: Vec<&str> = BUILT_IN.iter().map(|p| p.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate profile name in BUILT_IN");
    }

    #[test]
    fn built_in_needles_are_lowercase() {
        for profile in BUILT_IN {
            let pairs = profile.line_needles.iter().map(|(needle, _)| needle);
            for needle in profile
                .coauthor_needles
                .iter()
                .chain(profile.trailer_keys)
                .chain(pairs)
            {
                assert_eq!(
                    *needle,
                    needle.to_lowercase(),
                    "needle {needle:?} in profile {:?} must be lowercase",
                    profile.name
                );
            }
        }
    }

    #[test]
    fn a_hit_carries_the_line_it_matched() {
        let line = "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>";
        assert_eq!(claude().match_line(line).unwrap().line, line);
    }

    #[test]
    fn matches_a_claude_coauthor_trailer() {
        let hit = claude()
            .match_line("Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>")
            .expect("should match");
        assert_eq!(hit.agent, "claude");
        assert_eq!(hit.label, "Co-Authored-By");
    }

    #[test]
    fn matches_a_claude_session_trailer_by_key() {
        let hit = claude()
            .match_line("Claude-Session: https://claude.ai/code/session_011c")
            .expect("should match");
        assert_eq!(hit.label, "Claude-Session");
    }

    #[test]
    fn matches_the_generated_with_line() {
        let hit = claude()
            .match_line("🤖 Generated with [Claude Code](https://claude.com/claude-code)")
            .expect("should match");
        assert_eq!(hit.agent, "claude");
    }

    #[test]
    fn matches_a_bare_session_url_line() {
        assert!(
            claude()
                .match_line("https://claude.ai/code/session_011cHRTmeNC5X7")
                .is_some()
        );
    }

    #[test]
    fn leaves_a_human_coauthor_alone() {
        assert_eq!(
            claude().match_line("Co-Authored-By: Ada Lovelace <ada@example.com>"),
            None
        );
    }

    #[test]
    fn leaves_ordinary_prose_alone() {
        assert_eq!(
            claude().match_line("Fix the off-by-one in pagination"),
            None
        );
        assert_eq!(claude().match_line(""), None);
    }

    #[test]
    fn leaves_signed_off_by_alone() {
        // Even when it names an agent: the DCO sign-off is not ours to remove.
        assert_eq!(
            claude().match_line("Signed-off-by: Claude <noreply@anthropic.com>"),
            None
        );
    }

    #[test]
    fn ignores_agents_that_are_not_enabled() {
        assert_eq!(
            claude().match_line("Co-Authored-By: Copilot <copilot@github.com>"),
            None
        );
    }

    #[test]
    fn matches_an_agent_once_enabled() {
        let rules = Rules::from_names(&["claude", "copilot"]).unwrap();
        let hit = rules
            .match_line("Co-Authored-By: Copilot <copilot@github.com>")
            .expect("should match");
        assert_eq!(hit.agent, "copilot");
    }

    #[test]
    fn bot_catch_all_spares_humans_with_bot_in_their_name() {
        let rules = Rules::from_names(&["bot"]).unwrap();
        assert!(
            rules
                .match_line("Co-Authored-By: renovate[bot] <bot@users.noreply.github.com>")
                .is_some()
        );
        assert_eq!(
            rules.match_line("Co-Authored-By: Talbot Reginald <tal@example.com>"),
            None
        );
    }

    #[test]
    fn rejects_an_unknown_agent_name() {
        assert_eq!(
            Rules::from_names(&["skynet"]).unwrap_err(),
            UnknownAgent("skynet".to_string())
        );
    }

    #[test]
    fn resolves_agent_names_case_insensitively() {
        assert_eq!(
            Rules::from_names(&["CLAUDE"]).unwrap().agent_names(),
            vec!["claude"]
        );
    }

    #[test]
    fn an_empty_agent_list_matches_nothing() {
        let rules = Rules::from_names::<&str>(&[]).unwrap();
        assert_eq!(
            rules.match_line("Co-Authored-By: Claude <noreply@anthropic.com>"),
            None
        );
    }

    #[test]
    fn user_needles_extend_the_table() {
        let mut rules = Rules::from_names::<&str>(&[]).unwrap();
        rules.add_needle("my-internal-agent");
        let hit = rules
            .match_line("Co-Authored-By: My-Internal-Agent <x@y>")
            .expect("should match");
        assert_eq!(hit.agent, "config");
    }

    #[test]
    fn user_trailer_keys_extend_the_table() {
        let mut rules = Rules::from_names::<&str>(&[]).unwrap();
        rules.add_trailer_key("X-Agent-Run");
        let hit = rules.match_line("X-Agent-Run: 42").expect("should match");
        assert_eq!(hit.agent, "config");
        assert_eq!(hit.label, "X-Agent-Run");
    }

    #[test]
    fn user_needles_are_ignored_when_blank() {
        let mut rules = Rules::from_names::<&str>(&[]).unwrap();
        rules.add_needle("   ");
        rules.add_trailer_key("");
        assert_eq!(rules.match_line("Co-Authored-By: Claude <x@y>"), None);
        assert_eq!(rules.match_line("anything at all"), None);
    }

    #[test]
    fn coauthor_keys_do_not_match_prose_needles() {
        // "cursor" as a co-author needle must not strip a line that merely
        // mentions the word in a sentence.
        let rules = Rules::from_names(&["cursor"]).unwrap();
        assert_eq!(rules.match_line("Fix the cursor position on resize"), None);
    }
}
