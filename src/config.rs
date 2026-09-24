//! Reading the user's preferences out of git config.
//!
//! There is no config file of our own. Everything lives in git config, which
//! the user already knows how to inspect, override per repository and undo:
//!
//! - `noclaudetrailer.agent` — which agents to strip (repeatable, or comma separated)
//! - `noclaudetrailer.needle` — extra literal text that marks a line as attribution
//! - `noclaudetrailer.trailerKey` — extra trailer keys to remove outright

use crate::error::Failure;
use crate::git::Git;
use crate::profiles::{BUILT_IN, DEFAULT_AGENTS, Rules};

/// The config key holding enabled agent names.
pub const AGENT_KEY: &str = "noclaudetrailer.agent";
/// The config key holding extra literal needles.
pub const NEEDLE_KEY: &str = "noclaudetrailer.needle";
/// The config key holding extra trailer keys.
pub const TRAILER_KEY_KEY: &str = "noclaudetrailer.trailerKey";
/// Records that we, not the user, set `core.hooksPath`, so uninstall can undo it.
pub const OWNS_HOOKS_PATH_KEY: &str = "noclaudetrailer.ownsHooksPath";

/// The agent names configured, or the default set when none are.
pub fn agent_names(git: &Git) -> Vec<String> {
    let configured: Vec<String> = git
        .config_all(AGENT_KEY)
        .iter()
        .flat_map(|value| value.split(','))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    if configured.is_empty() {
        return DEFAULT_AGENTS.iter().map(|name| name.to_string()).collect();
    }
    configured
}

/// Builds the matching rules from config, explaining any unknown agent name.
pub fn rules(git: &Git) -> Result<Rules, Failure> {
    let names = agent_names(git);
    let mut rules = Rules::from_names(&names).map_err(|unknown| {
        Failure::usage(format!("unknown agent {:?} in {AGENT_KEY}", unknown.0))
            .fix(format!("Known agents: {}.", known_agents()))
    })?;
    for needle in git.config_all(NEEDLE_KEY) {
        rules.add_needle(&needle);
    }
    for key in git.config_all(TRAILER_KEY_KEY) {
        rules.add_trailer_key(&key);
    }
    Ok(rules)
}

/// Every built-in agent name, comma separated, for help and error messages.
pub fn known_agents() -> String {
    BUILT_IN
        .iter()
        .map(|profile| profile.name)
        .collect::<Vec<_>>()
        .join(", ")
}
