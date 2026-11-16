// The file-churn trace: the spine of the strip chart, drawn on canvas.
//
// `tracePoints` is pure so the geometry can be tested without a canvas.
// The drawing adds a small per-point jitter to Y so the line reads as ink
// rather than as a vector render.
//
// The reveal position comes from the playback clock directly, so a tick
// repaints the canvas without re-rendering the React tree around it.

import { useEffect, useMemo, useRef } from "react";
import type { Session } from "../../types";
import { churnSeries, xFor, type View } from "../../utils/layout";
import { downsample } from "../../utils/downsample";
import { useClock } from "../../hooks/useClock";
import type { Playback } from "../../hooks/usePlayback";

export interface TracePt {
  x: number;
  y: number;
}

const TRACK_H = 100;
const PADDING = 6;

/**
 * Map the cumulative churn series into canvas coordinates. Churn 0 sits at
 * the bottom of the track; the session's peak churn sits at the top.
 */
export function tracePoints(
  session: Session,
  view: View,
  maxPoints: number,
): TracePt[] {
  const series = churnSeries(session);
  const sampled = downsample(series, maxPoints);

  const peak = series.reduce((max, p) => Math.max(max, p.churn), 0);
  const usable = TRACK_H - PADDING * 2;
  const span = view.t1 - view.t0;

  return sampled.map((point) => {
    const x = span <= 0 ? 0 : xFor(point.t, view);
    const ratio = peak > 0 ? point.churn / peak : 0;
    const y = TRACK_H - PADDING - ratio * usable;
    return { x, y: Number.isFinite(y) ? y : TRACK_H - PADDING };
  });
}

interface Props {
  session: Session;
  view: View;
  playback: Playback;
  height?: number;
  /** Vertical offset of the track this trace belongs to. */
  top?: number;
}

export function CanvasTrace({
  session,
  view,
  playback,
  height = TRACK_H,
  top = 0,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const clockMs = useClock(playback);

  const points = useMemo(
    () => tracePoints(session, view, Math.max(200, view.width * 2)),
    [session, view],
  );

  // The jitter is computed once per point set, not per frame. Re-randomising
  // it on every tick made the ink line shimmer while the pen moved.
  const jitter = useMemo(
    () => points.map(() => (Math.random() - 0.5) * 2),
    [points],
  );

  // Size the backing store only when the geometry actually changes.
  // Assigning canvas.width reallocates and clears it, so doing that inside
  // the per-frame draw was both expensive and a source of flicker.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const width = Math.max(1, view.width);
    canvas.width = Math.floor(width * dpr);
    canvas.height = Math.floor(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
  }, [view.width, height]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const width = Math.max(1, view.width);

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);

    if (points.length === 0) return;

    // Only the part the pen has reached is drawn.
    const revealX = xFor(clockMs, view);

    ctx.strokeStyle = "#1c1917";
    ctx.lineWidth = 2;
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    ctx.beginPath();

    let started = false;
    let tip: TracePt | null = null;
    for (let i = 0; i < points.length; i++) {
      const point = points[i];
      if (point.x > revealX) break;
      const y = Math.max(1, Math.min(height - 1, point.y + jitter[i]));
      if (!started) {
        ctx.moveTo(point.x, y);
        started = true;
      } else {
        ctx.lineTo(point.x, y);
      }
      tip = point;
    }
    if (started) ctx.stroke();

    // The last drawn point gets a nib mark so the trace reads as in-progress.
    if (tip) {
      ctx.fillStyle = "#d4621a";
      ctx.beginPath();
      ctx.arc(tip.x, tip.y, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }, [points, jitter, view, clockMs, height]);

  return (
    <canvas
      ref={canvasRef}
      className="chart-canvas"
      aria-hidden="true"
      style={{ position: "absolute", left: 0, top }}
    />
  );
}
