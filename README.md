# no-claude-trailer

A git plugin that keeps AI attribution out of your commit messages.

_Dependency Free_

## Usage

Set it up by answering a few questions:

```sh
git no-claude-trailer setup
```

Or skip the questions and install the hook directly:

```sh
git no-claude-trailer install
Installed at /home/andrew/work/demo/.git/hooks/commit-msg
  Stripping: claude
```

From then on, attribution never lands. Commit this:

```
Add retry backoff

Retries now back off exponentially.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_011c
```

and this is what the repository keeps:

```
Add retry backoff

Retries now back off exponentially.
```

Commits that slipped through before you installed the hook can be found and
rewritten:

```sh
git no-claude-trailer check
2 of 3 commits carry AI attribution:
  837ad48  Fix pagination off-by-one  Co-Authored-By
  5ad7c5c  Tidy imports               Generated with [Claude Code], Claude session link

git no-claude-trailer clean
Rewrote 2 commits on main
Backup at refs/no-claude-trailer/backup/main-20260917T121744
```

`git no-claude-trailer` on its own shows what is set up and what to do next.

### Commands

| Command | What it does |
| --- | --- |
| `setup` | Asks where the hook should go and which agents to strip, then does it |
| `status` | Shows the repository, the hook, the agents and your unpushed commits |
| `install` / `uninstall` | Writes the commit-msg hook, or removes it and restores what it replaced |
| `check` / `clean` | Reports attribution, or rewrites the commits to remove it |
| `filter` | Strips a message file or stdin; this is what the hook runs |

By default `check` and `clean` only look at what you haven't pushed, so
published history is never rewritten unless you pass `--range` or `--all`.
Stripped agents default to just `claude`; add more with
`--agents=claude,copilot` (`copilot`, `cursor`, `codex`, `gemini`, `devin`,
`aider`, and `bot` for any bot co-author are built in).

Everything the hook needs — which agents, which extra trailers — lives in
git config, so there's nothing else to manage:

```sh
git config --add noclaudetrailer.agent copilot
```

## How It Works

A [git trailer](https://git-scm.com/docs/git-interpret-trailers) is a
`Key: value` line at the end of a commit message, and it's where coding
agents sign their work — `Co-Authored-By: Claude`, `Claude-Session: …`, a
`Generated with …` line and the session URL beneath it. Nothing wrong with
the convention; it just belongs to the tool rather than the history, and once
it's in a commit it stays there.

A [`commit-msg` hook](https://git-scm.com/docs/githooks#_commit_msg) rewrites
the message before each commit lands, so nothing new arrives carrying
attribution. For commits that already have it, `clean` rebuilds them with
`git commit-tree` and moves the branch — trees, authors, dates and merges are
all carried across untouched, and the old tip is saved as a ref first so it's
always one `git reset --hard` away from undone.

Matching is a small table of known agents, not a pattern language — each one
is a set of lowercase needles to look for, and adding one is adding an entry.
No regex engine, no config format of its own. The stripping is careful about
a few things a naive version gets wrong: subject lines are never touched,
nothing past the diff's scissors line is scanned, and a message that would be
left as nothing but attribution is an error rather than an empty commit.

The library works on its own, too:

```rust
use no_claude_trailer::message::{strip, Options};
use no_claude_trailer::profiles::Rules;

fn main() {
    let rules = Rules::from_names(&["claude"]).unwrap();
    let message = "Fix pagination\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n";
    let cleaned = strip(message, &rules, &Options::commit_object()).unwrap();
    println!("{}", cleaned.message); // Fix pagination
}
```

## Installing

```sh
cargo install no-claude-trailer
```

Or build from a local checkout with `cargo install --path .`. Either way the
binary is `git-no-claude-trailer`, which is what lets git find it as
`git no-claude-trailer`.

## License

MIT License

Copyright (c) 2026 Andrew McCall
