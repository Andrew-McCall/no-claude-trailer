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

Commits that slipped through before you installed the hook can be found:

```sh
git no-claude-trailer check
2 of 3 commits carry AI attribution:
  837ad48  Fix pagination off-by-one  Co-Authored-By
  5ad7c5c  Tidy imports               Generated with [Claude Code], Claude session link
```

and rewritten:

```sh
git no-claude-trailer clean
Rewrote 2 commits on main
  837ad48 → be3c827  Fix pagination off-by-one  Co-Authored-By
  5ad7c5c → 37ea9fd  Tidy imports               Generated with [Claude Code], Claude session link
Backup at refs/no-claude-trailer/backup/main-20260917T121744
  Undo with `git reset --hard refs/no-claude-trailer/backup/main-20260917T121744`
```

`git no-claude-trailer` on its own shows what is set up and what to do next.

### Commands

| Command | What it does |
| --- | --- |
| `setup` | Asks where the hook should go and which agents to strip, then does it |
| `status` | Shows the repository, the hook, the agents and your unpushed commits |
| `install` | Writes the commit-msg hook |
| `uninstall` | Removes it, and restores any hook it replaced |
| `check` | Reports attribution and exits 1 if it finds any |
| `clean` | Rewrites commit messages to remove attribution |
| `filter` | Strips a message file or stdin; this is what the hook runs |

Exit codes: `0` fine · `1` attribution found · `2` bad usage · `3` refused on
purpose · `4` git or the filesystem failed.

### Options

`--agents` chooses which agents to strip, and records the choice in git config:

```sh
git no-claude-trailer install --agents=claude,copilot
```

Built in: `claude`, `copilot`, `cursor`, `codex`, `gemini`, `devin`, `aider`,
and `bot` for any bot co-author. Only `claude` is on unless you say otherwise.

`--global` installs for every repository, by pointing `core.hooksPath` at a
hook directory in your config. It refuses if `core.hooksPath` is already
somebody else's arrangement, unless you add `--force`:

```sh
git no-claude-trailer install --global --agents=claude,copilot
```

`--dry-run` shows what `clean` would remove, and removes nothing:

```sh
git no-claude-trailer clean --dry-run
Would rewrite 2 commits on main
  837ad48  Fix pagination off-by-one
    - Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
  5ad7c5c  Tidy imports
    - 🤖 Generated with [Claude Code](https://claude.com/claude-code)
    - https://claude.ai/code/session_011cHRTmeNC5X7
Nothing has been changed.
```

`--range` and `--all` choose which commits `check` and `clean` look at. The
default is only what you have not pushed, so published history is never
rewritten unless you ask:

```sh
git no-claude-trailer clean --range main
git no-claude-trailer check --all
```

`--force` lets `install` replace an existing hook, keeping it as
`commit-msg.no-claude-trailer.bak`. Uninstalling puts it back.

### Configuration

Everything lives in git config, so there is nothing new to learn and nothing
to delete when you are done:

```sh
git config --add noclaudetrailer.agent copilot        # strip another agent
git config --add noclaudetrailer.needle my-agent      # extra text to look for
git config --add noclaudetrailer.trailerKey X-Agent   # extra trailer to remove
```

## How It Works

A [git trailer](https://git-scm.com/docs/git-interpret-trailers) is a
`Key: value` line at the end of a commit message, and it is where coding
agents sign their work — `Co-Authored-By: Claude`, `Claude-Session: …`, a
`Generated with …` line and the session URL beneath it. There is nothing wrong
with the convention; it just belongs to the tool rather than the history, and
once it is in a commit it stays there.

Two things remove it. A [`commit-msg`
hook](https://git-scm.com/docs/githooks#_commit_msg) runs before every commit
lands and rewrites the message file in place, so nothing new arrives carrying
attribution. For what arrived already, `clean` walks the commits you name,
rebuilds each one with `git commit-tree`, and moves the branch afterwards.
Trees, parents, authors, committers and dates are all carried across
untouched, merges keep every parent, and the old tip is saved as a ref before
anything moves. Signed commits are re-signed with your configured key.

Matching is a table, not a pattern language. Each agent is a [`Profile`] of
lowercase needles — trailer keys it owns outright, text to look for in the
value of a co-author trailer, and text to look for anywhere in a line — and
adding an agent means adding one entry. There is no regex engine: two
hand-written functions do a case-insensitive substring scan and split a
trailer into its key and value, which is all the matching in the crate.

Four rules keep the stripping honest, and each exists because the obvious
implementation gets it wrong:

- **The subject line is never removed**, so a commit titled
  `Add Co-Authored-By parsing` survives.
- **Nothing at or after the scissors line is touched.** With `commit.verbose`
  the message file contains the whole diff, and a diff can legitimately
  contain the text of a trailer.
- **Blank lines left behind are collapsed**, but blank runs the author wrote
  are left exactly as they are.
- **A message that is nothing but attribution is an error**, never an empty
  commit.

The library is usable on its own:

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

`Options::message_file("#")` reads the input as a hook's message file, with
comments and the scissors line respected. `Rules` also takes extra needles and
trailer keys at runtime, which is how the config keys above extend the table
without a rebuild.

## Installing

```sh
cargo install --path .
```

The binary is called `git-no-claude-trailer`, which is what lets git find it
as `git no-claude-trailer`. The hook records its absolute path, so a changed
`PATH` cannot quietly stop it working — and if the binary does go missing the
hook fails closed, refusing the commit with a message saying how to reinstall
it or remove the hook.

## License

MIT License

Copyright (c) 2026 Andrew McCall
