# Build log

A template, not a history. Each entry is one working session: what was asked of
the assistant, what came back, and what a human changed afterwards. Nothing in
this file is reconstructed from memory — an entry exists only if someone wrote it
down at the time.

Rule for filling it in: if you cannot point at the commit, the command, or the
transcript, do not write the row.

---

## YYYY-MM-DD — <short title>

**Prompt given to Freebuff**

> Paste the prompt verbatim. If it was a follow-up, paste the follow-up too.

**What it produced**

- Files created or changed (paths, and roughly what each does).
- Commands it ran and their result.
- What it claimed without evidence.

**What I changed by hand**

- Edits made after the assistant stopped, and why.
- Anything reverted.

**Verification at the end of the session**

- `powershell -ExecutionPolicy Bypass -File scripts\qa.ps1` → result
- `cd web && npm run build` → result
- Anything that could not be checked, and why.

---

## 2026-11-20 — Stage 0 to 3: discovery, baseline, engine tests, contract

**Prompt given to Freebuff**

> "Finisher" brief: verify, test live, fix bugs, then add features. Stage 0
> discovery and compliance gate; Stage 1 baseline; Stage 2 engine tests; Stage 3
> contract test. Read-only until "Proceed".

**What it produced** (all commits below are in this repository)

| Commit | Change |
|---|---|
| `5c7c884` | `scripts/qa.ps1` (single gate, PASS/FAIL table, non-zero exit), `docs/qa/BUGS.md` |
| `f10b271` | JSON depth limit (B-009) + regression test |
| `f506e96` | `scripts/qa-engine.ps1` — 39 engine checks in throwaway repos |
| `26671eb` | `scripts/scenarios/*.ps1`, `scripts/validate-sessions.mjs`, `fixtures/staged/` |
| `b1b897c` | NOTES dependency/build claims corrected (B-002) |
| `eb80912` | favicon, social card, page metadata, `npm run lint`, wrangler |
| `b26e6eb` | machine-local `.cargo/config.toml` untracked, example committed (B-007) |

**What I changed by hand**

- Installed the Rust toolchain the machine was missing (rustup had none).
- Recorded that the four scenario sessions are killed with `Stop-Process`, so
  they carry no `session_end`. The data is real; the missing end marker is the
  one artefact of being scripted, and it is stated in the scenario header.
- The two dangerous commands in `staged-thrash-danger.json` are written into the
  log, never executed, and the export is relabelled `fixture`.

**Verification at the end of the session**

- `scripts/qa.ps1` → 54 checks, 54 pass (before the last two commits; re-run
  recorded in `docs/qa/QA_REPORT.md`).
- Local production build served with `vite preview`: `/`, `/sessions/index.json`
  and `/sessions/weakened-tests.json` all 200; no external URL in `dist/index.html`.
- NOT verified: any rendering in a browser (no browser on this machine), the
  deployed site (Cloudflare credentials absent), and Ctrl+C (see BUGS.md).
