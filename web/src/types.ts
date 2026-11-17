// The wire format. Every field here mirrors
// `crates/flightstrip-core/src/schema.rs` exactly, including optionality.
// This is the only place the shape is written in TypeScript.

export type FileOp = "create" | "modify" | "delete" | "rename";

export type DiffTag = "context" | "add" | "remove";

export type PromptSource = "mark" | "pty" | "import";

export type CommandSource = "exec" | "pty" | "import";

// `recorded` and `fixture` are what the Rust writer emits (schema.rs).
// `synthetic` is written by scripts/make-synthetic.mjs for the stress session,
// which is generated in JavaScript and never round-trips through the reader.
export type SessionLabel = "recorded" | "fixture" | "synthetic";

export type IncidentKind =
  | "test_weakened"
  | "test_regression"
  | "thrashing"
  | "dangerous_command"
  | "dependency_added"
  | "secret_touched"
  | "scope_creep";

export type Severity = "info" | "warn" | "critical";

// ── events ──────────────────────────────────────────────────────────────

export interface SessionStartEvent {
  kind: "session_start";
  title: string;
  agent?: string;
  os: string;
  repo_name: string;
}

export interface PromptEvent {
  kind: "prompt";
  id: string;
  text: string;
  source: PromptSource;
}

export interface FileChangeEvent {
  kind: "file_change";
  path: string;
  op: FileOp;
  before_hash?: string;
  after_hash?: string;
  lines_added: number;
  lines_removed: number;
  prompt_id?: string;
}

export interface CommandEvent {
  kind: "command";
  cmd: string;
  exit_code?: number;
  duration_ms?: number;
  source: CommandSource;
  prompt_id?: string;
}

export interface TestRunEvent {
  kind: "test_run";
  cmd: string;
  passed: number;
  failed: number;
  skipped: number;
  total: number;
  ok: boolean;
  prompt_id?: string;
}

export interface NoteEvent {
  kind: "note";
  text: string;
}

export interface SessionEndEvent {
  kind: "session_end";
}

export type EventKind =
  | SessionStartEvent
  | PromptEvent
  | FileChangeEvent
  | CommandEvent
  | TestRunEvent
  | NoteEvent
  | SessionEndEvent;

// ── export ──────────────────────────────────────────────────────────────

export interface SessionMeta {
  id: string;
  title: string;
  agent?: string;
  started_at: string;
  duration_ms: number;
  os: string;
  repo_name: string;
  label: SessionLabel;
}

export interface PromptSummary {
  id: string;
  text: string;
  step: number;
  t_ms: number;
  files_touched: number;
  commands_run: number;
  incidents: number;
}

export interface DiffLine {
  tag: DiffTag;
  text: string;
}

export interface DiffHunk {
  old_start: number;
  new_start: number;
  lines: DiffLine[];
}

export interface ExportStep {
  seq: number;
  t_ms: number;
  kind: EventKind["kind"];
  hunks?: DiffHunk[];
  // Kind-specific fields are read through the narrow helpers in layout.ts
  // rather than duplicated here; the parser keeps them on the object.
  [key: string]: unknown;
}

export interface Incident {
  id: string;
  kind: IncidentKind;
  severity: Severity;
  /** Index into `steps`. Not a `seq`. */
  step: number;
  t_ms: number;
  title: string;
  evidence: string;
  why_it_matters: string;
  prompt_id?: string;
  path?: string;
}

export interface FileSummary {
  churn: number;
  prompts: string[];
  ops: Record<string, number>;
}

export interface SessionStats {
  steps: number;
  files_touched: number;
  commands: number;
  test_runs: number;
  incidents_by_kind: Record<string, number>;
  tests_first_ok?: boolean;
  tests_last_ok?: boolean;
}

export interface Session {
  version: number;
  meta: SessionMeta;
  prompts: PromptSummary[];
  steps: ExportStep[];
  incidents: Incident[];
  files: Record<string, FileSummary>;
  stats: SessionStats;
}

// ── narrow readers ──────────────────────────────────────────────────────
// `ExportStep` keeps unknown kind-specific fields, so consumers narrow with
// these instead of casting. They return undefined rather than throwing.

export function asFileChange(step: ExportStep): FileChangeEvent | undefined {
  return step.kind === "file_change" ? (step as unknown as FileChangeEvent) : undefined;
}

export function asTestRun(step: ExportStep): TestRunEvent | undefined {
  return step.kind === "test_run" ? (step as unknown as TestRunEvent) : undefined;
}

export function asCommand(step: ExportStep): CommandEvent | undefined {
  return step.kind === "command" ? (step as unknown as CommandEvent) : undefined;
}

export function asPrompt(step: ExportStep): PromptEvent | undefined {
  return step.kind === "prompt" ? (step as unknown as PromptEvent) : undefined;
}

export function asNote(step: ExportStep): NoteEvent | undefined {
  return step.kind === "note" ? (step as unknown as NoteEvent) : undefined;
}
