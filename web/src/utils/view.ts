// View arithmetic for the chart's zoom and reset.
//
// The wheel handler clamps to the session domain; the keyboard path must
// clamp identically, or `+`/`-` can push the view to negative times and
// leave the chart blank. Both go through here.

export interface ViewRect {
  t0: number;
  t1: number;
  width: number;
}

export interface TimeDomain {
  start: number;
  end: number;
}

const MIN_SPAN = 500;

/** Zoom around the view's centre, clamped to the session. */
export function zoomView(
  view: ViewRect,
  factor: number,
  domain: TimeDomain,
): ViewRect {
  const span = Math.max(MIN_SPAN, (view.t1 - view.t0) * factor);
  const centre = (view.t0 + view.t1) / 2;

  let t0 = centre - span / 2;
  let t1 = centre + span / 2;

  if (t0 < domain.start) {
    t0 = domain.start;
    t1 = domain.start + span;
  }
  if (t1 > domain.end) {
    t1 = domain.end;
    t0 = Math.max(domain.start, domain.end - span);
  }

  return { ...view, t0, t1 };
}

/** The whole session, whatever the current zoom. */
export function resetView(domain: TimeDomain, width: number): ViewRect {
  const end = Math.max(1, domain.end);
  return { t0: 0, t1: end, width };
}
