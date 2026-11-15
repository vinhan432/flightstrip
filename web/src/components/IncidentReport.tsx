// The investigation: an aviation-style report over the session's incidents.
//
// The verdict, the timeline, the probable cause and the recommendations are
// all derived from the incidents. Nothing here judges whether the change is
// correct; it points at what a reviewer should read, exactly as the CLI's
// detectors do.

import type { Session, Incident, IncidentKind, Severity } from "../types";
import { formatClock, incidentLabel, maxSeverity } from "../utils/format";

export interface Verdict {
  level: Severity;
  headline: string;
}

export function verdictOf(session: Session): Verdict {
  const incidents = session.incidents ?? [];
  const level = maxSeverity(incidents.map((i) => i.severity));
  const count = incidents.length;

  if (count === 0) {
    return {
      level: "info",
      headline: "No incidents detected. The session reads as a clean run.",
    };
  }

  const critical = incidents.filter((i) => i.severity === "critical").length;
  const word = level === "critical" ? "Critical" : level === "warn" ? "Attention" : "Note";

  return {
    level,
    headline: `${word}: ${count} incident${count === 1 ? "" : "s"} across this session${
      critical > 0 ? `, ${critical} critical` : ""
    }.`,
  };
}

/** The dominant incident kind, phrased as a cause. */
export function probableCause(session: Session): string {
  const incidents = session.incidents ?? [];
  if (incidents.length === 0) {
    return "Nothing in the recording points at a specific cause. The suite was green throughout and no detector fired.";
  }

  const counts = new Map<IncidentKind, number>();
  for (const incident of incidents) {
    counts.set(incident.kind, (counts.get(incident.kind) ?? 0) + 1);
  }

  let dominant: IncidentKind = incidents[0].kind;
  let best = 0;
  for (const [kind, n] of counts) {
    if (n > best) {
      best = n;
      dominant = kind;
    }
  }

  switch (dominant) {
    case "test_weakened":
      return "The suite reached green by removing evidence rather than by fixing code: assertions were deleted and a test was skipped. The final diff looks correct; the sequence does not.";
    case "test_regression":
      return "The suite was green, work continued, and it went red. The changes between those two runs are the place to look.";
    case "thrashing":
      return "One file was rewritten repeatedly inside a single prompt, which usually means the approach was not settled before editing began.";
    case "dangerous_command":
      return "A command with an irreversible or repository-wide effect was run. The recording preserves it even though a commit diff would not.";
    case "dependency_added":
      return "New packages entered the manifest. Worth a look for supply-chain and licensing reasons rather than correctness.";
    case "secret_touched":
      return "A secret-bearing path was touched or a credential-shaped value was added. The evidence is redacted, but the fact is not.";
    case "scope_creep":
      return "The work moved beyond what was asked, touching files the prompt did not mention.";
  }
}

/** One line per incident kind present. Empty when there is nothing to do. */
export function recommendations(session: Session): string[] {
  const kinds = new Set((session.incidents ?? []).map((i) => i.kind));
  const out: string[] = [];

  if (kinds.has("test_weakened")) {
    out.push("Read the removed assertions in the diff before merging. A green suite is an outcome, not evidence.");
  }
  if (kinds.has("test_regression")) {
    out.push("Compare the test run before the red one with the run after it, and check what changed in between.");
  }
  if (kinds.has("thrashing")) {
    out.push("Ask whether the repeated rewrites settled on the right approach, or just on a passing one.");
  }
  if (kinds.has("dangerous_command")) {
    out.push("Confirm the destructive command was intended and scoped to the right path.");
  }
  if (kinds.has("dependency_added")) {
    out.push("Review the added packages, their licences and their maintenance.");
  }
  if (kinds.has("secret_touched")) {
    out.push("Rotate anything that may have been written to a tracked file, and confirm the value is not in history.");
  }
  if (kinds.has("scope_creep")) {
    out.push("Check the out-of-scope files belong in this change.");
  }

  return out;
}

interface Props {
  session: Session;
  onSelectIncident?: (incident: Incident) => void;
}

export function IncidentReport({ session, onSelectIncident }: Props) {
  const verdict = verdictOf(session);
  const cause = probableCause(session);
  const recs = recommendations(session);
  const incidents = session.incidents ?? [];

  return (
    <div className="report">
      <div className="watermark" aria-hidden="true">
        {verdict.level === "info" ? "CLEARED" : "CLASSIFIED"}
      </div>

      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "flex-start",
          gap: "16px",
          flexWrap: "wrap",
        }}
      >
        <div>
          <p className="stamp">Incident Report</p>
          <h2 style={{ marginTop: "8px", fontSize: "clamp(1.4rem,3vw,2rem)" }}>
            {session.meta.title}
          </h2>
          <p className="data muted" style={{ fontSize: "0.78rem", marginTop: "6px" }}>
            {session.meta.id} · {session.meta.repo_name} · {session.meta.agent ?? "unknown agent"}
          </p>
        </div>

        <div
          className="stamp-in"
          style={{
            border: `2px solid ${verdict.level === "info" ? "var(--green)" : "var(--red)"}`,
            color: verdict.level === "info" ? "var(--green)" : "var(--red)",
            padding: "8px 16px",
            transform: "rotate(-3deg)",
            fontFamily: "var(--font-display)",
            fontWeight: 700,
            letterSpacing: "0.18em",
            textTransform: "uppercase",
            fontSize: "0.8rem",
          }}
        >
          {verdict.level === "info" ? "Cleared" : verdict.level === "warn" ? "Review" : "Critical"}
        </div>
      </div>

      <div className="report-block">
        <h3>Verdict</h3>
        <p>{verdict.headline}</p>
      </div>

      <div className="report-block">
        <h3>Timeline</h3>
        {incidents.length === 0 ? (
          <p className="muted">No incidents to list.</p>
        ) : (
          <ol className="report-timeline">
            {incidents.map((incident) => (
              <li
                key={incident.id}
                style={{ cursor: onSelectIncident ? "pointer" : "default" }}
                onClick={() => onSelectIncident?.(incident)}
              >
                <span className="data faint">{formatClock(incident.t_ms)}</span>
                <span>
                  <strong>{incident.title}</strong>
                  <span className="muted"> — {incidentLabel(incident.kind)}</span>
                  <br />
                  <code style={{ fontSize: "0.76rem" }}>{incident.evidence}</code>
                </span>
              </li>
            ))}
          </ol>
        )}
      </div>

      <div className="report-block">
        <h3>Probable Cause</h3>
        <p>{cause}</p>
      </div>

      <div className="report-block">
        <h3>Recommendations</h3>
        {recs.length === 0 ? (
          <p className="muted">Nothing to action.</p>
        ) : (
          <ul>
            {recs.map((rec, i) => (
              <li key={i} style={{ padding: "4px 0" }}>
                — {rec}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div
        className="report-block"
        style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <p className="stamp" style={{ margin: 0 }}>
          Detectors are deterministic rules. They never claim the change is correct.
        </p>
        <button
          className="btn btn-ghost btn-sm"
          type="button"
          onClick={() => window.print()}
        >
          Print
        </button>
      </div>
    </div>
  );
}
