// Contract check: the Rust reader and the website's parser must agree on every
// session file, one verdict per file.
//
//   flightstrip verify <file>      -> exit 0 / 1
//   parseSession(value)            -> { ok: true } / { ok: false, error }
//
// A disagreement is a P1: either the site renders a session the CLI rejects, or
// the site refuses one the CLI accepts.
//
// Usage:
//   node scripts/validate-sessions.mjs                     # fixtures + staging + shipped
//   node scripts/validate-sessions.mjs path/to/session.json

import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync, existsSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { resolve, dirname, join, basename } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..");
const webRoot = resolve(repoRoot, "web");
const exe = process.env.FS_EXE || join(repoRoot, "target", "release", process.platform === "win32" ? "flightstrip.exe" : "flightstrip");

const webRequire = createRequire(join(webRoot, "package.json"));
const esbuild = webRequire("esbuild");

// The parser is TypeScript; bundle it once and import the result.
async function loadParser() {
  const built = await esbuild.build({
    entryPoints: [join(webRoot, "src", "utils", "parseSession.ts")],
    bundle: true,
    format: "esm",
    write: false,
    platform: "neutral",
  });
  const tempDir = join(repoRoot, ".cache");
  mkdirSync(tempDir, { recursive: true });
  const modulePath = join(tempDir, `parse-session-${process.pid}.mjs`);
  writeFileSync(modulePath, built.outputFiles[0].text);
  const mod = await import(pathToFileURL(modulePath).href);
  rmSync(modulePath, { force: true });
  return mod.parseSession;
}

function collectFiles(argv) {
  if (argv.length > 0) return argv.map((p) => resolve(p));
  const files = [];
  for (const dir of [
    join(repoRoot, "fixtures"),
    join(repoRoot, "fixtures", "staged"),
    join(webRoot, "public", "sessions"),
  ]) {
    if (!existsSync(dir)) continue;
    for (const name of readdirSync(dir)) {
      if (name.endsWith(".json") && name !== "index.json") files.push(join(dir, name));
    }
  }
  return files.sort();
}

function cliVerdict(file) {
  const run = spawnSync(exe, ["verify", file], { encoding: "utf8" });
  const text = `${run.stdout ?? ""}${run.stderr ?? ""}`.trim();
  return { ok: run.status === 0, message: text.split(/\r?\n/).filter(Boolean).pop() ?? "" };
}

function webVerdict(parseSession, file) {
  let value;
  try {
    value = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    return { ok: false, message: `not JSON: ${error.message.split("\n")[0]}` };
  }
  const result = parseSession(value);
  return result.ok ? { ok: true, message: "ok" } : { ok: false, message: result.error };
}

const parseSession = await loadParser();
const files = collectFiles(process.argv.slice(2));

if (files.length === 0) {
  console.error("validate-sessions: no session files found");
  process.exit(1);
}

let disagreements = 0;
const rows = [];
for (const file of files) {
  const cli = cliVerdict(file);
  const web = webVerdict(parseSession, file);
  const agree = cli.ok === web.ok;
  if (!agree) disagreements++;
  rows.push({
    file: basename(file),
    dir: file.replace(repoRoot + "\\", "").replace(repoRoot + "/", ""),
    cli: cli.ok ? "ok" : "reject",
    web: web.ok ? "ok" : "reject",
    agree,
    cliMessage: cli.ok ? "" : cli.message,
    webMessage: web.ok ? "" : web.message,
  });
}

const width = Math.max(...rows.map((r) => r.file.length), 4);
console.log("");
console.log(`${"file".padEnd(width)}  cli     web     agree`);
console.log(`${"-".repeat(width)}  ------  ------  -----`);
for (const row of rows) {
  console.log(
    `${row.file.padEnd(width)}  ${row.cli.padEnd(6)}  ${row.web.padEnd(6)}  ${row.agree ? "yes" : "NO"}`,
  );
  if (!row.agree) {
    console.log(`    cli: ${row.cliMessage}`);
    console.log(`    web: ${row.webMessage}`);
  }
}

console.log("");
console.log(`${rows.length} file(s), ${rows.length - disagreements} agree, ${disagreements} disagree`);
process.exit(disagreements === 0 ? 0 : 1);
