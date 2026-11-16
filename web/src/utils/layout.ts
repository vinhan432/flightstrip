// Chart geometry. Pure functions, no DOM.
//
// Two facts about the wire format drive this file, and both are easy to get
// wrong because the fixtures hide them:
//
//  1. `steps[0]` (session_start) carries an ABSOLUTE epoch `t_ms`, while
//     every later step is relative to the session start. The time domain is
//     therefore `0..meta.duration_ms`, and step 0 is never plotted.
//  2. `incident.step` indexes `steps`, not `seq`. The fixtures happen to use
//     the same numbers for both, so a `seq`-based lookup passes the demo and
//     breaks on a real export.

import type { Session, ExportStep } from "../types";
import { asFileChange } from "../types";

export type Track =
  | "axis"
  | "prompts"
  | "files"
  | "tests"
  | "commands"
  | "incidents";

export const TRACK_HEIGHT: Record<Track, number> = {
  axis: 32,
  prompts: 48,
  files: 100,
  tests: 48,
  commands: 32,
  incidents: 48,
};

/** The tracks, top to bottom. The chart stacks them in this order. */
export const TRACK_ORDER: Track[] = [
  "axis",
  "prompts",
  "files",
  "tests",
  "commands",
  "incidents",
];

/**
 * Each track's vertical offset in the stacked chart, in pixels.
 *
 * The canvas trace and the SVG overlay must agree on this: the canvas is
 * absolutely positioned, so it needs `offsets.files` explicitly or it draws
 * across the tracks above.
 */
export function trackOffsets(): Record<Track, number> {
  const offsets = {} as Record<Track, number>;
  let y = 0;
  for (const track of TRACK_ORDER) {
    offsets[track] = y;
    y += TRACK_HEIGHT[track];
  }
  return offsets;
}

/** Total height of the stacked chart. */
export function chartHeight(): number {
  return TRACK_ORDER.reduce((sum, track) => sum + TRACK_HEIGHT[track], 0);
}

/** The visible time window, in session-relative milliseconds. */
export interface View {
  t0: number;
  t1: number;
  width: number;
}

export interface Domain {
  start: number;
  end: number;
}

/**
 * The plotted time range. Always starts at 0: the recording is relative.
 * `duration_ms` is authoritative; when it is missing or zero (a truncated
 * export) fall back to the last relative step so the axis is still sane.
 */
export function domain(session: Session): Domain {
  const declared = session.meta?.duration_ms ?? 0;
  if (declared > 0) return { start: 0, end: declared };

  let last = 0;
  for (const step of session.steps ?? []) {
    // Step 0 is the absolute anchor, not a relative time.
    if (step.seq === 0 || step.kind === "session_start") continue;
    if (Number.isFinite(step.t_ms) && step.t_ms > last) last = step.t_ms;
  }
  return { start: 0, end: last };
}

export function xFor(t: number, view: View): number {
  const span = view.t1 - view.t0;
  if (span <= 0) return 0;
  return ((t - view.t0) / span) * view.width;
}

export function tFor(x: number, view: View): number {
  if (view.width <= 0) return view.t0;
  return view.t0 + (x / view.width) * (view.t1 - view.t0);
}

export interface ChurnPoint {
  t: number;
  churn: number;
}

/**
 * Cumulative file churn over time: the spine of the strip chart. Walks the
 * relative steps in order, adding `lines_added + lines_removed` for each
 * file change. Always begins at `(0, 0)` so the trace has an origin.
 */
export function churnSeries(session: Session): ChurnPoint[] {
  const points: ChurnPoint[] = [{ t: 0, churn: 0 }];
  let churn = 0;

  for (const step of session.steps ?? []) {
    if (step.seq === 0 || step.kind === "session_start") continue;
    const change = asFileChange(step);
    if (!change) continue;
    churn += (change.lines_added ?? 0) + (change.lines_removed ?? 0);
    points.push({ t: step.t_ms, churn });
  }

  return points;
}

/** The step an incident points at. Indexes `steps`, never `seq`. */
export function stepForIncident(
  session: Session,
  incidentStep: number,
): ExportStep | undefined {
  return session.steps[incidentStep];
}

/** Marks within the visible window, plus one screen either side. */
export function visible<T extends { t: number }>(
  items: T[],
  view: View,
): T[] {
  const pad = view.t1 - view.t0;
  const lo = view.t0 - pad;
  const hi = view.t1 + pad;
  return items.filter((item) => item.t >= lo && item.t <= hi);
}
