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
| B-002 | P1 | docs | `NOTES.md` says both crates have an empty `[dependencies]`, that `Cargo.lock` has "exactly two packages", that `clap` was replaced by "a loop over `Vec<String>`", and that a cold release build is 29.1 s. Actual: `Cargo.lock` has **24** packages including `clap` + `clap_complete`; cold release build in a `git archive` clone on this machine took **2 m 15.9 s**. The docs predate commit `cb15e29 feat(cli): clap front end`. | `grep -c '^\[\[package\]\]' Cargo.lock` → 24; `cat crates/flightstrip-cli/Cargo.toml` → `clap`, `clap_complete` | OPEN |
| B-003 | P2 | web build | No `npm run lint` script exists (`npm run lint` → "Missing script: lint"), so the QA gate has no static-analysis step for the website. `tsc -b` runs inside `npm run build` only. | `cd web && npm run lint` | OPEN |
| B-004 | P0 | deploy | Nothing is live. `flightstrip.pages.dev` does not resolve (NXDOMAIN); no wrangler.toml, no deployment record, and no page URL anywhere in the tree. Criterion 6 (a judge can open it) currently fails. | `nslookup flightstrip.pages.dev` | OPEN |
| B-007 | P2 | compliance | `.cargo/config.toml` is committed with two absolute paths under `C:/Users/Admin/.rustup/...`. Harmless on a stock MSVC Windows box (the section only applies to the gnullvm target), but it is a personal path in the repo and a hard failure for anyone else building for gnullvm. | `grep -n 'C:/Users' .cargo/config.toml` | OPEN |
| B-008 | P3 | repo hygiene | `web/public/sessions/synthetic.json` is a 472 KB untracked stress fixture dated Nov 16. It is gitignored and `sync-fixtures.mjs` does not copy it into `dist/`, so it cannot leak to the site, but it is stale local state. | `ls -l web/public/sessions` | OPEN |

## Fixed

| ID | Sev | Area | Summary | Fix | Status |
|---|---|---|---|---|---|
| B-001 | P0 | environment | `rustup` had no installed toolchains and `~/.rustup/toolchains` did not exist, so `cargo fmt/clippy/test/build` could not run at all and every engine gate was unrunnable. `.cargo/config.toml` expected `stable-x86_64-pc-windows-gnullvm` plus the gnu sysroot. | Installed `stable-x86_64-pc-windows-gnullvm` and `stable-x86_64-pc-windows-gnu` (`--profile minimal`) and added the `clippy` + `rustfmt` components. `rustc 1.100.0`. This is machine state, not a repo change. | FIXED |
| B-005 | P2 | qa harness | `scripts/qa.ps1` reported `W5 web unit + integration tests` as PASS with the detail "no summary parsed" — the ANSI-coloured vitest summary was never matched, so the check would have passed on a run whose count was not displayed. | Stripped ANSI before matching `Tests (\d+) passed`; the count is now shown, and an unparsed summary still fails the detail honestly. | FIXED |
| B-006 | P2 | qa harness | First version of `scripts/qa.ps1` aborted on the first native command that wrote to stderr (`NativeCommandError` under `$ErrorActionPreference = 'Stop'`), so it could never have reported a FAIL table. | Switched to `'Continue'` and detect failure by exit code only. | FIXED |

## Verified this session (not bugs)

- `cargo fmt --check` clean; `clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace`: 19 + 125 + 1 + 6 = **151 passed, 0 failed, 0 ignored**.
- `cargo build --workspace --release`: exe **1,467,904 bytes** (matches the size claimed in NOTES.md).
- `npm ci` from the lockfile: 177 packages, `npm audit` reports 3 advisories (1 moderate, 2 critical) — to triage in the ship check, not force-upgraded.
- `npm run build`: JS 279,765 B raw / **87,354 B gzip**, CSS 27,978 B / 6,237 B → 93,591 B total, under the 200 KB gzip target.
- `npx vitest run`: 19 files, 129 tests passed.
- Clean clone (`git archive HEAD` → `%TEMP%\fs-qa\...`): both stacks build cold, `dist/sessions/` is populated by `prebuild`, `Cargo.lock` and `package-lock.json` are tracked, no untracked asset is required.
