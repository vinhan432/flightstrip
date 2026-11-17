# Flightstrip web — design decisions and known issues

## Design decisions

**Fonts.** Space Grotesk for headings and stamps, IBM Plex Mono for data and
body. Space Grotesk has the slightly mechanical, engineered feel of instrument
labelling; Plex Mono has tabular figures, which the clock and the axis need so
digits do not jitter as the numbers change. Both self-hosted via `@fontsource`,
`font-display: swap`, no runtime font fetch.

**One signal colour.** The palette is warm paper, near-black ink, and exactly
one accent (`--orange #d4621a`). Red is reserved for incidents and green for
passing tests, so when either appears it means something. This is why the page
does not look like a dashboard: nothing is coloured for decoration.

**The chart is the product.** The hero is short and the strip chart sits
immediately below it, already playing, so the first impression is the
instrument working rather than a marketing block.

**Ink, not vectors.** The canvas trace adds ±1px of per-point jitter so the
line reads as drawn. The pen head carries a two-second trail so the eye can
follow where the recording has just been.

**No side-tab callouts.** The impeccable detector flags a 3px left accent
border as the most recognisable AI-UI tell. The CVR callouts use a full
hairline border with corner registration ticks instead.

## Route contract

- `#/s/<id>` — a session, autoplaying.
- `#/s/<id>?step=N` — a session with step N selected (the drawer open).
- `#/report/<id>` — the investigation.
- No hash — falls back to `risky-session`.

The selected step lives in the URL, not in component state, which is what makes
back/forward work with no extra bookkeeping.

## Performance

- Canvas trace is downsampled with largest-triangle-three-buckets to at most 2×
  the viewport width.
- SVG marks are culled to the viewport ± one screen.
- Particles run at a 30fps cap, pause off-screen and when the tab is hidden.
- `npm run synthetic` writes a 3,000-step session for a stress check. It is
  gitignored; the site labels it `synthetic`.
- `npm run bench` measures the hot paths. At 3,002 steps / 2,641 churn points:
  downsample to 2,400 points takes **0.60 ms**; the full view holds 120
  prompts + 120 commands + 120 tests, and a 50× zoom culls that to 7 each.

## The CVR drawer

One drawer, three shapes, by breakpoint: a sticky right drawer on desktop, a
bottom sheet at ≤1023px, a full-screen overlay at ≤767px. It is a
`role="complementary"` region with a real focus trap — Tab cycles inside it,
Escape closes, and focus returns to whatever opened it.

Contents by event kind: the incident (evidence + why it matters + a severity
tag that is text, not just colour), prompt blame ("Caused by prompt #3" with
the quoted text and a jump button), a facts list (operation, line counts,
before/after hashes, exit code, duration, pass/fail/skip counts), the diff,
and a ±5-step transcript where every row is a button.

The blame callout is deliberately not a side-tab accent border — that is the
most recognisable AI-UI tell. It uses a full hairline border with corner
registration ticks, red for incidents and orange for prompt blame.

## Incident stamp
Every incident marker is present from the start — the recording is a finished
artifact, so the incident track must never read as empty. Each triangle stamps
in (a 420ms `stamp-drop`) as the pen reaches it during playback. The stamp uses
`transform-box: fill-box`; without it an SVG `scale()` happens about the SVG
origin and the triangle flies across the chart.

## Known issues / not yet done

- **No lint script.** The spec asks for `npm run lint`; there is no ESLint
  config. `tsc -b` runs as part of `npm run build` and is the current type
  gate. Adding ESLint is a small follow-up.
- **No `FIXTURE`/`LOCAL` badge yet.** `meta.label` is parsed and typed but not
  rendered. Honest labelling is a spec requirement and is the next thing to do.
- **No `src/config.ts`.** `DownloadButton` copies `cargo build --release` and
  links to nothing, because no release exists. When a release does, the asset
  URLs go in `config.ts`.
- **No error boundary.** A render throw shows a blank page, not the "recorder
  lost signal" screen.
- **No report route UI.** `#/report/<id>` parses, but the report is an in-page
  section, not a separate view.
- **`parseSession` returns one error string**, not a list. The Drop Zone shows
  the first bad field path; the spec wants the first three in plain language.
- **Canvas in jsdom** logs `getContext not implemented`; harmless test noise.
