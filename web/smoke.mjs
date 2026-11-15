// Build-artifact check.
//
// The real rendering check is `src/app.integration.test.tsx`, which mounts
// the whole <App/> against the real fixtures. This script checks the thing
// that test cannot: that `npm run build` produced a deployable `dist/` with
// the session data in it and no accidental external asset references.
//
// (An earlier version tried to run the built page in jsdom. jsdom does not
// execute `<script type="module">`, so the page never ran there and every
// check failed for the wrong reason. The integration test covers that ground.)
//
// Usage: node smoke.mjs   (expects `npm run build` to have run first)

import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const dist = resolve(here, "dist");

const failures = [];
function check(name, condition, detail = "") {
  if (condition) {
    console.log(`  ok   ${name}`);
  } else {
    console.log(`  FAIL ${name}${detail ? ` — ${detail}` : ""}`);
    failures.push(name);
  }
}

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

if (!existsSync(dist)) {
  console.error("smoke: dist/ is missing — run `npm run build` first");
  process.exit(1);
}

const files = walk(dist);
const relative = files.map((f) => f.slice(dist.length + 1).replace(/\\/g, "/"));

// ── the shell ────────────────────────────────────────────────────────────
check("index.html exists", relative.includes("index.html"));

const html = readFileSync(join(dist, "index.html"), "utf8");
check("the shell references a module bundle", /<script[^>]+type="module"/.test(html));
check("the shell has a root element", /id="root"/.test(html));
check("the shell has a title", /<title>[^<]+<\/title>/.test(html));

// ── the session data ─────────────────────────────────────────────────────
const sessions = relative.filter((f) => f.startsWith("sessions/"));
check("session data was copied into the build", sessions.length >= 4, `${sessions.length} file(s)`);
check("the manifest is present", sessions.includes("sessions/index.json"));
check("the risky fixture is present", sessions.includes("sessions/risky-session.json"));

const manifest = JSON.parse(
  readFileSync(join(dist, "sessions", "index.json"), "utf8"),
);
check("the manifest lists three sessions", manifest.length === 3, `${manifest.length}`);

for (const entry of manifest) {
  check(`manifest entry "${entry.id}" has a file`, sessions.includes(`sessions/${entry.file}`));
}

// ── the assets ───────────────────────────────────────────────────────────
const js = relative.filter((f) => f.endsWith(".js"));
const css = relative.filter((f) => f.endsWith(".css"));
check("a JS bundle was emitted", js.length > 0);
check("a CSS bundle was emitted", css.length > 0);

// Fonts are self-hosted: the build must not reference a font CDN.
const cssText = css.map((f) => readFileSync(join(dist, f), "utf8")).join("\n");
check(
  "fonts are self-hosted, not fetched from a CDN",
  !/fonts\.(googleapis|gstatic)\.com/.test(cssText),
);
check("at least one woff2 is bundled", relative.some((f) => f.endsWith(".woff2")));

// ── the budget ───────────────────────────────────────────────────────────
// The spec's budget is <200KB gzipped, which is what a browser downloads.
import { gzipSync } from "node:zlib";

const jsBytes = js.reduce((sum, f) => sum + statSync(join(dist, f)).size, 0);
const jsGzip = js.reduce(
  (sum, f) => sum + gzipSync(readFileSync(join(dist, f))).length,
  0,
);
const jsKb = Math.round(jsBytes / 1024);
const gzipKb = Math.round(jsGzip / 1024);
check(`JS is under the 200KB gzipped budget`, gzipKb < 200, `${gzipKb} KB gzip`);

console.log("");
console.log(
  `smoke: ${relative.length} files in dist/, ${jsKb} KB JS raw / ${gzipKb} KB gzip`,
);

if (failures.length > 0) {
  console.error(`smoke: ${failures.length} check(s) failed`);
  process.exit(1);
}
console.log("smoke: all checks passed");
