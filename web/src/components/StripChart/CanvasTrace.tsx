// The file-churn trace: the spine of the strip chart, drawn on canvas.
//
// `tracePoints` is pure so the geometry can be tested without a canvas.
// The drawing adds a small per-point jitter to Y so the line reads as ink
// rather than as a vector render.

import { useEffect, useMemo, useRef } from "react";
import type { Session } from "../../types";
import { churnSeries, xFor, type View } from "../../utils/layout";
import { downsample } from "../../utils/downsample";

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
  /** How far the pen has travelled, in session ms. */
  clockMs: number;
  height?: number;
}

export function CanvasTrace({ session, view, clockMs, height = TRACK_H }: Props) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  const points = useMemo(
    () => tracePoints(session, view, Math.max(200, view.width * 2)),
    [session, view],
  );

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const width = Math.max(1, view.width);
    canvas.width = Math.floor(width * dpr);
    canvas.height = Math.floor(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
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
    for (const point of points) {
      if (point.x > revealX) break;
      // ±1px of jitter: the line should look drawn, not computed.
      const jitter = (Math.random() - 0.5) * 2;
      const y = Math.max(1, Math.min(height - 1, point.y + jitter));
      if (!started) {
        ctx.moveTo(point.x, y);
        started = true;
      } else {
        ctx.lineTo(point.x, y);
      }
    }
    if (started) ctx.stroke();

    // The last drawn point gets a nib mark so the trace reads as in-progress.
    const drawn = points.filter((p) => p.x <= revealX);
    if (drawn.length > 0) {
      const tip = drawn[drawn.length - 1];
      ctx.fillStyle = "#d4621a";
      ctx.beginPath();
      ctx.arc(tip.x, tip.y, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }, [points, view, clockMs, height]);

  return (
    <canvas
      ref={canvasRef}
      className="chart-canvas"
      aria-hidden="true"
      style={{ position: "absolute", left: 0, top: 0 }}
    />
  );
}
