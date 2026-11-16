// Generate a synthetic 3,000-step session to prove the chart stays smooth at
// scale. This is a DEV TOOL, not sample data: it writes to public/sessions/
// only when run by hand, and the session is labelled `synthetic` so the site
// shows the honest badge.
//
//   node scripts/make-synthetic.mjs            # 3000 steps
//   node scripts/make-synthetic.mjs 12000      # bigger, for a stress check

import { writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(here, "..");
const outDir = resolve(webRoot, "public", "sessions");

const count = Number(process.argv[2] ?? 3000);
const stepMs = 40;

mkdirSync(outDir, { recursive: true });

const paths = Array.from({ length: 60 }, (_, i) => `src/module-${i}/index.ts`);
const commands = ["npm test", "npm run lint", "cargo check", "git status"];

const steps = [];
const prompts = [];
const incidents = [];
const files = {};

let t = 0;
let seq = 1;
let promptIndex = 0;

// Step 0 is the absolute anchor: an epoch timestamp, per the wire format.
steps.push({
  seq: 0,
  t_ms: Date.UTC(2026, 0, 1),
  kind: "session_start",
  title: "Synthetic stress session",
  os: "linux",
  repo_name: "synthetic-repo",
});

for (let i = 0; i < count; i++) {
  t += stepMs;
  const roll = i % 25;

  if (roll === 0) {
    const id = `p-${promptIndex++}`;
    prompts.push({
      id,
      text: `Synthetic prompt ${promptIndex}: refactor module ${i}`,
      step: steps.length,
      t_ms: t,
      files_touched: 3,
      commands_run: 1,
      incidents: 0,
    });
    steps.push({
      seq: seq++,
      t_ms: t,
      kind: "prompt",
      id,
      text: `Synthetic prompt ${promptIndex}: refactor module ${i}`,
      source: "mark",
    });
    continue;
  }

  if (roll === 12) {
    const ok = i % 50 !== 12;
    steps.push({
      seq: seq++,
      t_ms: t,
      kind: "test_run",
      cmd: "npm test",
      passed: ok ? 120 : 118,
      failed: ok ? 0 : 2,
      skipped: 1,
      total: ok ? 121 : 121,
      ok,
      prompt_id: `p-${Math.max(0, promptIndex - 1)}`,
    });
    continue;
  }

  if (roll === 20) {
    const failed = i % 100 === 20;
    steps.push({
      seq: seq++,
      t_ms: t,
      kind: "command",
      cmd: commands[i % commands.length],
      exit_code: failed ? 1 : 0,
      duration_ms: 100 + (i % 400),
      source: "exec",
      prompt_id: `p-${Math.max(0, promptIndex - 1)}`,
    });
    continue;
  }

  // Otherwise: a file change.
  const path = paths[i % paths.length];
  const added = 1 + (i % 12);
  const removed = i % 5;
  steps.push({
    seq: seq++,
    t_ms: t,
    kind: "file_change",
    path,
    op: i % 17 === 0 ? "create" : "modify",
    lines_added: added,
    lines_removed: removed,
    prompt_id: `p-${Math.max(0, promptIndex - 1)}`,
  });

  const entry = (files[path] ??= { churn: 0, prompts: [], ops: {} });
  entry.churn += added + removed;
  const pid = `p-${Math.max(0, promptIndex - 1)}`;
  if (!entry.prompts.includes(pid)) entry.prompts.push(pid);
  entry.ops.modify = (entry.ops.modify ?? 0) + 1;
}

// Two incidents, so the incident track and the report have something to show.
incidents.push({
  id: "i-0",
  kind: "thrashing",
  severity: "warn",
  step: Math.floor(steps.length * 0.4),
  t_ms: steps[Math.floor(steps.length * 0.4)].t_ms,
  title: "One file rewritten many times",
  evidence: "src/module-7/index.ts changed 40+ times.",
  why_it_matters: "Repeated rewrites of one file usually mean the agent is guessing.",
  prompt_id: "p-2",
  path: "src/module-7/index.ts",
});
incidents.push({
  id: "i-1",
  kind: "test_regression",
  severity: "critical",
  step: Math.floor(steps.length * 0.7),
  t_ms: steps[Math.floor(steps.length * 0.7)].t_ms,
  title: "Suite went red",
  evidence: "2 tests failed after a refactor.",
  why_it_matters: "A red suite shipped to the next prompt compounds the damage.",
  prompt_id: "p-5",
  path: null,
});

steps.push({ seq: seq++, t_ms: t + stepMs, kind: "session_end" });

const session = {
  version: 1,
  meta: {
    id: "synthetic",
    title: `Synthetic stress session (${count} steps)`,
    agent: "synthetic",
    started_at: new Date(Date.UTC(2026, 0, 1)).toISOString(),
    duration_ms: t,
    os: "linux",
    repo_name: "synthetic-repo",
    label: "synthetic",
  },
  prompts,
  steps,
  incidents,
  files,
  stats: {
    steps: steps.length,
    files_touched: Object.keys(files).length,
    commands: steps.filter((s) => s.kind === "command").length,
    test_runs: steps.filter((s) => s.kind === "test_run").length,
    incidents_by_kind: { thrashing: 1, test_regression: 1 },
    tests_first_ok: true,
    tests_last_ok: false,
  },
};

const out = resolve(outDir, "synthetic.json");
writeFileSync(out, JSON.stringify(session));
console.log(
  `make-synthetic: wrote ${steps.length} steps (${count} generated) to ${out}`,
);
