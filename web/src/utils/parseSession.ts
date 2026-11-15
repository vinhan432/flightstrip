// Validate an unknown value against the session wire format.
//
// The Rust reader (`schema.rs`) reports the first missing or wrong-typed
// field, and this mirrors that: it returns the dotted path of the first
// problem rather than a generic "invalid". The Drop Zone shows that path,
// so a person knows which field to look at.

import type { Session } from "../types";

export type ParseResult =
  | { ok: true; session: Session }
  | { ok: false; error: string };

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function isString(v: unknown): v is string {
  return typeof v === "string";
}

function isNumber(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v);
}

function isArray(v: unknown): v is unknown[] {
  return Array.isArray(v);
}

/** Returns the first bad field path, or null when `value` satisfies `fields`. */
function checkFields(
  value: Record<string, unknown>,
  path: string,
  fields: Record<string, (v: unknown) => boolean>,
): string | null {
  for (const [key, test] of Object.entries(fields)) {
    if (!(key in value) || value[key] === null || value[key] === undefined) {
      return `${path}.${key}`;
    }
    if (!test(value[key])) return `${path}.${key}`;
  }
  return null;
}

export function parseSession(value: unknown): ParseResult {
  if (!isObject(value)) {
    return { ok: false, error: "root: expected a JSON object" };
  }

  if (!("version" in value) || !isNumber(value.version)) {
    return { ok: false, error: "version" };
  }
  if (!("meta" in value) || !isObject(value.meta)) {
    return { ok: false, error: "meta" };
  }

  // Descend into meta before checking the later top-level keys: the reader
  // reports the first bad field in schema order, not the first missing key.
  const meta = value.meta as Record<string, unknown>;
  const metaError = checkFields(meta, "meta", {
    id: isString,
    title: isString,
    started_at: isString,
    duration_ms: isNumber,
    os: isString,
    repo_name: isString,
    label: isString,
  });
  if (metaError) return { ok: false, error: metaError };

  const topError = checkFields(value, "root", {
    prompts: isArray,
    steps: isArray,
    incidents: isArray,
    files: isObject,
    stats: isObject,
  });
  if (topError) return { ok: false, error: topError };

  // Every step needs a seq, a time and a kind. Kind-specific fields are
  // deliberately not deep-validated: the Rust reader tolerates a torn or
  // partial event, and so does this.
  const steps = value.steps as unknown[];
  for (let i = 0; i < steps.length; i++) {
    const step = steps[i];
    if (!isObject(step)) return { ok: false, error: `steps[${i}]` };
    const stepError = checkFields(step, `steps[${i}]`, {
      seq: isNumber,
      t_ms: isNumber,
      kind: isString,
    });
    if (stepError) return { ok: false, error: stepError };
  }

  const prompts = value.prompts as unknown[];
  for (let i = 0; i < prompts.length; i++) {
    const prompt = prompts[i];
    if (!isObject(prompt)) return { ok: false, error: `prompts[${i}]` };
    const promptError = checkFields(prompt, `prompts[${i}]`, {
      id: isString,
      text: isString,
      step: isNumber,
      t_ms: isNumber,
    });
    if (promptError) return { ok: false, error: promptError };
  }

  const incidents = value.incidents as unknown[];
  for (let i = 0; i < incidents.length; i++) {
    const incident = incidents[i];
    if (!isObject(incident)) return { ok: false, error: `incidents[${i}]` };
    const incidentError = checkFields(incident, `incidents[${i}]`, {
      id: isString,
      kind: isString,
      severity: isString,
      step: isNumber,
      title: isString,
    });
    if (incidentError) return { ok: false, error: incidentError };
  }

  return { ok: true, session: value as unknown as Session };
}
