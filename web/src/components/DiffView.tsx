// A lightweight diff renderer: a table, no Monaco, no syntax highlighting.
// Line numbers in --ink-faint, `+` on a green wash, `-` on a red wash.

import { useState } from "react";
import type { DiffHunk } from "../types";

interface Props {
  hunks: DiffHunk[] | undefined;
  maxLines?: number;
}

const DEFAULT_MAX = 60;

export function DiffView({ hunks, maxLines = DEFAULT_MAX }: Props) {
  const [expanded, setExpanded] = useState(false);

  if (!hunks || hunks.length === 0) {
    return (
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        No diff recorded for this step.
      </p>
    );
  }

  // Flatten to rows, tracking a line number per side.
  const rows: Array<{
    tag: string;
    text: string;
    oldLine: number | null;
    newLine: number | null;
  }> = [];

  for (const hunk of hunks) {
    let oldLine = hunk.old_start;
    let newLine = hunk.new_start;
    for (const line of hunk.lines) {
      if (line.tag === "add") {
        rows.push({ tag: line.tag, text: line.text, oldLine: null, newLine });
        newLine++;
      } else if (line.tag === "remove") {
        rows.push({ tag: line.tag, text: line.text, oldLine, newLine: null });
        oldLine++;
      } else {
        rows.push({ tag: line.tag, text: line.text, oldLine, newLine });
        oldLine++;
        newLine++;
      }
    }
  }

  const truncated = !expanded && rows.length > maxLines;
  const shown = truncated ? rows.slice(0, maxLines) : rows;

  const mark = (tag: string) =>
    tag === "add" ? "+" : tag === "remove" ? "-" : " ";

  return (
    <>
      <table className="diff">
        <tbody>
          {shown.map((row, i) => (
            <tr key={i} className={row.tag}>
              <td className="ln">{row.oldLine ?? ""}</td>
              <td className="ln">{row.newLine ?? ""}</td>
              <td className="mark">{mark(row.tag)}</td>
              <td>{row.text}</td>
            </tr>
          ))}

          {/* A truncation marker, the way a printed diff ends with a rule
              rather than stopping mid-line with no explanation. */}
          {truncated && (
            <tr className="diff-truncated">
              <td className="ln" />
              <td className="ln" />
              <td className="mark">⋯</td>
              <td>{rows.length - maxLines} more lines not shown</td>
            </tr>
          )}
        </tbody>
      </table>

      {truncated && (
        <button
          className="btn btn-ghost btn-sm"
          type="button"
          style={{ marginTop: "8px" }}
          onClick={() => setExpanded(true)}
        >
          Show full diff
        </button>
      )}
    </>
  );
}
