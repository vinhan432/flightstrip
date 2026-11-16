// The cockpit voice recorder: the drawer that explains one step.
//
// It shows the event, the prompt it is blamed on, the diff that caused it,
// and the surrounding transcript. Focus is trapped while open and returned
// to the opener on close.
//
// Layout by viewport (see components.css): a sticky right drawer on desktop,
// a bottom sheet on tablet, a full-screen overlay on mobile.

import { useEffect, useRef } from "react";
import type { Session, Incident } from "../types";
import { asCommand, asFileChange, asPrompt, asTestRun } from "../types";
import {
  formatClock,
  formatDuration,
  incidentLabel,
  promptNumber,
  promptStep,
  promptText,
} from "../utils/format";
import { DiffView } from "./DiffView";

interface Props {
  session: Session;
  stepIndex: number;
  /** Set when the drawer was opened from an incident marker. */
  incident?: Incident | null;
  onClose: () => void;
  onJump: (stepIndex: number) => void;
}

/** How many steps either side of the selection the transcript shows. */
const TRANSCRIPT_SPAN = 5;

const FOCUSABLE =
  'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])';

export function CVRPanel({
  session,
  stepIndex,
  incident,
  onClose,
  onJump,
}: Props) {
  const panelRef = useRef<HTMLElement | null>(null);
  const openerRef = useRef<Element | null>(null);

  // Focus trap: remember the opener, move focus in, restore on close, and
  // keep Tab inside the panel while it is open.
  useEffect(() => {
    openerRef.current = document.activeElement;
    panelRef.current?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
        return;
      }
      if (event.key !== "Tab") return;

      const panel = panelRef.current;
      if (!panel) return;
      const items = [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)];
      if (items.length === 0) return;

      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;

      // The panel itself is focusable, so treat "not in the list" as the
      // start of the cycle rather than letting Tab escape.
      if (event.shiftKey && (active === first || active === panel)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      if (openerRef.current instanceof HTMLElement) openerRef.current.focus();
    };
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
  const blameNumber = promptNumber(session, promptId);
  const blameStep = promptStep(session, promptId);

  // The transcript: a few steps either side, so the moment has context.
  const from = Math.max(0, stepIndex - TRANSCRIPT_SPAN);
  const to = Math.min(session.steps.length, stepIndex + TRANSCRIPT_SPAN + 1);
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
      className={`cvr is-open${incident ? " is-incident" : ""}`}
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

      <p className="data muted cvr-meta">
        step {stepIndex} · {formatClock(step.t_ms)}
        {incident && (
          <>
            {" · "}
            <span className={`sev-tag sev-${incident.severity}`}>
              {incident.severity}
            </span>
          </>
        )}
      </p>

      {/* ── the incident itself ─────────────────────────────────────── */}
      {incident && (
        <div className="cvr-blame is-incident">
          <span className="stamp">{incidentLabel(incident.kind)}</span>
          <p style={{ marginTop: "6px" }}>{incident.evidence}</p>
          <p className="muted cvr-why">{incident.why_it_matters}</p>
        </div>
      )}

      {/* ── prompt blame ────────────────────────────────────────────── */}
      {blame && (
        <div className="cvr-blame">
          <span className="stamp">
            Caused by prompt{blameNumber !== null ? ` #${blameNumber}` : ""}
          </span>
          <p style={{ marginTop: "4px" }}>“{blame}”</p>
          {blameStep !== null && (
            <button
              className="btn btn-ghost btn-sm cvr-jump"
              type="button"
              onClick={() => onJump(blameStep)}
            >
              Jump to the prompt
            </button>
          )}
        </div>
      )}

      {/* ── the event's own detail ──────────────────────────────────── */}
      {change && (
        <>
          <dl className="cvr-facts">
            <dt>Operation</dt>
            <dd>{change.op}</dd>
            <dt>Lines</dt>
            <dd>
              <span className="diff-add-text">+{change.lines_added}</span>{" "}
              <span className="diff-remove-text">−{change.lines_removed}</span>
            </dd>
            {change.before_hash && (
              <>
                <dt>Before</dt>
                <dd className="data">{change.before_hash.slice(0, 12)}</dd>
              </>
            )}
            {change.after_hash && (
              <>
                <dt>After</dt>
                <dd className="data">{change.after_hash.slice(0, 12)}</dd>
              </>
            )}
          </dl>

          <div style={{ marginTop: "16px" }}>
            <p className="stamp" style={{ marginBottom: "8px" }}>
              Diff
            </p>
            <DiffView hunks={step.hunks} />
          </div>
        </>
      )}

      {command && (
        <dl className="cvr-facts">
          <dt>Command</dt>
          <dd className="data">{command.cmd}</dd>
          <dt>Exit</dt>
          <dd
            className="data"
            style={{
              color:
                (command.exit_code ?? 0) === 0 ? "var(--green)" : "var(--red)",
            }}
          >
            {command.exit_code ?? "unknown"}
          </dd>
          {command.duration_ms !== undefined && (
            <>
              <dt>Duration</dt>
              <dd className="data">{formatDuration(command.duration_ms)}</dd>
            </>
          )}
          <dt>Source</dt>
          <dd className="data">{command.source}</dd>
        </dl>
      )}

      {run && (
        <dl className="cvr-facts">
          <dt>Command</dt>
          <dd className="data">{run.cmd}</dd>
          <dt>Result</dt>
          <dd style={{ color: run.ok ? "var(--green)" : "var(--red)" }}>
            {run.ok ? "passing" : "failing"}
          </dd>
          <dt>Counts</dt>
          <dd className="data">
            {run.passed} passed · {run.failed} failed · {run.skipped} skipped ·{" "}
            {run.total} total
          </dd>
        </dl>
      )}

      {prompt && (
        <dl className="cvr-facts">
          <dt>Prompt</dt>
          <dd>“{prompt.text}”</dd>
          <dt>Source</dt>
          <dd className="data">{prompt.source}</dd>
        </dl>
      )}

      {/* ── the transcript ──────────────────────────────────────────── */}
      <div style={{ marginTop: "20px" }}>
        <p className="stamp" style={{ marginBottom: "8px" }}>
          Transcript
        </p>
        <ol className="cvr-transcript">
          {around.map((s, i) => {
            const index = from + i;
            const active = index === stepIndex;
            return (
              <li key={index} className={active ? "is-active" : undefined}>
                <button
                  type="button"
                  onClick={() => onJump(index)}
                  aria-current={active ? "step" : undefined}
                >
                  <span className="data faint">{formatClock(s.t_ms)}</span>{" "}
                  {s.kind.replace(/_/g, " ")}
                </button>
              </li>
            );
          })}
        </ol>
      </div>
    </aside>
  );
}
