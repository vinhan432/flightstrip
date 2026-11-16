// Measure the chart's hot paths at 3,000 steps. Run with:
//   node scripts/bench.mjs
//
// This is a dev tool. It imports the built modules via a tiny esbuild-free
// path: the pure functions are re-implemented here only where they are pure
// arithmetic, and the real ones are exercised through vitest in the test
// suite. This script measures the two things that decide whether the chart
// stays smooth: the downsample+map cost, and the number of SVG marks the
// viewport culling leaves in the DOM.

import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const synthetic = resolve(here, "..", "public", "sessions", "synthetic.json");

if (!existsSync(synthetic)) {
  console.error("bench: run `npm run synthetic` first");
  process.exit(1);
}

const session = JSON.parse(readFileSync(synthetic, "utf8"));

// ── the churn series and the downsample ─────────────────────────────────
function churnSeries(s) {
  const points = [{ t: 0, churn: 0 }];
  let churn = 0;
  for (const step of s.steps) {
    if (step.seq === 0 || step.kind === "session_start") continue;
    if (step.kind !== "file_change") continue;
    churn += (step.lines_added ?? 0) + (step.lines_removed ?? 0);
    points.push({ t: step.t_ms, churn });
  }
  return points;
}

function downsample(points, maxPoints) {
  if (maxPoints <= 0) return [];
  if (points.length <= maxPoints) return points;
  if (maxPoints < 3) return [points[0], points[points.length - 1]];
  const sampled = [points[0]];
  const every = (points.length - 2) / (maxPoints - 2);
  let a = 0;
  for (let i = 0; i < maxPoints - 2; i++) {
    const avgStart = Math.floor((i + 1) * every) + 1;
    const avgEnd = Math.floor((i + 2) * every) + 1;
    const avgLen = Math.min(avgEnd, points.length) - avgStart;
    let avgT = 0;
    let avgC = 0;
    for (let j = avgStart; j < avgStart + avgLen; j++) {
      avgT += points[j].t;
      avgC += points[j].churn;
    }
    if (avgLen > 0) {
      avgT /= avgLen;
      avgC /= avgLen;
    }
    const from = Math.floor(i * every) + 1;
    const to = Math.floor((i + 1) * every) + 1;
    const pa = points[a];
    let maxArea = -1;
    let chosen = from;
    for (let j = from; j < to && j < points.length; j++) {
      const area = Math.abs(
        (pa.t - avgT) * (points[j].churn - pa.churn) -
          (pa.t - points[j].t) * (avgC - pa.churn),
      );
      if (area > maxArea) {
        maxArea = area;
        chosen = j;
      }
    }
    sampled.push(points[chosen]);
    a = chosen;
  }
  sampled.push(points[points.length - 1]);
  return sampled;
}

const series = churnSeries(session);
const width = 1200; // a typical chart width

let t0 = performance.now();
let points;
for (let i = 0; i < 100; i++) points = downsample(series, Math.max(200, width * 2));
const downsampleMs = (performance.now() - t0) / 100;

// ── the viewport culling: how many marks survive ────────────────────────
// A full-width view (no zoom) shows everything; the culling only helps when
// zoomed. Report both, because the unzoomed case is the worst case.
function countVisible(t0, t1) {
  const pad = t1 - t0;
  const lo = t0 - pad;
  const hi = t1 + pad;
  const prompts = session.prompts.filter((p) => p.t_ms >= lo && p.t_ms <= hi).length;
  const commands = session.steps.filter(
    (s) => s.kind === "command" && s.t_ms >= lo && s.t_ms <= hi,
  ).length;
  const tests = session.steps.filter(
    (s) => s.kind === "test_run" && s.t_ms >= lo && s.t_ms <= hi,
  ).length;
  return { prompts, commands, tests };
}

const full = countVisible(0, session.meta.duration_ms);
const zoomed = countVisible(
  session.meta.duration_ms * 0.4,
  session.meta.duration_ms * 0.42,
);

console.log(`bench: ${session.steps.length} steps, ${series.length} churn points`);
console.log(`  downsample to ${points.length} pts: ${downsampleMs.toFixed(3)} ms`);
console.log(
  `  SVG marks, full view: ${full.prompts} prompts, ${full.commands} commands, ${full.tests} tests`,
);
console.log(
  `  SVG marks, 50x zoom:  ${zoomed.prompts} prompts, ${zoomed.commands} commands, ${zoomed.tests} tests`,
);
