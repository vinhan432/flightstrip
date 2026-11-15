// Colour decisions, in one place. Everything returns a CSS custom-property
// name from tokens.css so the palette cannot drift into components.

import type { IncidentKind, Severity } from "../types";

export function severityColor(severity: Severity): string {
  switch (severity) {
    case "critical":
      return "var(--red)";
    case "warn":
      return "var(--orange)";
    default:
      return "var(--ink-faint)";
  }
}

export function severityClass(severity: Severity): string {
  return `sev-${severity}`;
}

/** Incident kinds are grouped by what a reviewer should do about them. */
export function kindColor(kind: IncidentKind): string {
  switch (kind) {
    case "test_weakened":
    case "test_regression":
    case "secret_touched":
    case "dangerous_command":
      return "var(--red)";
    case "thrashing":
    case "dependency_added":
    case "scope_creep":
      return "var(--orange)";
  }
}

/** A test run's state, used by the tests track and the report. */
export function testStateColor(ok: boolean): string {
  return ok ? "var(--green)" : "var(--red)";
}
