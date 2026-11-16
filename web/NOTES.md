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
- `node scripts/make-synthetic.mjs 3000` writes a 3,000-step session for a
  stress check. It is gitignored; the site labels it `synthetic`.

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
- **`label` type is `"real" | "fixture" | "synthetic"`**; the spec text says
  `"recorded" | "fixture"`. The engine's actual output wins.
- **Canvas in jsdom** logs `getContext not implemented`; harmless test noise.
