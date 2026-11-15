// The cockpit voice recorder: the drawer that explains one step.
//
// It shows the event, the prompt it is blamed on, the diff that caused it,
// and the surrounding transcript. Focus is trapped while open and returned
// to the opener on close.

import { useEffect, useRef } from "react";
import type { Session, Incident } from "../types";
import { asCommand, asFileChange, asPrompt, asTestRun } from "../types";
import { formatClock, incidentLabel, promptText } from "../utils/format";
import { DiffView } from "./DiffView";

interface Props {
  session: Session;
  stepIndex: number;
  /** Set when the drawer was opened from an incident marker. */
  incident?: Incident | null;
  onClose: () => void;
  onJump: (stepIndex: number) => void;
}

export function CVRPanel({
  session,
  stepIndex,
  incident,
  onClose,
  onJump,
}: Props) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  const openerRef = useRef<Element | null>(null);

  // Focus trap: remember the opener, move focus in, restore on close.
  useEffect(() => {
    openerRef.current = document.activeElement;
    panelRef.current?.focus();
    return () => {
      if (openerRef.current instanceof HTMLElement) openerRef.current.focus();
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const step = session.steps[stepIndex];
  if (!step) return null;

  const change = asFileChange(step);
  const run = asTestRun(step);
  const command = asCommand(step);
  const prompt = asPrompt(step);

  const promptId =
    change?.prompt_id ?? run?.prompt_id ?? command?.prompt_id ?? undefined;
  const blame = promptText(session, promptId);

  // The transcript: a few steps either side, so the moment has context.
  const from = Math.max(0, stepIndex - 2);
  const to = Math.min(session.steps.length, stepIndex + 3);
  const around = session.steps.slice(from, to);

  const heading =
    incident?.title ??
    change?.path ??
    run?.cmd ??
    command?.cmd ??
    prompt?.text ??
    step.kind;

  return (
    <aside
      ref={panelRef}
      className="cvr is-open"
      tabIndex={-1}
      role="complementary"
      aria-label="Step detail"
    >
      <div className="cvr-head">
        <div>
          <p className="stamp">{step.kind.replace(/_/g, " ")}</p>
          <h3 style={{ marginTop: "4px" }}>{heading}</h3>
        </div>
        <button
          className="btn btn-ghost btn-sm"
          type="button"
          onClick={onClose}
          aria-label="Close detail panel"
        >
          ✕
        </button>
      </div>

      <p className="data muted" style={{ fontSize: "0.78rem" }}>
        step {stepIndex} · {formatClock(step.t_ms)}
        {change && ` · ${change.op} · +${change.lines_added} −${change.lines_removed}`}
        {run && ` · ${run.passed} passed / ${run.failed} failed / ${run.total} total`}
        {command && ` · exit ${command.exit_code ?? "?"}`}
      </p>

      {incident && (
        <div className="cvr-blame is-incident" style={{ background: "var(--red-wash)" }}>
          <strong>{incidentLabel(incident.kind)}</strong>
          <p style={{ marginTop: "6px" }}>{incident.evidence}</p>
          <p className="muted" style={{ marginTop: "6px", fontSize: "0.78rem" }}>
            {incident.why_it_matters}
          </p>
        </div>
      )}

      {blame && (
        <div className="cvr-blame">
          <span className="stamp">Caused by</span>
          <p style={{ marginTop: "4px" }}>“{blame}”</p>
        </div>
      )}

      {change && (
        <div style={{ marginTop: "16px" }}>
          <p className="stamp" style={{ marginBottom: "8px" }}>
            Diff
          </p>
          <DiffView hunks={step.hunks} />
        </div>
      )}

      <div style={{ marginTop: "20px" }}>
        <p className="stamp" style={{ marginBottom: "8px" }}>
          Transcript
        </p>
        <ol>
          {around.map((s, i) => {
            const index = from + i;
            const active = index === stepIndex;
            return (
              <li
                key={index}
                style={{
                  padding: "6px 0",
                  borderBottom: "1px dashed var(--ink-faint)",
                  opacity: active ? 1 : 0.6,
                  fontWeight: active ? 600 : 400,
                  fontSize: "0.78rem",
                  cursor: "pointer",
                }}
                onClick={() => onJump(index)}
              >
                <span className="data faint">{formatClock(s.t_ms)}</span>{" "}
                {s.kind.replace(/_/g, " ")}
              </li>
            );
          })}
        </ol>
      </div>
    </aside>
  );
}
