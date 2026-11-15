// The trace map: prompts on the left, files on the right, incidents between.
//
// The layout is deterministic — index-based, no force simulation — so the
// same session always draws the same graph and a screenshot is meaningful.

import { useMemo, useState } from "react";
import type { Session } from "../types";
import { incidentLabel } from "../utils/format";

export interface Node {
  id: string;
  kind: "prompt" | "file" | "incident";
  x: number;
  y: number;
  r?: number;
  label: string;
  step: number;
}

export interface Edge {
  from: string;
  to: string;
}

export interface GraphLayout {
  prompts: Node[];
  files: Node[];
  incidents: Node[];
  edges: Edge[];
  width: number;
  height: number;
}

const HEIGHT = 500;
const COL_PROMPT = 130;
const COL_INCIDENT = 420;
const COL_FILE = 720;
const MIN_R = 20;
const MAX_R = 50;

/** Spread `count` items vertically, centred in the graph. */
function columnY(index: number, count: number): number {
  if (count <= 1) return HEIGHT / 2;
  const usable = HEIGHT - 120;
  const step = usable / (count - 1);
  return 60 + index * step;
}

export function graphLayout(session: Session): GraphLayout {
  const prompts = session.prompts ?? [];
  const fileEntries = Object.entries(session.files ?? {});
  const incidents = session.incidents ?? [];

  const maxChurn = fileEntries.reduce(
    (max, [, summary]) => Math.max(max, summary.churn ?? 0),
    0,
  );

  const promptNodes: Node[] = prompts.map((prompt, i) => ({
    id: prompt.id,
    kind: "prompt",
    x: COL_PROMPT,
    y: columnY(i, prompts.length),
    label: prompt.text,
    step: prompt.step,
  }));

  const fileNodes: Node[] = fileEntries.map(([path, summary], i) => {
    const ratio = maxChurn > 0 ? (summary.churn ?? 0) / maxChurn : 0;
    return {
      id: path,
      kind: "file",
      x: COL_FILE,
      y: columnY(i, fileEntries.length),
      r: MIN_R + ratio * (MAX_R - MIN_R),
      label: path,
      step: 0,
    };
  });

  const incidentNodes: Node[] = incidents.map((incident, i) => ({
    id: incident.id,
    kind: "incident",
    x: COL_INCIDENT,
    y: columnY(i, incidents.length),
    label: incidentLabel(incident.kind),
    step: incident.step,
  }));

  const edges: Edge[] = [];

  // Prompt → file, for each file the prompt touched.
  for (const [path, summary] of fileEntries) {
    for (const promptId of summary.prompts ?? []) {
      if (prompts.some((p) => p.id === promptId)) {
        edges.push({ from: promptId, to: path });
      }
    }
  }

  // Incident → file, when the incident names a path that is in the graph.
  for (const incident of incidents) {
    if (incident.path && fileEntries.some(([path]) => path === incident.path)) {
      edges.push({ from: incident.id, to: incident.path });
    }
  }

  return {
    prompts: promptNodes,
    files: fileNodes,
    incidents: incidentNodes,
    edges,
    width: 860,
    height: HEIGHT,
  };
}

interface Props {
  session: Session;
  onJumpToStep: (step: number) => void;
}

export function TraceGraph({ session, onJumpToStep }: Props) {
  const graph = useMemo(() => graphLayout(session), [session]);
  const [hovered, setHovered] = useState<string | null>(null);

  const nodeById = useMemo(() => {
    const map = new Map<string, Node>();
    for (const node of [...graph.prompts, ...graph.files, ...graph.incidents]) {
      map.set(node.id, node);
    }
    return map;
  }, [graph]);

  const isConnected = (edge: Edge) =>
    hovered === null || edge.from === hovered || edge.to === hovered;

  const isDimmed = (node: Node) => {
    if (hovered === null) return false;
    if (node.id === hovered) return false;
    return !graph.edges.some(
      (e) =>
        (e.from === hovered && e.to === node.id) ||
        (e.to === hovered && e.from === node.id),
    );
  };

  return (
    <div className="graph-wrap">
      <svg
        width={graph.width}
        height={graph.height}
        viewBox={`0 0 ${graph.width} ${graph.height}`}
        role="img"
        aria-label="Trace map: prompts, files and incidents"
      >
        {/* Edges first, so nodes sit on top. */}
        {graph.edges.map((edge, i) => {
          const from = nodeById.get(edge.from);
          const to = nodeById.get(edge.to);
          if (!from || !to) return null;

          // A gentle curve: control point halfway, offset vertically.
          const midX = (from.x + to.x) / 2;
          const midY = (from.y + to.y) / 2 - 24;
          const d = `M ${from.x} ${from.y} Q ${midX} ${midY} ${to.x} ${to.y}`;
          const active = hovered !== null && isConnected(edge);

          return (
            <path
              key={i}
              className={`graph-edge${hovered !== null && active ? " is-active" : ""}`}
              d={d}
              opacity={hovered !== null && !active ? 0.15 : 0.6}
            />
          );
        })}

        {/* Prompt nodes: dashed orange rounded rects. */}
        {graph.prompts.map((node) => (
          <g
            key={node.id}
            className="graph-node"
            opacity={isDimmed(node) ? 0.3 : 1}
            transform={`translate(${node.x - 90}, ${node.y - 25})`}
            role="button"
            tabIndex={0}
            aria-label={`Prompt: ${node.label}`}
            onMouseEnter={() => setHovered(node.id)}
            onMouseLeave={() => setHovered(null)}
            onClick={() => onJumpToStep(node.step)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") onJumpToStep(node.step);
            }}
          >
            <rect
              width={180}
              height={50}
              rx={6}
              fill="var(--paper)"
              stroke="var(--orange)"
              strokeWidth={1.5}
              strokeDasharray="5 3"
            />
            <foreignObject x={8} y={6} width={164} height={38}>
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: "9px",
                  lineHeight: 1.3,
                  color: "var(--ink)",
                  overflow: "hidden",
                  height: "100%",
                }}
              >
                {node.label.length > 90
                  ? `${node.label.slice(0, 90)}…`
                  : node.label}
              </div>
            </foreignObject>
          </g>
        ))}

        {/* File nodes: circles sized by churn. */}
        {graph.files.map((node) => (
          <g
            key={node.id}
            className="graph-node"
            opacity={isDimmed(node) ? 0.3 : 1}
            role="button"
            tabIndex={0}
            aria-label={`File: ${node.label}`}
            onMouseEnter={() => setHovered(node.id)}
            onMouseLeave={() => setHovered(null)}
          >
            <circle
              cx={node.x}
              cy={node.y}
              r={node.r}
              fill="var(--paper-dark)"
              stroke="var(--ink)"
              strokeWidth={1.5}
            />
            <text
              x={node.x}
              y={node.y + 4}
              fontSize={9}
              fill="var(--ink)"
              fontFamily="var(--font-mono)"
              textAnchor="middle"
            >
              {node.label.split("/").pop()}
            </text>
          </g>
        ))}

        {/* Incident nodes: red triangles. */}
        {graph.incidents.map((node) => (
          <g
            key={node.id}
            className="graph-node"
            opacity={isDimmed(node) ? 0.3 : 1}
            role="button"
            tabIndex={0}
            aria-label={`Incident: ${node.label}`}
            onMouseEnter={() => setHovered(node.id)}
            onMouseLeave={() => setHovered(null)}
            onClick={() => onJumpToStep(node.step)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") onJumpToStep(node.step);
            }}
          >
            <polygon
              points={`${node.x},${node.y - 12} ${node.x - 12},${node.y + 10} ${node.x + 12},${node.y + 10}`}
              fill="var(--red)"
              stroke="var(--paper)"
              strokeWidth={1.5}
            />
            <text
              x={node.x}
              y={node.y + 6}
              fontSize={10}
              fill="var(--paper)"
              fontFamily="var(--font-display)"
              fontWeight={700}
              textAnchor="middle"
            >
              !
            </text>
          </g>
        ))}
      </svg>
    </div>
  );
}
