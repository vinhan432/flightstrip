// Every file the catalogue points at must satisfy the wire format. This is
// the guard the spec asks for: a hand-edited or truncated session in
// public/sessions/ fails here rather than in the browser.

import { describe, it, expect } from "vitest";
import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { parseSession } from "./parseSession";

const dir = resolve(__dirname, "../../public/sessions");

interface Entry {
  id: string;
  file: string;
}

const catalogue: Entry[] = JSON.parse(
  readFileSync(resolve(dir, "index.json"), "utf8"),
);

describe("public/sessions", () => {
  it("lists at least one session", () => {
    expect(catalogue.length).toBeGreaterThan(0);
  });

  for (const entry of catalogue) {
    it(`validates ${entry.file}`, () => {
      const path = resolve(dir, entry.file);
      expect(existsSync(path)).toBe(true);

      const parsed = parseSession(JSON.parse(readFileSync(path, "utf8")));
      if (!parsed.ok) throw new Error(`${entry.file}: ${parsed.error}`);
      expect(parsed.ok).toBe(true);
    });
  }

  // The synthetic stress fixture is generated, not committed. Validate it
  // when present so a broken generator is caught, but do not require it.
  const synthetic = resolve(dir, "synthetic.json");
  it.skipIf(!existsSync(synthetic))("validates the synthetic fixture", () => {
    const parsed = parseSession(JSON.parse(readFileSync(synthetic, "utf8")));
    if (!parsed.ok) throw new Error(`synthetic.json: ${parsed.error}`);
    expect(parsed.ok).toBe(true);
  });
});
