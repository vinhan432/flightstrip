# Flightstrip Replay — The Flight Recorder

**Date:** 2026-11-15
**Status:** design, awaiting approval
**Scope:** the `web/` replay page only. The Rust recorder is not changed.

---

## 1. Intent and why

Flightstrip already records what an AI coding agent did: every file change,
command, test run and the deterministic incidents they add up to. It exports one
`session.json`. What it does not yet have is a replay page that makes the
recording *felt* — the current `web/` page is a competent, quiet document that
shows the data but does not make anyone lean in.

This work replaces that page with an instrument. The page is judged on UI, UX,
originality and "not AI slop", so the bar is not "reads well" but "the first
load is a moment". The concept is the one the product already earned: **the
inside of a cockpit flight recorder.** Warm paper, ruled graph lines, precise
instruments, ink-on-paper traces, and exactly one signal colour.

The page has one job: take a `session.json` and let a person *watch the session
happen*, then walk away knowing what the agent actually did — including the
things a final diff hides.

### Who it is for

- A developer reviewing an agent session before merging, who wants to see the
  moment a test went red and whether it went green again for the right reason.
- A team lead who needs to hand a suspicious session to a colleague and have the
  suspicion be legible in ten seconds.
- A judge at the hackathon, who is the harshest case: they will load it once and
  decide.

### Why now

The recorder, the schema, the detectors and three real fixtures all exist and
are tested. The only missing piece is the surface that makes them undeniable.

---

## 2. Goals, non-goals, anti-goals

**Goals**

1. A first load that is unmistakably an instrument, not a dashboard. The hero is
   a full-viewport moment.
2. The strip chart is the product: a real, scrubbable, zoomable, playable
   timeline of the session, with the file trace as its spine.
3. Every incident is traceable to the exact step, prompt and diff line that
   caused it, in one click.
4. It tells the truth about the real schema. It loads the real fixtures.
5. It is deployable as a static site on Cloudflare Pages with no server.

**Non-goals**

- Changing the Rust recorder, the detectors, the schema, or the fixtures.
- A server, a database, auth, or any API key. No model in the loop.
- Comparing two sessions (that is a recorder feature, listed in NOTES.md).
- Live recording from the browser.

**Anti-goals** — these make the work a failure even if it technically runs:

- Looking like a generic dark-mode SaaS dashboard. No glassmorphism, no neon
  glow, no purple gradient, no emoji.
- Fake data. A hand-written demo session that drifts from the schema, or
  placeholder binaries that download and do nothing.
- The instrument being decoration. If the chart is a static picture that only
  looks like a chart, the point is lost.
- Losing the project's honesty. The README's tone — "detectors never claim the
  change is correct" — must survive into the UI.

---

## 3. Constraints

- **Stack (decided):** Vite 6 + React 19 + TypeScript 5. No CSS framework, no
  animation library, no component library. Custom CSS, CSS keyframes,
  `requestAnimationFrame`, Web Animations API. Canvas 2D for particles and the
  trace; SVG for the graph and overlay.
- **Fonts:** self-hosted via `@fontsource` — Space Grotesk (headings) and IBM
  Plex Mono (data/body). No external font fetch at runtime.
- **three.js: not used.** The spec's Canvas 2D + SVG approach wins; WebGL fights
  the analog-paper concept and the perf budget.
- **Data:** the real `session.json` contract, produced by
  `crates/flightstrip-core/src/schema.rs`. The real fixtures are the demo data.
- **Routing:** hash-based, `#/session/<id>` and `#/session/<id>/<focus>`, where
  `<focus>` is an incident id or a step index, so a link lands on the exact
  finding. No server rewrites needed.
- **Target:** <200KB JS gzipped, first paint <1s, 60fps interactions, 30fps
  particles.
- **Accessibility:** WCAG AA, full keyboard control, `prefers-reduced-motion`
  honoured, colour never the only signal.
- **Deploy:** the deliverable is the local build at `web/dist`. The
  `npx wrangler pages deploy dist --project-name flightstrip` command is
  documented for a maintainer to run with their own Cloudflare credentials; no
  credentials are used or required in this work.

---

## 4. The data contract (the part that must be right)

This is the real shape, read from `schema.rs` and the fixtures. It differs from
the brief's invented `demo-session.json`; the brief is overruled here.

```
SessionExport {
  version: 1
  meta:   { id, title, agent?, started_at, duration_ms, os, repo_name, label }
  prompts: [ { id, text, step, t_ms, files_touched, commands_run, incidents } ]
  steps:  [ { seq, t_ms, kind, ...kind fields, hunks? } ]
  incidents: [ { id, kind, severity, step, t_ms, title, evidence,
                 why_it_matters, prompt_id?, path? } ]
  files:  { "<path>": { churn, prompts: [id], ops: { <op>: n } } }
  stats:  { steps, files_touched, commands, test_runs,
            incidents_by_kind: {}, tests_first_ok?, tests_last_ok? }
}
```

`step.kind` is one of: `session_start`, `prompt`, `file_change`, `command`,
`test_run`, `note`, `session_end`.

Enums that matter to the UI:

- `file_change.op`: `create` | `modify` | `delete` | `rename`
- `incident.kind`: `test_weakened` | `test_regression` | `thrashing` |
  `dangerous_command` | `dependency_added` | `secret_touched` | `scope_creep`
- `incident.severity`: `info` | `warn` | `critical`
- `diff line.tag`: `context` | `add` | `remove`

### Three facts a naive implementation gets wrong

1. **`steps[0]` is absolute, every later step is relative.** `session_start`
   carries an epoch `t_ms` (e.g. `1794653100000`); the rest are milliseconds
   since start (e.g. `1500`). The time axis is built from `steps[1..]` and
   `meta.duration_ms`; `steps[0]` is the anchor, never plotted.
2. **`incident.step` is an index into `steps`, not a `seq`.** The fixtures use
   them interchangeably because they coincide; the code must not assume that.
3. **`files` is an object, not an array**, and `prompts` inside it are ids that
   must be joined back to `prompts[]` for text.

### Data flow

`fixtures/*.json` stay the single source of truth. A prebuild step copies them
into `web/public/sessions/` and writes an `index.json` manifest from
`web/sessions.json`, which stays committed at the repo root of `web/` as the
home of the hand-written catalogue prose (title, hook, blurb) — the fixtures
carry `meta.title` but no hook. `web/public/sessions/` is generated and
git-ignored, so the build survives the `web-legacy/` cleanup. Nothing is
hand-written; the fixtures test in
`crates/flightstrip-core/tests/fixtures.rs` keeps guarding them.

---

## 5. Approaches considered

The five decisions that shaped this, and what was chosen.

### 5.1 Toolchain — Vite + React + TS (chosen)

The project's README and NOTES.md are explicit that the page has *no npm
dependencies and no build step*, and that this is a deliberate trade. The brief
asks for the opposite. Three options were put up:

| Option | Trade |
|---|---|
| Keep zero-dependency vanilla | Preserves the project's stated identity and keeps `smoke.mjs` alive. Authoring a 7-section instrument with a canvas chart, drawer and graph in raw DOM is slower and more error-prone. |
| **Vite + React + TS (chosen)** | Component model and types pay off on the strip chart, CVR panel and graph. Costs a build step, a dependency tree, and a rewritten `smoke.mjs`. Contradicts README/NOTES. |
| React via ESM CDN, no bundler | Middle path, off-spec, unusual to maintain. |

**Chosen: Vite + React + TS**, because the brief is explicit and the component
model genuinely helps the chart. **The cost is accepted knowingly:** README.md
and NOTES.md must be updated so the project does not document a constraint it no
longer holds. This is the single biggest reversal in the work and it is
deliberate, not an oversight.

### 5.2 Where it lives — replace `web/` in place (chosen)

`web/` already holds a working page and there is **no git repository**, so an
overwrite is unrecoverable. Chosen: replace in place, but **first** `git init`
and copy the current page to `web-legacy/` as a safety net, then remove
`web-legacy/` once the new page is verified. The final artifact is one page, not
two.

### 5.3 Data — real fixtures and the real schema (chosen)

The brief's `demo-session.json` (50+ steps, invented) is rejected. NOTES.md is
explicit that hand-written session JSON "silently invalidates the fixtures". The
site consumes `clean.json`, `weakened-tests.json` and `risky-session.json`
through the real contract. The brief's "50+ steps, 2 incidents" demo is
satisfied by `risky-session.json`, which has more steps and more incidents than
the brief asked for, and is real.

### 5.4 Download CTA — build from source (chosen)

There is no published crate and no GitHub release, so `cargo install flightstrip`
and placeholder binaries would both be lies. Chosen: the section shows the real
install path —

```
cargo build --release
# → target/release/flightstrip
```

— with a copy-to-clipboard button, plus the real command list from the README
(`watch`, `mark`, `exec`, `export`, `report`). The `DownloadButton` component
still auto-detects the OS and names the correct binary, but it links to the
build instructions rather than to a file that does not exist. If the local
`target/release/flightstrip.exe` is wanted as a genuine secondary download, that
is a one-line addition and a builder's call.

### 5.5 three.js — dropped (chosen)

The brief specifies Canvas 2D and SVG and never mentions three.js. WebGL would
fight the ink-on-paper aesthetic and the perf budget. The `@threejs-animation`
tag was automatic and does not apply.

---

## 6. The design

### 6.1 Visual identity

Palette (tokens in `styles/tokens.css`):

```
--paper:       #f4efe4    --ink:         #1c1917    --orange:      #d4621a
--paper-dark:  #e6dfd2    --ink-light:   #78716c    --orange-glow: #ff8c42
                          --ink-faint:   #c4b9a8    --red:         #b91c1c
                                                     --green:       #15803d
                                                     --green-muted: #bbf7d0
```

Type: Space Grotesk 700 for headings, tight tracking; IBM Plex Mono 300 for
body; Space Grotesk uppercase wide-tracked for stamps and labels.

Texture: an SVG-noise paper grain at 3% opacity, faint horizontal rules via
`repeating-linear-gradient`, a radial vignette on the hero. No glass, no glow,
no gradient mesh.

Motion: ink drawing in (`stroke-dashoffset`), a pen head tracing the chart, a
rubber-stamp drop, fade-up scroll reveals. All gated on
`prefers-reduced-motion`.

### 6.2 Sections

1. **Hero — "The Cockpit".** Full viewport. `ParticleField` canvas behind.
   Heading `SEE WHAT YOUR AI AGENT ACTUALLY DID.` in Space Grotesk 700,
   uppercase, `clamp(2.5rem, 7.5vw, 6.5rem)`, `--ink` on `--paper`, revealed
   letter-by-letter (120ms each, 40ms stagger) with a single `aria-label` on the
   `h1`. Subtitle, then **Watch a Replay** (primary) and **Download CLI**
   (secondary). A `DataPlate` badge top-right: version, a blinking `REC` dot,
   live step count and clock, fed from the active session.
2. **The Strip Chart — "The Recorder".** The centrepiece. Six tracks: time axis,
   prompts, files (canvas trace), tests, commands, incidents. SVG overlay for
   interactive marks, canvas for the trace, a pen head, a custom scrubber with
   incident dots, wheel zoom toward the cursor, drag-pan, and a `CVRPanel`
   drawer that opens on any step.
3. **Incident Report — "The Investigation".** A printed-document card with a
   rotated red stamp and a faint `CLASSIFIED` watermark. Verdict, timeline,
   probable cause, recommendations, and a print button using `print.css`.
4. **Trace Graph — "The Network".** Deterministic left-to-right SVG: prompt
   nodes, file circles sized by churn, incident triangles, curved edges that
   animate direction on hover. Clicking a node jumps the chart to its step.
5. **How It Works — "The Briefing".** Three cards: Record, Analyze, Review.
   Staggered fade-up on scroll.
6. **Download — "The Supply Drop".** The `DownloadButton`, OS-detected, with a
   cursor-following ripple and a checkmark on click, then the build-from-source
   command with copy-to-clipboard.
7. **Drop Zone — "Load Your Own".** Dashed drop target, drag-over highlight,
   schema validation, a toast on success, a shake and a red message on failure,
   plus a hidden file input for mobile.
8. **Footer.** Minimal: "Built with Freebuff" and a hackathon badge. No GitHub
   link until a remote exists; a dead link is a visible defect.

### 6.3 File structure

The brief's structure is followed, with two changes: `web-legacy/` is added as
the temporary backup, and `public/sessions/` holds copied fixtures plus a
generated `index.json` rather than a hand-written `demo-session.json`.

```
web/
  index.html  vite.config.ts  tsconfig.json  package.json
  public/
    _headers                      cache control for JSON
    sessions/                     copied fixtures + generated index.json
  src/
    main.tsx  App.tsx  types.ts  store.tsx
    hooks/    useSession usePlayback useKeyboard useHashRoute
              useScrollReveal useParticleCanvas
    components/ Hero ParticleField DataPlate ShortcutHelp Footer
      StripChart/ StripChart CanvasTrace PromptTrack TestTrack
                  CommandTrack IncidentTrack PenHead Scrubber TimeAxis
      CVRPanel DiffView IncidentReport TraceGraph SessionPicker
      DropZone DownloadButton
    styles/   tokens reset typography paper animations components print
    utils/    layout color format downsample particles
  scripts/    sync-fixtures.mjs     copies fixtures, writes index.json
```

### 6.4 Types

`src/types.ts` mirrors `schema.rs` exactly — the same union members and the same
optionality — and is the only place the wire shape is written in TypeScript. A
`parseSession` function validates an unknown object and returns either a
`Session` or a message naming the first bad field, so the Drop Zone can report
precisely what is wrong.

### 6.5 State

One React context (`store.tsx`) holds the loaded session, the playback clock,
zoom and pan, the selected step, and the drawer state. `usePlayback` owns a
single `requestAnimationFrame` loop that advances time at 1×/2×/4× and is the
only thing that mutates the clock; everything else derives from it.

### 6.6 Playback and keyboard

Autoplay when the chart scrolls into view (IntersectionObserver), from the
start. `Space` play/pause, `←/→` step, `J/K` previous/next incident, `+/-` zoom,
`0` reset, `Esc` close the drawer, `?` open `ShortcutHelp`. A `role="application"`
region documents the keys and an `aria-live="polite"` region announces the
current step.

### 6.7 Performance

- Particles: 100 points, O(n²) pair tests (~4,950/frame), frame-skipped to 30fps,
  paused off-screen and when the tab is hidden, one static frame under reduced
  motion, DPR capped at 2, `clearRect` each frame.
- Trace: downsampled to 2× viewport width when zoomed out; redrawn only when
  zoom, pan or clock changes.
- Overlay: only marks within the viewport ± one screen are rendered.
- Session JSON is fetched on selection, never all at load.
- Fonts preloaded, `font-display: swap`. Skeleton states while loading.

### 6.8 Responsive

Desktop: drawer on the right, graph visible. Tablet: drawer as a bottom sheet,
simplified graph. Mobile: single column, horizontally scrollable chart, drawer as
a full overlay, graph replaced with a "view on desktop" note, full-width
download button.

---

## 7. What is left to the builder

- Exact pixel geometry of the six tracks and the pen head animation timing.
- Whether to also offer the local Windows binary as a secondary download.
- The precise wording of the incident report's "probable cause" and
  "recommendations" sections, derived from incident kinds.
- The particle count and connection distance, within the stated budget.
- The `_headers` cache durations.
- Whether `web-legacy/` is removed before or after the deploy is verified.

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| The stack reversal contradicts README/NOTES | Update both files as part of the work; state the reversal in the PR. |
| No git repo, in-place overwrite is unrecoverable | `git init` and copy to `web-legacy/` before any edit. |
| `smoke.mjs` dies with the rewrite | Replace it with a headless check that boots the built page and asserts the chart, steps and drawer render. |
| Hand-written data drifts from the schema | Never hand-write; copy fixtures at prebuild. |
| The Download CTA has nothing real behind it | Build-from-source instructions, no fake binaries. |
| Fonts or assets blow the perf budget | Self-hosted woff2 subsets, `font-display: swap`, measured against the <200KB target. |
| The instrument becomes decoration | The chart is driven by the real clock and real events; every mark maps to a step. |
