// The strip chart: six stacked tracks, a canvas trace under an SVG overlay,
// a pen head, wheel zoom toward the cursor and drag-pan.
//
// The tracks are laid out on one SVG so the pen head and its trail can span
// the full height in a single element.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Session } from "../../types";
import {
  TRACK_HEIGHT,
  TRACK_ORDER,
  trackOffsets,
  chartHeight,
  domain,
  tFor,
  type View,
} from "../../utils/layout";
import { CanvasTrace } from "./CanvasTrace";
import { TimeAxis } from "./TimeAxis";
import { PromptTrack } from "./PromptTrack";
import { TestTrack } from "./TestTrack";
import { CommandTrack } from "./CommandTrack";
import { IncidentTrack } from "./IncidentTrack";
import { PenHead } from "./PenHead";

interface Props {
  session: Session;
  view: View;
  clockMs: number;
  onViewChange: (view: View) => void;
  onSelectStep: (index: number) => void;
  onSelectIncident: (incidentId: string) => void;
}

export function StripChart({
  session,
  view,
  clockMs,
  onViewChange,
  onSelectStep,
  onSelectIncident,
}: Props) {
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const [panning, setPanning] = useState(false);
  const panState = useRef({ x: 0, t0: 0, t1: 0 });
  // Live pointers, for one-finger pan and two-finger pinch.
  const pointers = useRef(new Map<number, number>());
  const pinchState = useRef({ distance: 0, t0: 0, t1: 0, centre: 0 });

  const totalHeight = chartHeight();

  // Each track's vertical offset in the stacked chart. Shared with the
  // canvas trace so the two cannot disagree about where a track begins.
  const offsets = useMemo(trackOffsets, []);

  const { start, end } = domain(session);

  /** Clamp a window to the session, preserving its span. */
  const clamp = useCallback(
    (t0: number, t1: number) => {
      const span = t1 - t0;
      if (t0 < start) return { t0: start, t1: start + span };
      if (t1 > end) return { t0: Math.max(start, end - span), t1: end };
      return { t0, t1 };
    },
    [start, end],
  );

  // A native, non-passive wheel listener. React attaches `wheel` passively,
  // so `event.preventDefault()` inside an onWheel prop is ignored and the
  // page scrolls while the chart zooms.
  useEffect(() => {
    const el = surfaceRef.current;
    if (!el) return;

    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = el.getBoundingClientRect();
      if (!rect) return;

      const cursorT = tFor(event.clientX - rect.left, view);
      const factor = event.deltaY > 0 ? 1.15 : 1 / 1.15;

      const span = view.t1 - view.t0;
      const nextSpan = Math.min(end - start, Math.max(500, span * factor));

      // Keep the time under the cursor fixed while the span changes.
      const ratio = (cursorT - view.t0) / span;
      const t0 = cursorT - ratio * nextSpan;
      const clamped = clamp(t0, t0 + nextSpan);

      onViewChange({ ...view, ...clamped });
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [view, start, end, onViewChange, clamp]);

  const onPointerDown = useCallback(
    (event: React.PointerEvent) => {
      if (event.button !== 0 && event.pointerType === "mouse") return;
      pointers.current.set(event.pointerId, event.clientX);
      (event.target as Element).setPointerCapture?.(event.pointerId);

      if (pointers.current.size === 2) {
        // Two fingers down: switch from pan to pinch.
        const [a, b] = [...pointers.current.values()];
        pinchState.current = {
          distance: Math.abs(a - b),
          t0: view.t0,
          t1: view.t1,
          centre: (a + b) / 2,
        };
        setPanning(false);
        return;
      }

      setPanning(true);
      panState.current = { x: event.clientX, t0: view.t0, t1: view.t1 };
    },
    [view.t0, view.t1],
  );

  const onPointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (!pointers.current.has(event.pointerId)) return;
      pointers.current.set(event.pointerId, event.clientX);

      const rect = surfaceRef.current?.getBoundingClientRect();
      if (!rect) return;

      // Pinch: the span scales by the change in finger distance.
      if (pointers.current.size === 2) {
        const [a, b] = [...pointers.current.values()];
        const distance = Math.abs(a - b);
        const pinch = pinchState.current;
        if (pinch.distance <= 0) return;

        const span = pinch.t1 - pinch.t0;
        const nextSpan = Math.min(
          end - start,
          Math.max(500, span * (pinch.distance / distance)),
        );

        // Hold the time under the pinch centre.
        const centreT = tFor(pinch.centre - rect.left, {
          ...view,
          t0: pinch.t0,
          t1: pinch.t1,
        });
        const ratio = (centreT - pinch.t0) / span;
        const t0 = centreT - ratio * nextSpan;
        onViewChange({ ...view, ...clamp(t0, t0 + nextSpan) });
        return;
      }

      if (!panning) return;

      const dx = event.clientX - panState.current.x;
      const span = panState.current.t1 - panState.current.t0;
      const dt = -(dx / rect.width) * span;

      const t0 = panState.current.t0 + dt;
      onViewChange({ ...view, ...clamp(t0, t0 + span) });
    },
    [panning, view, start, end, onViewChange, clamp],
  );

  const endPan = useCallback((event: React.PointerEvent) => {
    pointers.current.delete(event.pointerId);
    if (pointers.current.size < 2) pinchState.current.distance = 0;
    if (pointers.current.size === 0) setPanning(false);
  }, []);

  const trackProps = { session, view, onSelectStep };

  return (
    <div
      ref={surfaceRef}
      className={`chart-well${panning ? " is-panning" : ""}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endPan}
      onPointerCancel={endPan}
      role="application"
      aria-label="Session strip chart. Scroll or pinch to zoom, drag to pan."
    >
      <div className="chart-surface" style={{ height: totalHeight }}>
        {/* The canvas sits at the FILES track's offset, not at the top of the
            surface: otherwise the trace draws across the time and prompt
            tracks and the files track stays empty. */}
        <CanvasTrace
          session={session}
          view={view}
          clockMs={clockMs}
          height={TRACK_HEIGHT.files}
          top={offsets.files}
        />

        {/* No aria-hidden here: the track marks inside are focusable buttons,
            and hiding them from assistive tech while leaving them tabbable is
            a WCAG 4.1.2 failure. The decorative parts opt out individually. */}
        <svg
          className="chart-overlay"
          width={view.width}
          height={totalHeight}
          viewBox={`0 0 ${view.width} ${totalHeight}`}
        >
          <defs>
            <linearGradient id="pen-trail-gradient" x1="0" x2="1" y1="0" y2="0">
              <stop offset="0%" stopColor="var(--orange)" stopOpacity="0" />
              <stop offset="100%" stopColor="var(--orange)" stopOpacity="0.12" />
            </linearGradient>
          </defs>

          {/* Track separators. */}
          {TRACK_ORDER.map((track) => (
            <line
              key={track}
              x1={0}
              y1={offsets[track]}
              x2={view.width}
              y2={offsets[track]}
              stroke="var(--ink-faint)"
              strokeWidth={1}
              opacity={0.5}
            />
          ))}

          <g transform={`translate(0, ${offsets.axis})`}>
            <TimeAxis session={session} view={view} />
          </g>
          <g transform={`translate(0, ${offsets.prompts})`}>
            <PromptTrack {...trackProps} height={TRACK_HEIGHT.prompts} />
          </g>
          <g transform={`translate(0, ${offsets.tests})`}>
            <TestTrack {...trackProps} height={TRACK_HEIGHT.tests} />
          </g>
          <g transform={`translate(0, ${offsets.commands})`}>
            <CommandTrack {...trackProps} height={TRACK_HEIGHT.commands} />
          </g>
          <g transform={`translate(0, ${offsets.incidents})`}>
            <IncidentTrack
              session={session}
              view={view}
              height={TRACK_HEIGHT.incidents}
              clockMs={clockMs}
              onSelectIncident={onSelectIncident}
            />
          </g>

          <PenHead view={view} clockMs={clockMs} height={totalHeight} />
        </svg>

        {/* Track labels sit above the SVG so they never intercept a click. */}
        {TRACK_ORDER.map((track) => (
          <span
            key={track}
            className="track-label stamp"
            style={{ top: offsets[track] + 3 }}
          >
            {track === "axis" ? "TIME" : track.toUpperCase()}
          </span>
        ))}
      </div>
    </div>
  );
}
