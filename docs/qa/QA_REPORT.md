# QA report — Flightstrip

**As of** 2026-11-20 12:00 +07:00 (UTC+7). **Deadline (assumed)** Sun 22 Nov 23:59 +07.
**Build:** `main` at `b26e6eb`, 31 commits.

---

## 1. Executive summary

**Submittable now: WITH CONDITIONS.**

The engine is in good shape and is now covered by a 54-check gate that runs in a
fresh process tree and fails loudly. The website builds, its parser agrees with
the Rust reader on all 11 session files, and the bundle is well under budget.

Two conditions stand between this and a submittable entry:

1. **It is not deployed.** `flightstrip.pages.dev` does not resolve, there is no
   Cloudflare credentials file on this machine, and `wrangler whoami` reports
   "not authenticated". A judge has no URL to open.
2. **Nothing has been seen in a browser.** There is no Chrome, Edge automation or
   Playwright install here. Layout, focus rings, motion and the mobile
   breakpoints are asserted by unit and integration tests and by reading the CSS,
   not by looking at them.

Everything else in the brief that could be checked from a script was checked.

---

## 2. Scoreboard

Scored conservatively: a criterion that could not be observed in a browser is
capped at 3.

| # | Criterion | Score | One critical sentence |
|---|---|---|---|
| 1 | UI cohesion and detail | 3/5 | The token set, type scale and three-colour discipline are real and consistent in code, but no screen was rendered here, so "cohesive" is read, not seen. |
| 2 | UX: navigation and smoothness | 3/5 | Hash routing, keyboard control and the CVR drawer's focus trap have tests; the first-ten-seconds experience and the drop zone's error copy were not exercised against a real browser. |
| 3 | Features that work | 4/5 | Six detectors, redaction, three report formats and `verify` all behave as documented under test; `flightstrip view`, the TUI and `--html` do not exist in this build. |
| 4 | Originality | 4/5 | A flight-recorder strip chart as the interface for agent auditing is a genuinely specific idea, and the copy is about the work, not about the tool being clever. |
| 5 | Creativity | 4/5 | The chart, pen head, incident stamps and CVR metaphor carry the product; the new social card shows the same language off the page. |
| 6 | Availability | 1/5 | The live URL does not exist. This is the only P0 left. |

---

## 3. Verified / partial / broken / not verified

**Verified by running it (this session, on this machine)**

- `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`: **152 passed, 0 failed, 0 ignored**.
- `cargo build --workspace --release`: exe **1,468,416 bytes**.
- `npm ci`, `npm run lint`, `npm run build`: clean; **129** web tests in 19 files.
- `scripts/validate-sessions.mjs`: **11 files, 11 agree, 0 disagree**.
- `scripts/qa.ps1`: **54 checks, 54 passed, 0 failed**, exit 0.
- `scripts/qa-engine.ps1`: **39 checks, 38 pass, 0 fail, 1 documented skip** — lifecycle, attribution, CRLF/metadata/atomic-save semantics, ignored trees, 2 MB and binary files, all six detectors, redaction, corrupt logs, report formats, `verify`, `doctor`/`status`/`init`/`list --json`/`prune --dry-run`, ANSI discipline, 10 fuzz inputs.
- Clean clone via `git archive`: both stacks build cold; `dist/sessions/` is populated by `prebuild`.
- Local production build (`vite preview`): `/` 200 `text/html`, `/sessions/index.json` 200, `/sessions/weakened-tests.json` 200 (9,643 B); **no external URL** in `dist/index.html`.
- Generated `og.png`: valid PNG, 1200×630, 6,703 B.

**Partial**

- **Secret handling.** `--redact` removes all four test tokens and leaves no
  `.env` body; without `--redact` all four are present in the export. That is the
  documented behaviour, and it means an export is only safe to share if you ask
  for redaction.
- **Detection precision.** A moved assertion does not fire; a *rewritten* one
  does, naming the old line. Intentional, but a reviewer will see it.
- **`dependency_added`.** Works on multi-line manifests (`3 packages added ·
  alpha, beta, gamma`), blind to a minified single-line `package.json` (B-011).

**Broken**

- Nothing on the main path. Two robustness gaps remain open as P3s (BOM-prefixed
  JSON, a corrupt middle line in `events.jsonl`).

**Not verified**

- Any browser rendering: layout, console errors, Lighthouse, DPR, `prefers-reduced-motion`, focus order, colour contrast in situ.
- The deployed site (no credentials), its cache headers, and a fresh-profile load.
- Ctrl+C closing a session (documented skip; the crate test covers the disk half).
- `flightstrip view` / TUI — the subcommands do not exist in this build, so the
  brief's viewer tests are not applicable rather than failed.
- `cargo audit` / `npm audit --json` triage: `npm ci` reports 3 advisories (1
  moderate, 2 critical) in dev tooling; not force-upgraded.

---

## 4. Bugs

**Fixed this session**

| ID | Sev | Commit | What |
|---|---|---|---|
| B-001 | P0 | — | Rust toolchain absent on the machine; no cargo command could run. Installed gnullvm + gnu + clippy/rustfmt. |
| B-009 | P2 | `f10b271` | 5,000-deep JSON killed `verify`/`report` with a stack overflow. Depth capped at 256, with a regression test. |
| B-002 | P1 | `b1b897c` | NOTES claimed a 2-package, dependency-free build. It has 24 packages and a 2m16s cold build. |
| B-007 | P2 | `b26e6eb` | `.cargo/config.toml` committed personal `C:/Users/Admin/` paths. Now gitignored with a placeholder example. |
| B-003 | P2 | `eb80912` | No `npm run lint`. Added (`tsc -p tsconfig.json`, `noEmit`). |
| B-005, B-006 | P2 | `5c7c884` | The gate itself could report PASS without evidence, and aborted on stderr. |
| B-008 | P3 | `eb80912` | Stale 472 KB `synthetic.json` was being copied into `dist/`. Removed; regenerable with `npm run synthetic`. |

**Open**

| ID | Sev | What | Why it is not fixed |
|---|---|---|---|
| B-004 | **P0** | No deployment | Needs `wrangler login` in a browser; no credentials on this machine. |
| B-010 | P3 | BOM-prefixed `session.json` rejected | Readable error, not a crash. |
| B-011 | P3 | Minified single-line `package.json` hides `dependency_added` | Detector reads added lines; a real fix is a JSON-aware manifest diff. |
| B-012 | P3 | Corrupt middle line in `events.jsonl` skipped silently, exit 0 | A torn *last* line is documented; a mid-file one is not. |

---

## 5. Measurements

| Metric | Value | How |
|---|---|---|
| Rust tests | 152 passed / 0 failed / 0 ignored | `cargo test --workspace` |
| Release exe | 1,468,416 B | `stat` |
| Cold release build (clean clone) | 2 m 15.9 s | `time cargo build --release` in `%TEMP%` |
| Web tests | 129 passed in 19 files | `npx vitest run` |
| JS shipped | 279,765 B raw / **87,354 B gzip** | `GZipStream` + `gzip -9` |
| CSS shipped | 27,978 B raw / **6,237 B gzip** | same |
| Budget | 93,591 B of 200,000 (47%) | `scripts/qa.ps1` W2 |
| `dist/` total | 885 KiB | `du -sh` |
| Social card | 1200×630, 6,703 B | `scripts/make-og.mjs` |
| Gate | 54 checks, 54 pass | `scripts/qa.ps1` |
| Contract | 11 files, 0 disagreements | `scripts/validate-sessions.mjs` |

Not measured: frame times, Lighthouse scores, interactivity timing, memory. No
browser was available, so none of these have numbers and none are claimed.

---

## 6. Overclaims corrected

- NOTES.md: "both Rust crates now have an empty `[dependencies]`", "the lockfile
  has exactly two packages", "`clap` … replaced by a loop over `Vec<String>`",
  "cold release build 29.1 s". All four were true before `cb15e29` and are false
  now; the file says so explicitly.
- README.md: the `.cargo/config.toml` note now describes the real, three-command
  workaround and states the file is machine-local.
- `docs/superpowers/plans/…`: contained a `cd C:/Users/Admin/Desktop/hackathon/…`
  line; genericised.

Still accurate and worth keeping: "Attribution is a heuristic", "Test parsing is
guesswork", "Redaction is best-effort … not a security boundary", "macOS and Linux
builds are untested", "no backend, no database, no API keys".

---

## 7. Top risks

| Risk | Mitigation |
|---|---|
| **No live URL at judging time** (P0) | One `wrangler login`, then `npx wrangler pages deploy dist --project-name flightstrip`; recheck `/`, `/sessions/index.json`, one session. |
| **A layout bug that no test can see** | Open it once in a real browser before submitting; the four widths and the drop-zone errors are the highest-risk screens. |
| **A false critical in the detector** teaches judges to distrust it | Already limited: moved assertions do not fire, `rm -rf node_modules` is a warning. The rewritten-assertion case is documented rather than hidden. |
| **Redaction is not a boundary** | The copy says so. Never ship an export of a real key without checking it. |
| **Pre-event commit timestamps** | All 24 pre-existing commits are dated 15–17 Nov; seven more were made on 20 Nov during this session. Neutral, uneditable history — state it, do not spin it. |

---

## 8. Manual checks for a human

1. **Deploy**, then open the URL from a phone on mobile data (not the same
   network) at 360 px wide.
2. **First ten seconds**: does a session play on its own, and can a stranger say
   what the page is?
3. **Drop zone**: a valid export, `{}`, a random JPEG, and a 26 MB file.
4. **Ctrl+C**: `flightstrip watch` in a scratch repo, press it once — expect
   "closing the session", a summary, and `.flightstrip\active` gone.
5. **Report page**: print preview, then `Ctrl+P` to PDF — the paper styling is
   the thing most likely to break.
6. **A real Freebuff run**: record an actual session with `mark` + `exec`, export,
   and open it on the site.
7. **GitHub repo public** and the README's five-line quickstart actually works
   from a clean clone on a machine without MSVC.
