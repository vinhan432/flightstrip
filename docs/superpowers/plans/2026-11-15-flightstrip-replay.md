# Flightstrip Replay — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `web/` with a Vite + React + TS replay page that renders a real `session.json` as a paper strip chart you can scrub, play and inspect, then presents its incidents as an aviation-style investigation report.

**Architecture:** A single React app. One context (`store.tsx`) holds the loaded session, the playback clock, viewport (zoom/pan), selection and drawer state; one `requestAnimationFrame` loop in `usePlayback` is the only writer of the clock and everything else derives from it. The strip chart is a canvas (file trace) under an SVG overlay (prompts, tests, commands, incidents, pen head). Fixtures are copied into `public/sessions/` at prebuild, never hand-written.

**Tech Stack:** Vite 6, React 19, TypeScript 5, `@fontsource/space-grotesk`, `@fontsource/ibm-plex-mono`. No CSS framework, no animation library, no component library. Canvas 2D + SVG only.

**Spec:** `docs/superpowers/specs/2026-11-15-flightstrip-replay-design.md`

## Global Constraints

- Node ≥ 20; Vite 6; React 19; TypeScript 5 strict.
- **No** Tailwind/Bootstrap/shadcn, **no** Framer Motion/GSAP, **no** component library. CSS keyframes, transitions, rAF, Web Animations API only.
- **No three.js, no WebGL.** Canvas 2D + SVG only.
- Fonts self-hosted via `@fontsource`; `font-display: swap`; no runtime font fetch.
- Palette tokens exactly: `--paper #f4efe4`, `--paper-dark #e6dfd2`, `--ink #1c1917`, `--ink-light #78716c`, `--ink-faint #c4b9a8`, `--orange #d4621a`, `--orange-glow #ff8c42`, `--red #b91c1c`, `--green #15803d`, `--green-muted #bbf7d0`.
- Headings Space Grotesk 700, tracking `-0.02em`; data/body IBM Plex Mono; stamps Space Grotesk uppercase tracking `0.15em`.
- No glassmorphism, no neon glow, no purple gradients, no emoji.
- `prefers-reduced-motion: reduce` disables every animation and shows a static state.
- WCAG AA contrast; colour is never the only signal (incidents carry shape + label).
- Budget: <200KB JS gzipped, first paint <1s, 60fps interaction, 30fps particles.
- Deploy target: `npx wrangler pages deploy web/dist --project-name flightstrip`.
- **Data is never hand-written.** `web/public/sessions/*.json` are produced by `web/scripts/sync-fixtures.mjs` from `fixtures/`.

## Review Focus

The spec implies these inputs and conditions but assigns no test to them. Each is pinned to its owning task below.

1. **`steps[0].t_ms` is an absolute epoch while every later step is relative** (`weakened-tests.json` line 54 = `1794653100000`, line 63 = `1500`). A time axis built from all steps produces a ~1.8e12 ms domain and a flat line. Expected: the axis spans `0..meta.duration_ms` and `session_start` is never plotted. → Task 4.
2. **`incident.step` indexes `steps`, not `seq`.** They coincide in the fixtures, so a `seq`-based lookup passes the demo and breaks on any real export. → Task 4.
3. **`files` is an object keyed by path, and its `prompts` are ids.** Treating it as an array, or rendering ids, shows `p-5` instead of the prompt text. → Task 3.
4. **A malformed or truncated session JSON.** The Drop Zone must name the first bad field and shake, not throw into a blank page. → Task 11.
5. **A session with zero incidents, zero prompts, or a single step** (`clean.json` has a prompt with `files_touched: 0`). Empty tracks, an empty incident report and a one-point trace must render without NaN geometry or a crash. → Tasks 4 and 8.
6. **A deep link naming a focus that does not exist** (`#/session/risky/i-99`, or an incident id from a different session). Expected: load the session, ignore the unknown focus, open no drawer — not a blank page. → Task 7.

---

### Task 1: Scaffold, safety net, and design tokens

**Files:**
- Create: `web/package.json`, `web/vite.config.ts`, `web/tsconfig.json`, `web/tsconfig.node.json`, `web/index.html`, `web/src/main.tsx`, `web/src/App.tsx`
- Create: `web/src/styles/tokens.css`, `reset.css`, `typography.css`, `paper.css`, `animations.css`
- Create: `web/.gitignore`
- Move: current `web/index.html`, `web/style.css`, `web/app.js`, `web/serve.js`, `web/smoke.mjs`, `web/sessions.json` → `web-legacy/`

**Interfaces:**
- Produces: a dev server on `:5173` and a build to `web/dist`; `tokens.css` variable names used by every later task.

- [ ] **Step 1: Create the safety net before touching anything**

```bash
cd C:/Users/Admin/Desktop/hackathon/flightstrip
git init
git add -A
git commit -m "chore: snapshot before web rewrite"
mkdir web-legacy
git mv web/index.html web/style.css web/app.js web/serve.js web/smoke.mjs web-legacy/
```

Expected: `web-legacy/` holds the old page; `git log` shows the snapshot commit.

**`web/sessions.json` is deliberately NOT moved.** It is the hand-written session catalogue (title, file, blurb) and is the only home for that prose — the fixtures carry `meta.title` but no hook. It stays at `web/sessions.json` as a committed source file so the build survives the `web-legacy/` cleanup in Task 13. `web/public/sessions/` is generated and git-ignored.

- [ ] **Step 2: Write `web/package.json`**

Scripts: `dev` (`vite`), `build` (`tsc -b && vite build`), `preview` (`vite preview`), `prebuild` (`node scripts/sync-fixtures.mjs`), `test` (`vitest run`). Deps: `react@^19`, `react-dom@^19`, `@fontsource/space-grotesk`, `@fontsource/ibm-plex-mono`. Dev deps: `vite@^6`, `@vitejs/plugin-react`, `typescript@^5`, `vitest`, `jsdom`, `@testing-library/react`.

- [ ] **Step 3: Write `vite.config.ts`, `tsconfig.json`, `index.html`**

`vite.config.ts`: `base: './'` (hash routing, no server rewrites), `build.outDir: 'dist'`, `build.target: 'es2020'`, `test.environment: 'jsdom'`. `index.html`: `lang="en"`, the meta description from the spec, `#root`, and `<script type="module" src="/src/main.tsx">`.

- [ ] **Step 4: Write the five CSS files**

`tokens.css` defines the palette, the two font stacks, spacing scale, radii, and an `--ease-out: cubic-bezier(.16,1,.3,1)`. `reset.css` is a minimal modern reset. `typography.css` sets the three type roles. `paper.css` holds the SVG-noise grain (3% opacity), the `repeating-linear-gradient` rules and the hero vignette. `animations.css` holds exactly the spec's keyframes: `ink-draw`, `stamp-drop`, `fade-up`, `blink`, `pen-trail`, `shake`, `pulse-glow`, `ripple`, plus a `@media (prefers-reduced-motion: reduce)` block that sets `animation: none !important; transition: none !important`.

- [ ] **Step 5: Write `main.tsx` and a placeholder `App.tsx`**

`main.tsx` imports the five CSS files in order and the two `@fontsource` weights, then renders `<App/>`. `App.tsx` returns a `<h1>` in Space Grotesk to prove the fonts and tokens load.

- [ ] **Step 6: Verify the scaffold**

Run: `cd web && npm install && npm run build`
Expected: build succeeds, `web/dist/index.html` exists, no TypeScript errors.

- [ ] **Step 7: Commit**

```bash
git add web && git commit -m "feat(web): scaffold vite + react + ts with flight-recorder tokens"
```

---

### Task 2: Wire types and the session parser

**Files:**
- Create: `web/src/types.ts`, `web/src/utils/parseSession.ts`
- Test: `web/src/utils/parseSession.test.ts`

**Interfaces:**
- Produces: `Session`, `SessionMeta`, `PromptSummary`, `ExportStep`, `EventKind`, `FileChange`, `Command`, `TestRun`, `Note`, `DiffHunk`, `DiffLine`, `Incident`, `IncidentKind`, `Severity`, `FileSummary`, `SessionStats`, `FileOp`, `DiffTag`, `PromptSource`, `CommandSource`, `SessionLabel`.
- Produces: `parseSession(value: unknown): { ok: true; session: Session } | { ok: false; error: string }`.

- [ ] **Step 1: Write `types.ts` mirroring `schema.rs`**

Every interface mirrors `crates/flightstrip-core/src/schema.rs` field-for-field, including optionality: `meta.agent?: string`, `file_change.before_hash?: string`, `command.exit_code?: number`, `exportStep.hunks?: DiffHunk[]`, `incident.prompt_id?: string`, `incident.path?: string`, `stats.tests_first_ok?: boolean`. `EventKind` is a discriminated union on `kind`. `FileOp`, `DiffTag`, `IncidentKind`, `Severity`, `PromptSource`, `CommandSource`, `SessionLabel` are string-literal unions with exactly the members in `schema.rs` (note `scope_creep` and `rename` exist).

- [ ] **Step 2: Write the failing test**

```ts
import { parseSession } from "./parseSession";
import weakened from "../../../fixtures/weakened-tests.json";

it("accepts a real fixture", () => {
  const r = parseSession(weakened);
  expect(r.ok).toBe(true);
});
it("names the first bad field", () => {
  const r = parseSession({ version: 1, meta: { id: "x" } });
  expect(r.ok).toBe(false);
  if (!r.ok) expect(r.error).toMatch(/meta\.title/);
});
it("rejects a non-object", () => {
  expect(parseSession(null).ok).toBe(false);
});
```

- [ ] **Step 3: Run it and watch it fail**

Run: `cd web && npx vitest run src/utils/parseSession.test.ts`
Expected: FAIL — `parseSession` is not defined.

- [ ] **Step 4: Implement `parseSession`**

Walk the required keys in schema order (`version`, `meta` then its fields, `prompts`, `steps`, `incidents`, `files`, `stats`); on the first missing or wrong-typed one return `{ ok: false, error: "meta.title" }`-shaped text. Do not deep-validate `hunks` — a missing hunk is tolerated, matching the Rust reader's tolerance.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd web && npx vitest run src/utils/parseSession.test.ts`
Expected: PASS, 3 tests.

- [ ] **Step 6: Commit**

```bash
git add web/src/types.ts web/src/utils && git commit -m "feat(web): mirror the session schema and add a validating parser"
```

---

### Task 3: Fixture sync, the manifest, and the store

**Files:**
- Create: `web/scripts/sync-fixtures.mjs`
- Create: `web/src/store.tsx`, `web/src/utils/format.ts`, `web/src/utils/color.ts`
- Test: `web/src/utils/format.test.ts`

**Interfaces:**
- Consumes: `Session`, `parseSession` from Task 2.
- Produces: `SessionManifest = { id: string; title: string; hook: string; severity: Severity; file: string }[]`.
- Produces: `SessionProvider`, `useStore(): { manifest, session, status, error, loadSession(id), loadFromFile(file) }`.
- Produces: `formatClock(ms: number): string` (`"1:23"`), `formatDuration(ms: number): string` (`"1m 39s"`), `promptText(session, id): string`, `incidentColor(severity): string`.

- [ ] **Step 1: Write `sync-fixtures.mjs`**

Read `web/sessions.json` (the committed catalogue, which Task 1 leaves in place) and `../fixtures/*.json`. Copy each fixture into `web/public/sessions/` and write `index.json` mapping each catalogue entry to `{ id, title, hook, severity, file }`, where `id` is the fixture basename without extension, `severity` is the max incident severity in that fixture, and `title`/`hook` come from the catalogue. Fail loudly if a referenced fixture is missing. Add `web/public/sessions/` to `web/.gitignore` — it is generated, and the catalogue is the source of truth.

- [ ] **Step 2: Run it and inspect the output**

Run: `cd web && node scripts/sync-fixtures.mjs && cat public/sessions/index.json`
Expected: three entries; `risky` has `severity: "critical"`; `clean` has `severity: "info"`; the three JSON files are byte-identical to `fixtures/`.

- [ ] **Step 3: Write the failing test for the formatters**

```ts
import { formatClock, formatDuration, promptText } from "./format";
it("formats a clock", () => expect(formatClock(83000)).toBe("1:23"));
it("formats a duration", () => expect(formatDuration(99000)).toBe("1m 39s"));
it("joins a prompt id to its text", () => {
  const s = { prompts: [{ id: "p-5", text: "make the build green" }] } as any;
  expect(promptText(s, "p-5")).toBe("make the build green");
});
it("returns empty for an unknown prompt id", () => expect(promptText({ prompts: [] } as any, "p-9")).toBe(""));
```

- [ ] **Step 4: Run it and watch it fail**

Run: `cd web && npx vitest run src/utils/format.test.ts`
Expected: FAIL.

- [ ] **Step 5: Implement `format.ts` and `color.ts`**

`formatClock` floors to seconds; `formatDuration` renders `Xm Ys` under an hour and `Xh Ym` above. `promptText` does the id→text join (Review Focus 3). `color.ts` maps severity and incident kind to CSS custom-property names, never raw hex.

- [ ] **Step 6: Implement `store.tsx`**

A context with `manifest` (fetched once from `sessions/index.json`), `session`, `status: 'idle' | 'loading' | 'ready' | 'error'`, `error`. `loadSession(id)` fetches `sessions/<file>`, runs `parseSession`, and on failure sets `error` to the parser message. `loadFromFile(file)` reads via `FileReader` and follows the same path. A `useEffect` loads the `risky` session on mount.

- [ ] **Step 7: Run the tests, then commit**

Run: `cd web && npx vitest run`
Expected: PASS.

```bash
git add web/scripts web/src/store.tsx web/src/utils web/public/sessions
git commit -m "feat(web): sync fixtures and add the session store"
```

---

### Task 4: Chart geometry — the two bugs the fixtures hide

**Files:**
- Create: `web/src/utils/layout.ts`, `web/src/utils/downsample.ts`
- Test: `web/src/utils/layout.test.ts`

**Interfaces:**
- Consumes: `Session` from Task 2.
- Produces: `type Track = 'axis' | 'prompts' | 'files' | 'tests' | 'commands' | 'incidents'`.
- Produces: `TRACK_HEIGHT: Record<Track, number>` = `{ axis: 32, prompts: 48, files: 100, tests: 48, commands: 32, incidents: 48 }`.
- Produces: `domain(session): { start: number; end: number }`, `xFor(t: number, view): number`, `tFor(x: number, view): number`, `View = { t0: number; t1: number; width: number }`.
- Produces: `churnSeries(session): { t: number; churn: number }[]`, `downsample(points, maxPoints)`.

- [ ] **Step 1: Write the failing test for the time domain**

```ts
import { domain, churnSeries } from "./layout";
import weakened from "../../../fixtures/weakened-tests.json";

it("spans 0..duration, not the absolute epoch (Review Focus 1)", () => {
  const d = domain(weakened as any);
  expect(d.start).toBe(0);
  expect(d.end).toBe(99000);
});
it("never plots the session_start step", () => {
  const ts = churnSeries(weakened as any).map((p) => p.t);
  expect(Math.max(...ts)).toBeLessThan(1e6);
});
it("maps an incident step index to the right time (Review Focus 2)", () => {
  const s = weakened as any;
  const inc = s.incidents[0];            // step: 4
  expect(s.steps[inc.step].t_ms).toBe(16000);
  expect(s.steps[inc.step].kind).toBe("test_run");
});
it("survives a single-step session (Review Focus 5)", () => {
  const d = domain({ meta: { duration_ms: 0 }, steps: [{ seq: 0, t_ms: 0, kind: "session_start" }] } as any);
  expect(Number.isFinite(d.end)).toBe(true);
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/utils/layout.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement `layout.ts`**

`domain` returns `{ start: 0, end: session.meta.duration_ms }`, falling back to the max relative `t_ms` when `duration_ms` is 0. Every consumer indexes `session.steps[incident.step]` (Review Focus 2). `churnSeries` walks `steps[1..]`, accumulating `lines_added + lines_removed` for `file_change` events into a cumulative series starting at `(0, 0)`. `xFor`/`tFor` are the linear maps over `View`. Guard `end === start` to avoid divide-by-zero (Review Focus 5).

- [ ] **Step 4: Implement `downsample.ts`**

Largest-Triangle-Three-Buckets over `(t, churn)` to at most `maxPoints`, preserving first and last. Pure function, no DOM.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd web && npx vitest run src/utils/layout.test.ts`
Expected: PASS, 4 tests.

- [ ] **Step 6: Commit**

```bash
git add web/src/utils/layout.ts web/src/utils/downsample.ts web/src/utils/layout.test.ts
git commit -m "feat(web): chart geometry with correct time domain and step indexing"
```

---

### Task 5: The particle field and the hero

**Files:**
- Create: `web/src/utils/particles.ts`, `web/src/hooks/useParticleCanvas.ts`, `web/src/components/ParticleField.tsx`, `web/src/components/DataPlate.tsx`, `web/src/components/Hero.tsx`
- Test: `web/src/utils/particles.test.ts`

**Interfaces:**
- Produces: `interface Particle { x: number; y: number; vx: number; vy: number; radius: number; opacity: number }`.
- Produces: `makeParticles(n, w, h): Particle[]`, `step(p, w, h, dt): void`, `repel(p, mx, my, radius): void`, `connections(ps, maxDist): [number, number][]`.
- Produces: `ParticleField({ paused?: boolean })`, `DataPlate({ session, clockMs, steps })`, `Hero({ onWatch, onDownload })`.

- [ ] **Step 1: Write the failing test for the particle maths**

```ts
import { makeParticles, step, connections } from "./particles";
it("makes the requested count inside the box", () => {
  const ps = makeParticles(100, 800, 600);
  expect(ps).toHaveLength(100);
  expect(ps.every((p) => p.x >= 0 && p.x <= 800 && p.y >= 0 && p.y <= 600)).toBe(true);
});
it("wraps around an edge", () => {
  const p = { x: 799, y: 10, vx: 5, vy: 0, radius: 1, opacity: 1 };
  step(p, 800, 600, 16);
  expect(p.x).toBeGreaterThanOrEqual(0);
  expect(p.x).toBeLessThan(800);
});
it("connects only pairs within the distance", () => {
  const ps = [{ x: 0, y: 0, vx: 0, vy: 0, radius: 1, opacity: 1 },
              { x: 50, y: 0, vx: 0, vy: 0, radius: 1, opacity: 1 },
              { x: 500, y: 0, vx: 0, vy: 0, radius: 1, opacity: 1 }];
  expect(connections(ps, 100)).toEqual([[0, 1]]);
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/utils/particles.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement `particles.ts`**

`makeParticles` uses the spec's ranges (vx 0.1–0.3, vy −0.1 to −0.3, radius 1–2). `step` integrates and wraps. `repel` applies a gentle outward force using **squared** distance (no `sqrt`). `connections` is the O(n²) pair test at `maxDist`.

- [ ] **Step 4: Implement `useParticleCanvas.ts` and `ParticleField.tsx`**

The hook owns the canvas, DPR capped at 2, a frame-skip to 30fps (`>= 33ms`), `clearRect` each frame, an `IntersectionObserver` on the host and a `visibilitychange` listener that both pause the loop, a debounced `ResizeObserver`, and a reduced-motion branch that draws exactly one static frame and starts no loop. `ParticleField` is `position:absolute; inset:0; z-index:0` behind the hero.

- [ ] **Step 5: Implement `Hero.tsx` and `DataPlate.tsx`**

`Hero` renders the `h1` as per-letter `<span>`s with `aria-label` carrying the full sentence, a 120ms per-letter fade with a 40ms stagger, the subtitle, and the two buttons wired to `onWatch`/`onDownload`. `DataPlate` renders the bordered badge with a blinking `REC` dot and live `steps`/`clockMs` values.

- [ ] **Step 6: Verify in the browser**

Run: `cd web && npm run dev`, open `:5173`.
Expected: hero fills the viewport, letters stagger in, particles drift and connect, the badge blinks, and DevTools shows the loop idle when the hero is scrolled away.

- [ ] **Step 7: Commit**

```bash
git add web/src/utils/particles.ts web/src/hooks/useParticleCanvas.ts web/src/components
git commit -m "feat(web): hero, data plate and the particle field"
```

---

### Task 6: The strip chart — tracks, canvas trace, pen head

**Files:**
- Create: `web/src/components/StripChart/StripChart.tsx`, `CanvasTrace.tsx`, `TimeAxis.tsx`, `PromptTrack.tsx`, `TestTrack.tsx`, `CommandTrack.tsx`, `IncidentTrack.tsx`, `PenHead.tsx`
- Test: `web/src/components/StripChart/CanvasTrace.test.ts`

**Interfaces:**
- Consumes: `TRACK_HEIGHT`, `domain`, `churnSeries`, `xFor`, `downsample` (Task 4); `View` (Task 4).
- Produces: `StripChart({ session, view, clockMs, onSelectStep, onViewChange })`.
- Produces: `tracePoints(session, view, maxPoints): { x: number; y: number }[]` — the pure function `CanvasTrace` draws.

- [ ] **Step 1: Write the failing test for the trace points**

```ts
import { tracePoints } from "./CanvasTrace";
import weakened from "../../../../fixtures/weakened-tests.json";
it("returns one point per downsampled sample, inside the box", () => {
  const pts = tracePoints(weakened as any, { t0: 0, t1: 99000, width: 1200 }, 600);
  expect(pts.length).toBeGreaterThan(1);
  expect(pts.every((p) => p.x >= 0 && p.x <= 1200)).toBe(true);
});
it("is flat and finite for a session with no file changes (Review Focus 5)", () => {
  const pts = tracePoints({ meta: { duration_ms: 1000 }, steps: [] } as any, { t0: 0, t1: 1000, width: 100 }, 100);
  expect(pts.every((p) => Number.isFinite(p.y))).toBe(true);
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/components/StripChart/CanvasTrace.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement `CanvasTrace.tsx`**

`tracePoints` maps `churnSeries` through `xFor` and scales churn to the 100px track. The component draws with `--ink` at 2px, adds ±1px jitter to each Y for the ink feel, and reveals the line up to `clockMs` using `ctx.setLineDash` with an animated offset. Redraw only when `view` or the revealed length changes. Cap points at `2 × viewport width` via `downsample`.

- [ ] **Step 4: Implement the five overlay tracks and `TimeAxis.tsx`**

`PromptTrack`: orange vertical lines with rotated labels from `session.prompts`. `TestTrack`: a green/red step function from `test_run` events, green-muted background. `CommandTrack`: ticks from `command` events. `IncidentTrack`: red triangles with labels, positioned at `steps[incident.step].t_ms`. `TimeAxis`: ticks and timestamps across `0..duration`. All read `TRACK_HEIGHT` and `xFor`; all render only marks within the viewport ± one screen.

- [ ] **Step 5: Implement `PenHead.tsx` and assemble `StripChart.tsx`**

`PenHead` is an SVG triangle at `xFor(clockMs)` with a fading orange trail. `StripChart` stacks the six tracks, hosts the canvas and the SVG overlay, and handles wheel zoom toward the cursor and drag-pan, calling `onViewChange`.

- [ ] **Step 6: Verify in the browser**

Run: `cd web && npm run dev`.
Expected: the six tracks render for the `risky` session, the trace draws in, wheel zooms toward the cursor, drag pans, and clicking a mark calls `onSelectStep`.

- [ ] **Step 7: Commit**

```bash
git add web/src/components/StripChart
git commit -m "feat(web): strip chart with canvas trace, tracks and pen head"
```

---

### Task 7: Playback, the scrubber, and the keyboard

**Files:**
- Create: `web/src/hooks/usePlayback.ts`, `web/src/hooks/useKeyboard.ts`, `web/src/hooks/useHashRoute.ts`, `web/src/components/StripChart/Scrubber.tsx`
- Test: `web/src/hooks/usePlayback.test.ts`

**Interfaces:**
- Consumes: `domain` (Task 4), the store (Task 3).
- Produces: `usePlayback({ durationMs }): { clockMs, playing, speed, play(), pause(), toggle(), seek(ms), step(delta), setSpeed(n) }`.
- Produces: `useKeyboard(map: Record<string, () => void>)`, `useHashRoute(): [Route, (r: Route) => void]` where `Route = { sessionId: string; focus?: string }` and `focus` is an incident id or a step index.

- [ ] **Step 1: Write the failing test for the clock**

```ts
import { advance } from "./usePlayback";
it("advances by dt times speed", () => expect(advance(1000, 500, 2)).toBe(2000));
it("clamps at the duration", () => expect(advance(99000, 500, 4)).toBe(99000));
it("never goes below zero", () => expect(advance(0, -500, 1)).toBe(0));
```

- [ ] **Step 1b: Write the failing test for route parsing**

```ts
import { parseRoute } from "./useHashRoute";
it("parses a session route", () => expect(parseRoute("#/session/risky")).toEqual({ sessionId: "risky" }));
it("parses a focus route", () => expect(parseRoute("#/session/risky/i-0")).toEqual({ sessionId: "risky", focus: "i-0" }));
it("ignores an unknown focus rather than failing (Review Focus 6)", () => {
  expect(parseRoute("#/session/risky/i-99")).toEqual({ sessionId: "risky", focus: "i-99" });
});
it("treats an empty hash as no session", () => expect(parseRoute("")).toEqual({ sessionId: "" }));
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/hooks/usePlayback.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement `usePlayback.ts`**

`advance` is the pure clamp. The hook owns the single rAF loop, accumulating real `dt` and advancing the clock at `speed` (1/2/4), pausing at the end. `step(delta)` seeks to the previous/next step's `t_ms`. Nothing else in the app writes the clock.

- [ ] **Step 4: Implement `Scrubber.tsx`**

A custom bar: `--ink-faint` track, `--orange` fill, a circular thumb, and red dots at incident times. Draggable with pointer events, keyboard-focusable with `role="slider"` and `aria-valuenow`/`aria-valuemax`.

- [ ] **Step 5: Implement `useKeyboard.ts` and `useHashRoute.ts`**

`useKeyboard` binds the spec's map: `Space` toggle, `←/→` step, `J/K` prev/next incident, `+/-` zoom, `0` reset, `Esc` close, `?` help — ignoring keystrokes while focus is in an input. `useHashRoute` reads/writes `location.hash` and listens to `hashchange`. The route is `#/session/<id>` for the session, optionally `#/session/<id>/<focus>` where `<focus>` is an incident id (`i-0`) or a step index (`12`). On load with a focus, the app selects that step and opens the drawer, so a link lands on the exact finding. `parseRoute` is a pure function; when the focus does not resolve to a step in the loaded session the app ignores it and opens no drawer (Review Focus 6). The store owns the selection, the hook only reflects it.

- [ ] **Step 6: Wire autoplay and verify**

Run: `cd web && npm run dev`.
Expected: the chart autoplays when scrolled into view; `Space` pauses; arrows step; `J/K` jump incidents; the scrubber tracks the clock and drags.

- [ ] **Step 7: Commit**

```bash
git add web/src/hooks web/src/components/StripChart/Scrubber.tsx
git commit -m "feat(web): playback clock, scrubber and keyboard control"
```

---

### Task 8: The CVR panel and the diff view

**Files:**
- Create: `web/src/components/CVRPanel.tsx`, `web/src/components/DiffView.tsx`
- Test: `web/src/components/DiffView.test.tsx`

**Interfaces:**
- Consumes: `Session`, `ExportStep`, `DiffHunk` (Task 2); `promptText`, `formatClock` (Task 3).
- Produces: `CVRPanel({ session, stepIndex, onClose, onJump })`, `DiffView({ hunks, maxLines? })`.

- [ ] **Step 1: Write the failing test for the diff renderer**

```tsx
import { render, screen } from "@testing-library/react";
import { DiffView } from "./DiffView";
import weakened from "../../../fixtures/weakened-tests.json";

it("marks added and removed lines", () => {
  const step = (weakened as any).steps[3];       // the src/upload.ts change
  render(<DiffView hunks={step.hunks} />);
  expect(screen.getByText(/return retry/)).toBeTruthy();
});
it("renders nothing, not a crash, without hunks (Review Focus 5)", () => {
  const { container } = render(<DiffView hunks={undefined} />);
  expect(container.textContent).toContain("No diff");
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/components/DiffView.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Implement `DiffView.tsx`**

A table: line numbers in `--ink-faint`, `+` rows on a green wash, `-` rows on a red wash, `context` plain. Cap at `maxLines` (default 60) with a "Show more" that reveals the rest. No Monaco, no syntax highlighting.

- [ ] **Step 4: Implement `CVRPanel.tsx`**

A right drawer that slides in with `transform: translateX` over 300ms on `--ease-out`. Shows the event kind, the timestamp, the blame line (`Caused by prompt #N` via `promptText`), the `DiffView` when hunks exist, and a transcript of surrounding steps. Traps focus while open and restores focus to the opener on close.

- [ ] **Step 5: Verify in the browser**

Run: `cd web && npm run dev`.
Expected: clicking a step or incident opens the drawer with the right diff and blame; `Esc` and outside-click close it; focus returns to the trigger.

- [ ] **Step 6: Commit**

```bash
git add web/src/components/CVRPanel.tsx web/src/components/DiffView.tsx
git commit -m "feat(web): CVR drawer with blame and diff"
```

---

### Task 9: The incident report

**Files:**
- Create: `web/src/components/IncidentReport.tsx`, `web/src/styles/print.css`
- Test: `web/src/components/IncidentReport.test.tsx`

**Interfaces:**
- Consumes: `Session`, `Incident`, `IncidentKind` (Task 2).
- Produces: `IncidentReport({ session })`, `verdictOf(session): { level: Severity; headline: string }`.

- [ ] **Step 1: Write the failing test for the verdict**

```ts
import { verdictOf } from "./IncidentReport";
import weakened from "../../../fixtures/weakened-tests.json";
import clean from "../../../fixtures/clean.json";
it("calls a critical session critical", () => {
  expect(verdictOf(weakened as any).level).toBe("critical");
});
it("calls a clean session clean", () => {
  expect(verdictOf(clean as any).level).toBe("info");
});
it("handles zero incidents without throwing (Review Focus 5)", () => {
  expect(verdictOf({ incidents: [], stats: {} } as any).level).toBe("info");
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/components/IncidentReport.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Implement `verdictOf` and `IncidentReport.tsx`**

`verdictOf` takes the max incident severity and produces a headline. The component renders a printed-document card: a rotated red stamp, a faint 45° `CLASSIFIED` watermark, and four blocks — verdict, timeline (incidents by step), probable cause (derived from the dominant incident kind) and recommendations (one line per kind present). A print button calls `window.print()`.

- [ ] **Step 4: Write `print.css`**

`@media print`: hide the chart, scrubber, drawer and footer; force the report to a white background with black text; expand all "Show more"; drop the watermark to 5% so it does not obscure text.

- [ ] **Step 5: Verify in the browser and in print preview**

Run: `cd web && npm run dev`, then `Ctrl+P`.
Expected: the report reads as a printed document; print preview shows only the report, legibly.

- [ ] **Step 6: Commit**

```bash
git add web/src/components/IncidentReport.tsx web/src/styles/print.css
git commit -m "feat(web): incident report with print styles"
```

---

### Task 10: The trace graph

**Files:**
- Create: `web/src/components/TraceGraph.tsx`
- Test: `web/src/components/TraceGraph.test.ts`

**Interfaces:**
- Consumes: `Session`, `FileSummary`, `Incident` (Task 2); `TRACK_HEIGHT` is not used here.
- Produces: `graphLayout(session): { prompts: Node[]; files: Node[]; incidents: Node[]; edges: Edge[] }` where `Node = { id: string; kind: 'prompt'|'file'|'incident'; x: number; y: number; r?: number; label: string; step: number }` and `Edge = { from: string; to: string }`.

- [ ] **Step 1: Write the failing test for the layout**

```ts
import { graphLayout } from "./TraceGraph";
import weakened from "../../../fixtures/weakened-tests.json";
it("is deterministic", () => {
  expect(graphLayout(weakened as any)).toEqual(graphLayout(weakened as any));
});
it("sizes a file node by churn", () => {
  const g = graphLayout(weakened as any);
  expect(g.files.every((f) => f.r! >= 20 && f.r! <= 50)).toBe(true);
});
it("gives every node a finite position (Review Focus 5)", () => {
  const g = graphLayout({ prompts: [], files: {}, incidents: [], steps: [] } as any);
  expect(g.prompts.length + g.files.length + g.incidents.length).toBe(0);
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/components/TraceGraph.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement `graphLayout`**

Prompts in a left column, files in a right column, incidents in a middle column. Deterministic vertical distribution (index-based, not random). File radius maps churn `1..max` to `20..50`. Edges: prompt→file for each prompt id in `FileSummary.prompts`, and incident→file where `incident.path` matches. No force simulation.

- [ ] **Step 4: Implement `TraceGraph.tsx`**

An SVG ~500px tall: prompt nodes as dashed orange rounded rects (180×50), file nodes as circles, incident nodes as red triangles. Edges are curved paths in `--ink-faint` that animate `stroke-dashoffset` on hover to show direction. Hover highlights connected nodes; clicking a node calls `onJumpToStep(node.step)`.

- [ ] **Step 5: Verify in the browser**

Run: `cd web && npm run dev`.
Expected: the graph renders deterministically, hover highlights neighbours, and a click scrolls the chart to that step.

- [ ] **Step 6: Commit**

```bash
git add web/src/components/TraceGraph.tsx
git commit -m "feat(web): deterministic trace graph"
```

---

### Task 11: How it works, download, and the drop zone

**Files:**
- Create: `web/src/components/HowItWorks.tsx`, `DownloadButton.tsx`, `DropZone.tsx`, `Footer.tsx`, `SessionPicker.tsx`, `ShortcutHelp.tsx`
- Create: `web/src/hooks/useScrollReveal.ts`
- Test: `web/src/components/DropZone.test.tsx`

**Interfaces:**
- Consumes: `useStore` (Task 3), `parseSession` (Task 2).
- Produces: `useScrollReveal(ref): boolean` (threshold 0.15, `rootMargin: '0px 0px -50px 0px'`, latches true).
- Produces: `HowItWorks()`, `DownloadButton({ os? })`, `DropZone({ onLoad })`, `Footer()`, `SessionPicker({ manifest, onPick })`, `ShortcutHelp({ open, onClose })`.

- [ ] **Step 1: Write the failing test for the drop zone**

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { DropZone } from "./DropZone";
it("reports an invalid file instead of crashing (Review Focus 4)", async () => {
  const onError = vi.fn();
  render(<DropZone onLoad={() => {}} onError={onError} />);
  const zone = screen.getByRole("application", { name: /drop/i });
  fireEvent.drop(zone, { dataTransfer: { files: [new File(["not json"], "x.json")] } });
  await screen.findByText(/invalid session file/i);
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npx vitest run src/components/DropZone.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Implement `DropZone.tsx`**

A dashed 200px target. On `dragover` the border turns `--orange` and the text becomes "Release to load". On drop, read the file, run `parseSession`, and on success call `onLoad(session)` plus a toast; on failure add the `shake` class and show "Invalid session file" in `--red` with the parser's field message. A hidden `<input type="file">` triggered by click covers mobile.

- [ ] **Step 4: Implement `useScrollReveal.ts`, `HowItWorks.tsx`, `DownloadButton.tsx`**

`useScrollReveal` uses the spec's observer options and latches visible. `HowItWorks` renders three cards (01/02/03) with a 100ms stagger. `DownloadButton` detects the OS from `navigator.userAgent`, names the correct binary, follows the cursor with a radial-gradient ripple, scales to 0.97 on click and swaps to a checkmark for 2s. Per the design it links to the build-from-source instructions, not a fake file, and shows `cargo build --release` with a copy-to-clipboard button.

- [ ] **Step 5: Implement `SessionPicker.tsx`, `ShortcutHelp.tsx`, `Footer.tsx`**

`SessionPicker` lists the manifest with title, hook and a severity dot. `ShortcutHelp` is a modal listing the key map, opened by `?`. `Footer` is a rule plus "Built with Freebuff" and a hackathon badge. **No GitHub link** — there is no remote, and a dead link is a visible defect on a page judged on polish. Leave a single commented line in `Footer.tsx` marking where it goes once a remote exists.

- [ ] **Step 6: Verify in the browser**

Run: `cd web && npm run dev`.
Expected: cards stagger in on scroll; the download button ripples and confirms; dropping `fixtures/clean.json` loads it and scrolls to the chart; dropping a text file shakes and shows the red message.

- [ ] **Step 7: Commit**

```bash
git add web/src/components web/src/hooks/useScrollReveal.ts
git commit -m "feat(web): briefing, download, drop zone and pickers"
```

---

### Task 12: Assemble the page, accessibility, responsive, and the headless check

**Files:**
- Modify: `web/src/App.tsx`
- Create: `web/src/hooks/useSession.ts`
- Create: `web/public/_headers`
- Create: `web/smoke.mjs`
- Modify: `README.md`, `NOTES.md`

**Interfaces:**
- Consumes: every component above.
- Produces: the assembled single page; `smoke.mjs` exits non-zero on failure.

- [ ] **Step 1: Assemble `App.tsx`**

Compose Hero → StripChart + Scrubber + CVRPanel → IncidentReport → TraceGraph → HowItWorks → Download → DropZone → Footer, with `SessionProvider` at the root, `useHashRoute` for `#/session/<id>`, the `useKeyboard` map bound once, and an `aria-live="polite"` region announcing the current step. Add a skeleton state (pulsing `--paper-dark` rectangles) while `status === 'loading'`.

- [ ] **Step 2: Add the responsive rules**

Desktop ≥1024px: drawer right, graph visible. Tablet 768–1023px: drawer as a bottom sheet, simplified graph. Mobile <768px: single column, horizontally scrollable chart, drawer as a full overlay, graph replaced with "view on desktop", full-width download button. Implement with media queries in `components.css`.

- [ ] **Step 3: Add `_headers`**

Cache `/*.json` for a short TTL with `access-control-allow-origin: *`, and hashed assets immutably.

- [ ] **Step 4: Write the headless check `web/smoke.mjs`**

Boot `vite preview` on `dist`, load `#/session/risky` in jsdom, and assert: the six track groups exist, at least one prompt marker and one incident marker rendered, the step list is non-empty, and opening a step produces a diff row. Exit 1 with a message on any failure. This replaces the old `web-legacy/smoke.mjs`, which cannot survive the rewrite.

- [ ] **Step 5: Run the full verification**

Run: `cd web && npm run build && node smoke.mjs && npx vitest run`
Expected: build clean, smoke passes, all unit tests pass.

- [ ] **Step 6: Update `README.md` and `NOTES.md`**

Correct the two places that now state something untrue: the "no npm dependencies, no build step" claims become "the replay page is a Vite + React + TS app; the recorder remains dependency-free", and the Website section of NOTES.md describes the new stack and the fixture-sync step. State the reversal plainly rather than quietly deleting the old text.

- [ ] **Step 7: Commit**

```bash
git add web README.md NOTES.md
git commit -m "feat(web): assemble the page, add responsive + a11y, replace the smoke check"
```

---

### Task 13: Verify locally and clean up the safety net

**Files:**
- Delete: `web-legacy/` (after local verification passes)

- [ ] **Step 1: Build and verify locally**

Run: `cd web && npm run build && node smoke.mjs && npx vitest run`
Expected: build clean, smoke passes, all unit tests pass. The deliverable is the local build at `web/dist`.

- [ ] **Step 2: Serve the built output and check it end to end**

Run: `cd web && npx vite preview`
Check on desktop and a narrow viewport: hero, chart autoplay, drawer, report, drop zone, and a deep link (`#/session/risky/i-0`) landing with the drawer open.

- [ ] **Step 3: Remove the safety net**

Run: `git rm -r web-legacy && git commit -m "chore: drop the pre-rewrite page now that the new one is verified"`
Expected: one replay page in the repository. The build still succeeds, because `web/sessions.json` stayed in place (Task 1).

- [ ] **Step 4: Document the deploy command**

Add the `npx wrangler pages deploy dist --project-name flightstrip` line to `README.md` under a "Deploy" heading, marked as the step a maintainer runs with their own Cloudflare credentials. No credentials are used in this work.

---

## Self-Review

**Spec coverage.** Hero → Task 5. Strip chart (six tracks, canvas trace, pen head, zoom, pan, scrubber) → Tasks 4, 6, 7. CVR + diff → Task 8. Incident report + print → Task 9. Trace graph → Task 10. How it works → Task 11. Download → Task 11. Drop zone → Task 11. Footer → Task 11. Animations → Tasks 1, 5, 11. Particles → Task 5. Scroll reveals → Task 11. Responsive → Task 12. Accessibility → Tasks 7, 8, 12. Performance → Tasks 4, 5, 6. Sample session → Task 3 (real fixtures replace the hand-written one). Deploy → Task 13.

**Review Focus coverage.** 1 → Task 4 Step 1. 2 → Task 4 Step 1. 3 → Task 3 Step 3. 4 → Task 11 Step 1. 5 → Tasks 4, 6, 8, 9, 10. 6 → Task 7 Step 1b.

**Type consistency.** `View`, `TRACK_HEIGHT`, `domain`, `xFor` are defined in Task 4 and consumed unchanged in Tasks 6 and 7. `parseSession`'s result shape is defined in Task 2 and consumed in Tasks 3 and 11. `Node`/`Edge` are defined and consumed inside Task 10 only.

**Deliberate deviations from the brief, carried from the spec.** React/Vite (reversal, documented in Task 12); real fixtures instead of a hand-written demo (Task 3); build-from-source instead of placeholder binaries (Task 11); no three.js (never introduced).
