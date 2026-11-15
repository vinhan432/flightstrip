# Flightstrip

A flight recorder for AI coding agents.

An agent tells you it fixed the failing test. Flightstrip shows you that it
deleted the assertion. It records what an agent session actually did — every file
change, every command, every test run — and turns the recording into a paper
strip chart you can replay, plus a set of deterministic findings you can put in a
pull request.

No backend. No database. No API keys. No model in the loop. One binary that
writes plain files, and a static page that reads one of them.

```
flightstrip watch                      # record in the repo you're working in
flightstrip mark "add retry to upload" # what you asked the agent for
flightstrip exec -- cargo test         # record the command and its result
flightstrip export --out session.json  # one portable artifact
flightstrip report                     # markdown summary for the PR
```

From `flightstrip report` on the bundled `weakened-tests` fixture:

```
| Step | Kind          | Severity | Evidence                                  |
|------|---------------|----------|-------------------------------------------|
| 12   | test_weakened | critical | expect(retries).toBe(2)                   |
| 19   | test_regression | critical | npm test · 41 passed / 1 failed · touched: src/upload.ts |
| 26   | test_weakened | critical | test.skip('drains the queue', async () => { |
```

## Why

Two things are hard to see in an agent session after the fact:

1. **What changed between the tests being red and green.** A green suite is an
   outcome, not evidence. Removing an assertion and fixing the code look
   identical in a final diff.
2. **What the agent ran.** `git push --force`, `rm -rf` against the wrong path, a
   secret pasted into a config file — none of it appears in a commit diff.

Flightstrip records the session as it happens, so the sequence is preserved.
Detectors are small, deterministic rules over that sequence. They never claim the
change is correct; they point at the moments worth reading.

## Install

```sh
cargo build --release
# target/release/flightstrip  (single binary, no runtime dependencies)
```

Windows is the first target. The code is std-only and the only platform-specific
piece is the Ctrl+C handler, which has a Unix branch — but the macOS and Linux
builds were not exercised while developing this, so treat them as untested.
Rust stable, no other toolchain, no C compiler, no `pkg-config`.

> On this machine the workspace is configured for the gnullvm/gnu toolchain in
> `.cargo/config.toml`; a stock `cargo build` elsewhere does not need it.

## Use

### Record

```sh
cd your-repo
flightstrip watch --agent "claude code" --title "checkout refactor"
```

`watch` polls the working tree (500 ms by default; `--interval` changes it),
hashes what changed, and stores snapshots in `.flightstrip/`. Ctrl+C closes the
session and prints a summary.

Before you send the agent a prompt:

```sh
flightstrip mark "add a retry to the uploader"
```

And run its commands through the recorder:

```sh
flightstrip exec -- cargo test
```

`exec` runs the command exactly as a shell would, echoes its output, records the
exit code and duration, and parses the test summary. Its own exit code is the
child's, so it can sit in a script.

Add `.flightstrip/` to `.gitignore`.

### Export

```sh
flightstrip export --out session.json           # latest session
flightstrip export 20261114T133000Z-c3 --redact # scrub secret-looking values
flightstrip export --fixture                   # mark it synthetic
```

`session.json` is the only interface between the recorder and anything else.
Everything below is derived from it.

### Report

```sh
flightstrip report --out report.md
```

A markdown summary: verdict counts, an incident table, test status, commands run,
dependencies added, and which files belong to which prompt. It renders on GitHub
and in a terminal.

### Replay

```sh
node web/serve.js    # http://localhost:4173/
```

The replay page takes a `session.json` — one of the bundled fixtures, or your
own via **Open session.json…** — and draws it as a paper strip chart. Press play
and the tape advances: prompt bands, file changes, commands, test runs and
incident spikes appear in the order they happened. Click any step to read the
diff hunk that caused it.

The page has no build step and no npm dependencies: `index.html`, `style.css` and
`app.js`. Next to them are two small Node scripts — `serve.js`, a static server
that exists only because `fetch` does not work from `file://`, and `smoke.mjs`,
which runs the real client against a stubbed DOM as a startup check.

## What gets recorded

| Event | Fields |
|---|---|
| `session_start` | title, agent, os, repo name |
| `prompt` | id, text, source (`mark`/`pty`/`import`) |
| `file_change` | path, op, before/after hash, lines added/removed, prompt |
| `command` | cmd, exit code, duration, source, prompt |
| `test_run` | cmd, passed/failed/skipped/total, ok, prompt |
| `note` | free text |
| `session_end` | — |

Paths are repo-relative with forward slashes on every platform. Content is
LF-normalised before hashing, so a CRLF checkout or an editor's line-ending
setting cannot manufacture a change.

## Detectors

Six deterministic rules. Each answers "what would a careful reviewer want to look
at next", never "is this correct".

| Detector | Fires when |
|---|---|
| `test_regression` | the suite was green, then went red, with the changes in between listed |
| `test_weakened` | an assertion was removed, a test was skipped/todo'd, a test file was deleted, or a run reports fewer tests than the previous one |
| `thrashing` | one file was rewritten five or more times inside a single prompt |
| `dangerous_command` | recursive delete of a filesystem or drive, force-push to the default branch, piping a download into a shell, `drop table`, `chmod 777 /`, … |
| `dependency_added` | a manifest gained packages (npm, cargo, pip, python, go, bundler, maven) |
| `secret_touched` | a secret-bearing path was touched, or a credential-shaped value was added to a diff |

Incidents are ordered by step and carry the prompt they belong to, so the tape can
credit them to what was asked.

## Honest limitations

- **Attribution is a heuristic.** Work is credited to the most recent prompt
  unless it arrives more than five minutes later with no command in between. Two
  agents in one repository will be conflated.
- **`watch` polls.** 500 ms of latency by default, and one directory walk per
  interval. Ignored directories are pruned before they are read, but a very large
  tree is still walked repeatedly. That is the trade for not depending on
  platform notification APIs.
- **`.gitignore` support is a subset.** The root `.gitignore` only: comments,
  `!` negation, trailing `/`, leading `/`, `*`, `**`, `?`. Nested `.gitignore`
  files and `[a-z]` classes are ignored. Built-in exclusions (`node_modules`,
  `target`, `.git`, `.venv`, …) cover the common cases anyway.
- **Diffs are capped.** Three hunks, 20 lines each, 200 characters per line, two
  lines of context. A change needing more than 512 inserted+deleted lines to
  describe is reported as a whole-file replacement rather than spending seconds
  searching for a minimal script.
- **Test parsing is guesswork** over human-facing output (cargo, pytest, jest,
  vitest, mocha, go, rspec, node's own test runner in both its TAP and default
  reporter). Unknown output falls back to the exit code and says
  so by reporting one anonymous test.
- **Binary and oversized (>1 MiB) files** are recorded as opaque changes: hash,
  operation and line counts, no diff.
- **Redaction is best-effort.** `--redact` catches known token prefixes,
  `key = value` pairs and high-entropy strings. It is not a security boundary; do
  not export a session you would not paste in a PR.
- **The store grows.** Snapshots are content-addressed and deduplicated, but
  nothing is pruned automatically. `.flightstrip/` is disposable:
  `rm -rf .flightstrip` loses history, not code.
- **A deleted file is only recorded as a deletion** if the recorder knew the file.
  Editor scratch files that appear and vanish are ignored on purpose.

## Repository layout

```
crates/flightstrip-core/    recording, storage, diffing, detectors, export
crates/flightstrip-cli/     argument parsing, terminal, process spawning
fixtures/                   three generated sessions (clean / weakened / risky)
web/                        static replay page, no dependencies
```

## Development

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -p flightstrip-core --example fixtures   # regenerate fixtures/
node web/smoke.mjs                                # boot the replay page headlessly
```

The core has no third-party dependencies and the web page has no npm
dependencies. Why, and what that costs, is in [NOTES.md](NOTES.md).
