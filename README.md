# no-claude-trailer

A dependency-free Git plugin that keeps AI attribution out of your commit messages.

## Usage

First install it:

```sh
cargo install no-claude-trailer
```

Then set it up interactively:

```sh
git no-claude-trailer setup
```

Or skip the questions and install the hook directly:

```sh
git no-claude-trailer install
Installed at /home/andrew/work/demo/.git/hooks/commit-msg
  Stripping: claude
```

From then on, AI attribution is removed before a commit is created.

## Existing commits

Commits created before installing the hook can be inspected and cleaned:

```sh
git no-claude-trailer check
2 of 3 commits carry AI attribution:
  837ad48  Fix pagination off-by-one  Co-Authored-By
  5ad7c5c  Tidy imports               Generated with [Claude Code], Claude session link
```

Then rewrite them with:

```sh
git no-claude-trailer clean
Rewrote 2 commits on main
Backup at refs/no-claude-trailer/backup/main-20260917T121744
```

The original branch tip is saved before rewriting, so the operation can be undone.

Running the command without arguments shows the current configuration and what to do next:

```sh
git no-claude-trailer
```

## Commands

| Command     | Description                                                         |
| ----------- | ------------------------------------------------------------------- |
| `setup`     | Interactively configures and installs the hook                      |
| `status`    | Shows the repository, hook, configured agents, and unpushed commits |
| `install`   | Installs the `commit-msg` hook                                      |
| `uninstall` | Removes the hook and restores anything it replaced                  |
| `check`     | Reports commits containing AI attribution                           |
| `clean`     | Rewrites commits to remove AI attribution                           |
| `filter`    | Filters a commit-message file or stdin                              |

By default, `check` and `clean` only inspect commits that have not been pushed. Published history is never rewritten unless you explicitly provide `--range` or `--all`.

## Agents

Claude is enabled by default.

Additional agents can be selected with:

```sh
git no-claude-trailer install --agents=claude,copilot
```

Built-in profiles include:

- `claude`
- `copilot`
- `cursor`
- `codex`
- `gemini`
- `devin`
- `aider`
- `bot` — generic bot co-authors

Agents can also be added through Git configuration:

```sh
git config --add noclaudetrailer.agent copilot
```

Everything the hook needs is stored in Git configuration. There is no additional configuration file to maintain.

## How It Works

Coding agents commonly add attribution to commit messages using Git trailers and related metadata, such as:

```text
Co-Authored-By: Claude <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_011c
```

Some agents also add generated-by text and session URLs.

A [`commit-msg` hook](https://git-scm.com/docs/githooks#_commit_msg) runs before Git creates the commit and filters the message. This prevents new commits from acquiring unwanted attribution.

For existing commits, `clean` rebuilds the affected history using `git commit-tree` and moves the branch to the rewritten commits. Trees, authors, dates, parents, and merges are preserved. The original tip is saved under a backup ref before anything is rewritten.

Matching is deliberately conservative. Each built-in agent profile contains a small set of lowercase strings used to identify its attribution. There is no regex engine, pattern language, or separate configuration format.

The filtering also handles a few important edge cases:

- Subject lines are never modified.
- Content after Git's diff-scissors marker is ignored.
- Only attribution matching a configured agent is removed.
- A message that would contain nothing except attribution is rejected rather than turned into an empty commit.

## Design

The project is intentionally small:

- Dependency-free at runtime
- Native Rust implementation
- No external configuration files
- Git configuration for agent selection
- Safe history rewriting with automatic backup refs
- Conservative commit-message filtering

## Feedback

Issues, suggestions, and feedback are very welcome. If you find something that does not work as expected, please open an issue and let me know.

## License

MIT License

Copyright (c) 2026 Andrew McCall
