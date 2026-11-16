// Presentation helpers. Pure functions, no DOM.

import type { Session, Severity, IncidentKind } from "../types";

/** `83000` → `"1:23"`. Rolls to `h:mm:ss` past an hour. Negative clamps to 0. */
export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** `99000` → `"1m 39s"`, `3723000` → `"1h 2m"`. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

/**
 * Join a prompt id back to its text. The `files` map and every event carry
 * ids, never text, so this is the one place the join happens.
 */
export function promptText(session: Session, id: string | undefined): string {
  if (!id) return "";
  return session.prompts.find((p) => p.id === id)?.text ?? "";
}

/**
 * The 1-based number a prompt is shown as ("Caused by prompt #3"), or null
 * when the id is unknown. Numbering follows the `prompts` array, which is the
 * order the recorder emitted them in.
 */
export function promptNumber(session: Session, id: string | undefined): number | null {
  if (!id) return null;
  const index = session.prompts.findIndex((p) => p.id === id);
  return index === -1 ? null : index + 1;
}

/** The step a prompt was issued at, for a "jump to the prompt" action. */
export function promptStep(session: Session, id: string | undefined): number | null {
  if (!id) return null;
  const prompt = session.prompts.find((p) => p.id === id);
  return prompt ? prompt.step : null;
}

export interface FileRow {
  path: string;
  churn: number;
  ops: string;
  promptLabels: string[];
}

/** The file table: one row per file, busiest first, prompt ids resolved. */
export function fileRows(session: Session): FileRow[] {
  const files = session.files ?? {};
  return Object.entries(files)
    .map(([path, summary]) => ({
      path,
      churn: summary.churn,
      ops: Object.entries(summary.ops ?? {})
        .map(([op, n]) => `${op}×${n}`)
        .join(" "),
      promptLabels: (summary.prompts ?? [])
        .map((id) => promptText(session, id))
        .filter(Boolean),
    }))
    .sort((a, b) => b.churn - a.churn || a.path.localeCompare(b.path));
}

/** Severity → a CSS custom-property name. Never a raw hex. */
export function severityVar(severity: Severity): string {
  switch (severity) {
    case "critical":
      return "--red";
    case "warn":
      return "--orange";
    default:
      return "--ink-faint";
  }
}

/** Incident kind → the short label used on the tape and in the report. */
export function incidentLabel(kind: IncidentKind): string {
  switch (kind) {
    case "test_weakened":
      return "Test weakened";
    case "test_regression":
      return "Test regression";
    case "thrashing":
      return "Thrashing";
    case "dangerous_command":
      return "Dangerous command";
    case "dependency_added":
      return "Dependency added";
    case "secret_touched":
      return "Secret exposed";
    case "scope_creep":
      return "Scope creep";
  }
}

/** The highest severity present. `info` when there are none. */
export function maxSeverity(severities: Severity[]): Severity {
  if (severities.includes("critical")) return "critical";
  if (severities.includes("warn")) return "warn";
  return "info";
}
