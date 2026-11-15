# Notes

Design decisions, what they cost, and what was measured. Written while building,
so the numbers are from this machine (Windows 11, Rust 1.100, release profile
`lto = "thin"`, `codegen-units = 1`, `strip = true`).

## The dependency decision

The project started with the conventional crate set: `serde` + `serde_json`,
`chrono`, `blake3`, `similar`, `notify` + a debouncer, `ignore`, `regex`, `clap`,
`anyhow`, `thiserror`, `ctrlc`, `lz4_flex`. That is a reasonable default, and it
cost a 112-package lockfile and a build that needed a linker setup that kept
failing on this machine.

The goal changed to "as lightweight as possible", so both Rust crates now have an
**empty `[dependencies]`** section apart from the internal path dependency. The
lockfile has exactly two packages: the two workspace members.

| Removed | Replacement | What it cost |
|---|---|---|
| `serde`, `serde_json` | `json.rs`: a JSON parser + compact/pretty writer, ~450 lines | Field names are typed twice (writer and reader) per type; no derive, so adding a wire field means touching both. Guarded by round-trip tests. |
| `chrono` | `time.rs`: Hinnant's `days_from_civil` / `civil_from_days`, ~90 lines of date math | No time zones beyond a numeric offset, no calendars, no locale. The recorder only needs UTC RFC 3339 and a compact stamp. |
| `blake3` | `hash.rs`: 128-bit FNV-1a, 32 hex chars | Not cryptographic, and not collision-resistant against an adversary who can choose file contents. That is fine for change detection in your own repository; it would not be fine as a security primitive. |
| `similar` | `differ.rs`: Myers O(ND) with a 512-edit budget | A file rewritten more than 512 edits deep is reported as a whole-file replacement instead of a minimal diff. Real agent edits are a handful of lines, so the budget is generous in practice. |
| `notify` + debouncer | `scan.rs`: a polling scanner | 500 ms latency and one directory walk per interval. No inotify/kqueue/FSEvents/`ReadDirectoryChangesW` per platform, no debouncing state machine, and identical behaviour everywhere. |
| `ignore` | `paths.rs`: own `.gitignore` matcher (~120 lines) | Root `.gitignore` only, no nested files, no character classes. `[a-z]` patterns silently do not match. |
| `regex` | Hand-written matchers in `testparse`, `redact` and the detectors | More code, and markers are substring/shape based rather than fully general patterns. Each rule reads as the thing it detects. |
| `clap` | `main.rs`: a loop over `Vec<String>` | No generated completions, no man pages, no `--help` for free. Six subcommands did not justify the library. |
| `anyhow`, `thiserror` | `error.rs`: a `String` alias plus a `Context` trait | No error source chain backtraces; the CLI prints one line and exits 1. The core still has a small typed `Error` enum for library callers. |
| `lz4_flex` | Nothing — snapshots are plain LF-normalised bytes | The store uses more disk. Deduplication by content hash means repeated saves of the same file cost nothing extra, and `.flightstrip/` is disposable. |

### Measured

| | Before | After |
|---|---|---|
| Packages in `Cargo.lock` | 112 | 2 |
| Release binary | not recorded | 852,480 bytes (832 KiB), stripped |
| Cold debug build | ~25 s | 12.1 s |
| Cold release build | not recorded | 29.1 s |

These are build-time and size numbers on one machine, not a performance claim.
Runtime work — hashing, diffing, polling — was not benchmarked, and no speedup is
asserted for it.

## Data model

**Events are the source of truth.** `.flightstrip/sessions/<id>/events.jsonl` is
append-only, one JSON object per line, with `seq` and a session-relative `t_ms`.
Relative time means a clock change mid-session cannot produce a negative gap, and
it makes the fixture timelines reproducible. The absolute start lives in
`record.json`, which is the only place it is needed.

A torn last line from a crash is skipped when reading. That is why the reader
tolerates it rather than failing: losing one event beats losing the recording.

**Content addressing.** Snapshots are stored under
`.flightstrip/objects/<first two hex>/<hash>.snap`. The two-character shard keeps
NTFS directories small. Writes go to a temp file in the same directory and are
renamed, so a crash cannot leave a half-written object behind. Content is
LF-normalised *before* hashing, so CRLF and lone-CR inputs collapse to the same
object and the same line endings cannot produce a phantom diff.

**Deletes need a memory.** A path that vanishes only becomes a `delete` event if
the recorder had recorded it. Editor atomic saves (write temp, rename over) and
tooling scratch files otherwise show up as spurious deletions.

**Deterministic output.** `SessionExport::files`, `SessionStats::incidents_by_kind`
and `FileSummary::ops` are `BTreeMap`s, not `HashMap`s, so the same recording
produces byte-identical `session.json` on every run and platform. The wire format
did not change. Incident ids are assigned after sorting by step, never by hash
order.

## Detector philosophy

Every detector is a pure function of `(events, diffs)`: no I/O, no clock, no
randomness. That is what makes the export reproducible, and it makes each rule
testable with three lines of input.

Rules favour precision over recall. A false positive in a PR review summary is
expensive — it teaches the reviewer to skip the section. `rm -rf node_modules` is
a warning, `rm -rf /` is critical, `rm file.txt` is silent; a hash-looking string
is not a leaked credential; a long identifier is not a secret.

Evidence is **redacted before it is attached to an incident**, so recording a
secret does not create a second place to leak it. `<.env` paths are named, because
a reviewer needs to know a secret-bearing file was touched, but no values are.

## Fixtures

`fixtures/*.json` are generated by `cargo run -p flightstrip-core --example fixtures`,
which builds real events, real snapshot objects and a real session directory, then
calls the same exporter the CLI calls. Hand-writing session JSON means every
schema change silently invalidates the fixtures and the website is the first
thing to find out. `crates/flightstrip-core/tests/fixtures.rs` re-parses all three
on every test run, checks that every incident points at an existing step, and
asserts the specific incidents each fixture is supposed to demonstrate.

Three sessions, chosen to cover the range: a clean refactor (nothing detected,
tests green throughout), a session that goes green by removing assertions and
skipping a test, and a session that touches `.env`, adds a literal secret, adds
dependencies, rewrites one file five times and ends with a force-push.

## Website

The replay page was rebuilt as a Vite + React + TypeScript app, replacing the
no-build `index.html` + `style.css` + `app.js` page. **This reverses the earlier
decision** that the page should have no npm dependencies and no build step, which
this document argued for. The reversal is deliberate and the cost is real, so it
is recorded rather than quietly dropped.

| | Before | After |
|---|---|---|
| Build step | none | `vite build` |
| Packages | 0 | ~180 in `node_modules` (dev-only except React) |
| Page source | 3 files, ~53 KB | ~30 modules under `web/src/` |
| Type safety | none | TypeScript strict, checked in `tsc -b` |
| JS shipped | ~30 KB raw | ~267 KB raw / ~84 KB gzip |

What the build bought: the strip chart, the CVR drawer, the trace graph and the
report are separate components with typed interfaces, and the wire format is
written down once in `types.ts` instead of being re-derived in every render. The
chart's geometry (`layout.ts`, `downsample.ts`) and the particle maths
(`particles.ts`) are pure functions with tests, which the old page could not
have — `web/smoke.mjs` could only assert that the DOM had been built, never that
a coordinate was right.

What it cost: a dependency tree, a build step, and the offline-from-`file://`
property. `web-legacy/` holds the old page until the new one is verified.

`web/smoke.mjs` was rewritten. The old one ran the real client against a stubbed
DOM; the new one checks the built `dist/` (shell, copied session data,
self-hosted fonts, size budget), and the rendering check moved to
`src/app.integration.test.tsx`, which mounts the whole page against the real
fixtures. The reason for the split: jsdom does not execute
`<script type="module">`, so running the *built* page headlessly never actually
ran the app — every check failed for the wrong reason. Mounting the real tree in
vitest tests the same behaviour and executes.

Session data is still never hand-written. `web/scripts/sync-fixtures.mjs` copies
`fixtures/*.json` into `web/public/sessions/` at prebuild and writes the
catalogue manifest from `web/sessions.json`, so the fixtures remain the single
source of truth and the page cannot drift from the schema.

The obvious cost of no browser in the development environment remains: layout
was not seen. The integration test cannot catch a visual bug either. That gap is
still open, and browser verification in CI is still the fix (see *Next, in
order*).

## What is deliberately missing

- **No pruning/compaction command.** `.flightstrip/` grows until deleted. The
  right shape is `flightstrip gc --keep-sessions N`, not an automatic policy that
  could delete evidence.
- **No prompt capture from a terminal.** `mark` is manual by design: parsing
  another tool's TUI is brittle and would need a PTY layer. `PromptSource::Pty`
  exists in the schema for when that is worth doing.
- **No schema validation on read beyond the parser.** `SessionExport::parse`
  reports the first missing or wrong-typed field, which is what a human needs;
  there is no JSON Schema document for other clients.
- **No per-file incident grouping or severity configuration.** Severities are
  fixed in the rules.
- **No session-to-session comparison.** Diffing two exports (what did the second
  attempt change?) is the obvious next feature and is not implemented.

## Next, in order

1. `gc` for the snapshot store, with `--keep-sessions`.
2. Compare two sessions: same prompts, different outcomes.
3. Nested `.gitignore` support, since monorepos will hit the current limit.
4. A `--pty` wrapper for agent CLIs, so `mark` can be automatic.
5. Browser verification in CI, so the replay page's layout is checked somewhere.
