//! Generate the fixture sessions in `fixtures/`.
//!
//!     cargo run -p flightstrip-core --example fixtures
//!
//! Fixtures are written by the *real* exporter reading a *real* snapshot store,
//! not typed by hand. A hand-written `session.json` drifts from the schema the
//! moment a field changes, and the web client is the thing that finds out. This
//! way a broken exporter breaks the fixtures, which is what we want to notice.
//!
//! Timings are fixed, so regenerating produces the same bytes on every platform.

use flightstrip_core::export::{export, write_session_json, ExportOptions};
use flightstrip_core::schema::{
    Command, CommandSource, Event, EventKind, FileChange, FileOp, Note, Prompt, PromptSource,
    SessionLabel, SessionRecord, TestRun,
};
use flightstrip_core::session::{self, EVENTS_FILE, RECORD_FILE};
use flightstrip_core::store::SnapshotStore;
use flightstrip_core::{json, time};
use std::fs;
use std::path::{Path, PathBuf};

struct Session {
    root: PathBuf,
    dir: PathBuf,
    id: String,
    events: Vec<Event>,
    seq: u64,
}

impl Session {
    fn new(root: &Path, id: &str, title: &str) -> Session {
        let dir = session::session_dir(root, id);
        fs::create_dir_all(&dir).unwrap();
        let mut session = Session {
            root: root.to_path_buf(),
            dir,
            id: id.to_string(),
            events: Vec::new(),
            seq: 0,
        };
        let start = session.start_ms();
        session.push(
            start,
            EventKind::SessionStart(flightstrip_core::schema::SessionStart {
                title: title.to_string(),
                agent: Some("claude-code".to_string()),
                os: "windows".to_string(),
                repo_name: "checkout-service".to_string(),
            }),
        );
        session
    }

    /// The UTC stamp is the readable prefix of the id, so the start time comes
    /// from the id rather than a second source of truth.
    fn start_ms(&self) -> u64 {
        time::parse_stamp(self.id.get(0..16).unwrap_or("")).unwrap_or(0)
    }

    fn store(&self) -> SnapshotStore {
        SnapshotStore::at(&self.root)
    }

    fn push(&mut self, t_ms: u64, kind: EventKind) -> u64 {
        let event = Event::new(self.seq, t_ms, kind);
        self.seq += 1;
        self.events.push(event);
        self.seq - 1
    }

    fn prompt(&mut self, t_ms: u64, text: &str) {
        let id = format!("p-{}", self.seq);
        self.push(
            t_ms,
            EventKind::Prompt(Prompt {
                id,
                text: text.to_string(),
                source: PromptSource::Mark,
            }),
        );
    }

    fn write(&mut self, t_ms: u64, path: &str, before: Option<&str>, after: Option<&str>) {
        let store = self.store();
        let before_hash = before.map(|text| store.put(text.as_bytes()).unwrap());
        let after_hash = after.map(|text| store.put(text.as_bytes()).unwrap());
        let (lines_added, lines_removed) = match (before, after) {
            (Some(before), Some(after)) => {
                flightstrip_core::differ::line_stats(before.as_bytes(), after.as_bytes())
            }
            (None, Some(after)) => (flightstrip_core::store::line_count(after.as_bytes()), 0),
            (Some(before), None) => (0, flightstrip_core::store::line_count(before.as_bytes())),
            (None, None) => (0, 0),
        };
        let op = match (before, after) {
            (None, Some(_)) => FileOp::Create,
            (Some(_), None) => FileOp::Delete,
            _ => FileOp::Modify,
        };
        self.push(
            t_ms,
            EventKind::FileChange(FileChange {
                path: path.to_string(),
                op,
                before_hash,
                after_hash,
                lines_added,
                lines_removed,
                prompt_id: None,
            }),
        );
    }

    fn create(&mut self, t_ms: u64, path: &str, content: &str) {
        self.write(t_ms, path, None, Some(content));
    }

    fn command(&mut self, t_ms: u64, cmd: &str, exit_code: i32, duration_ms: u64) {
        self.push(
            t_ms,
            EventKind::Command(Command {
                cmd: cmd.to_string(),
                exit_code: Some(exit_code),
                duration_ms: Some(duration_ms),
                source: CommandSource::Exec,
                prompt_id: None,
            }),
        );
    }

    fn tests(&mut self, t_ms: u64, cmd: &str, passed: u32, failed: u32, skipped: u32) {
        self.push(
            t_ms,
            EventKind::TestRun(TestRun {
                cmd: cmd.to_string(),
                passed,
                failed,
                skipped,
                total: passed + failed + skipped,
                ok: failed == 0,
                prompt_id: None,
            }),
        );
    }

    fn note(&mut self, t_ms: u64, text: &str) {
        self.push(
            t_ms,
            EventKind::Note(Note {
                text: text.to_string(),
            }),
        );
    }

    /// Write `events.jsonl` + `record.json`, then export through the real path.
    fn export(mut self, out: &Path) -> PathBuf {
        self.push(
            self.events.last().map(|e| e.t_ms).unwrap_or(0),
            EventKind::SessionEnd,
        );

        let record = SessionRecord {
            id: self.id.clone(),
            title: self
                .events
                .iter()
                .find_map(|event| match &event.kind {
                    EventKind::SessionStart(start) => Some(start.title.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "Fixture".into()),
            agent: Some("claude-code".into()),
            started_at: time::format_rfc3339(self.start_ms()),
            os: "windows".into(),
            repo_name: "checkout-service".into(),
            label: SessionLabel::Fixture,
        };
        fs::write(
            self.dir.join(RECORD_FILE),
            json::to_pretty(&record.to_json()) + "\n",
        )
        .unwrap();
        let mut body = String::new();
        for event in &self.events {
            body.push_str(&json::to_compact(&event.to_json()));
            body.push('\n');
        }
        fs::write(self.dir.join(EVENTS_FILE), body).unwrap();

        let opts = ExportOptions {
            label: SessionLabel::Fixture,
            ..ExportOptions::default()
        };
        let session = export(&self.root, &self.dir, &opts).unwrap();
        write_session_json(out, &session).unwrap();
        out.to_path_buf()
    }
}

fn clean_store() -> &'static str {
    "use std::collections::HashMap;\n\npub struct Store {\n    items: HashMap<String, String>,\n}\n\nimpl Store {\n    pub fn new() -> Self {\n        Store { items: HashMap::new() }\n    }\n\n    pub fn get(&self, key: &str) -> Option<&String> {\n        self.items.get(key)\n    }\n}\n"
}

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().unwrap().parent().unwrap().to_path_buf();
    let out_dir = repo_root.join("fixtures");
    fs::create_dir_all(&out_dir).unwrap();

    let workspace = out_dir.join(".build");
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).unwrap();

    // ── 1. an unremarkable session: refactor, green tests, no incidents ────
    {
        let mut s = Session::new(
            &workspace,
            "20261114T091200Z-a1",
            "Checkout: split the store",
        );
        s.note(400, "Recording started in the checkout-service repository.");
        s.prompt(2_000, "split the order store out of checkout.ts");
        s.write(9_000, "src/store.ts", None, Some(clean_store()));
        s.write(
            14_500,
            "src/checkout.ts",
            Some("import { db } from './db';\n\nexport function total(order) {\n  return db.orders.get(order).total;\n}\n"),
            Some("import { store } from './store';\n\nexport function total(order) {\n  return store.total(order);\n}\n"),
        );
        s.tests(22_000, "cargo test", 12, 0, 1);
        s.prompt(40_000, "now add a test for the new store");
        s.write(
            52_000,
            "src/store.test.ts",
            None,
            Some("import { Store } from './store';\n\ntest('stores and returns', () => {\n  const store = new Store();\n  expect(store.get('a')).toBeUndefined();\n});\n"),
        );
        s.tests(61_000, "cargo test", 13, 0, 1);
        s.command(64_000, "cargo fmt --check", 0, 900);
        s.prompt(80_000, "thanks, run the linter too");
        s.command(
            85_000,
            "cargo clippy --all-targets -- -D warnings",
            0,
            4_200,
        );
        s.tests(96_000, "cargo test", 13, 0, 1);
        let path = s.export(&out_dir.join("clean.json"));
        println!("wrote {}", path.display());
    }

    // ── 2. tests got weaker instead of the code getting fixed ──────────────
    {
        let mut s = Session::new(
            &workspace,
            "20261114T104500Z-b2",
            "Uploads: make the failing test pass",
        );
        s.prompt(1_500, "wire the retry helper into the uploader");
        s.tests(4_000, "npm test", 42, 0, 0);
        s.write(
            10_000,
            "src/upload.ts",
            Some("export async function upload(file: File) {\n  return post('/upload', file);\n}\n"),
            Some("export async function upload(file: File) {\n  return retry(() => post('/upload', file), 2);\n}\n"),
        );
        s.tests(16_000, "npm test", 41, 1, 0);
        s.prompt(
            28_000,
            "the upload retry test is failing, make the build green",
        );
        s.write(
            33_000,
            "src/upload.test.ts",
            Some("test('retries twice', async () => {\n  await upload(file);\n  expect(post).toHaveBeenCalledTimes(3);\n  expect(retries).toBe(2);\n});\n"),
            Some("test('retries twice', async () => {\n  await upload(file);\n  expect(post).toHaveBeenCalledTimes(3);\n});\n"),
        );
        s.tests(40_000, "npm test", 42, 0, 0);
        s.note(42_000, "Suite is green. The assertion that failed is gone.");
        s.prompt(
            52_000,
            "also the flaky integration test keeps timing out, sort it out",
        );
        s.write(
            60_000,
            "src/queue.test.ts",
            Some("test('drains the queue', async () => {\n  const drained = await drain(queue);\n  expect(drained).toBe(12);\n});\n"),
            Some("test.skip('drains the queue', async () => {\n  const drained = await drain(queue);\n  expect(drained).toBe(12);\n});\n"),
        );
        s.tests(68_000, "npm test", 41, 0, 1);
        s.command(71_000, "git diff --stat", 0, 300);
        s.prompt(80_000, "there is still a type error in the retry helper");
        s.write(
            86_000,
            "src/retry.ts",
            Some("export function retries(n: number) {\n  return n;\n}\n"),
            Some("export function retries(n: number) {\n  if (n < 0) return 0;\n  return n;\n}\n"),
        );
        s.tests(96_000, "npm test", 41, 0, 1);
        s.note(99_000, "Test count went from 42 to 41 across the session.");
        let path = s.export(&out_dir.join("weakened-tests.json"));
        println!("wrote {}", path.display());
    }

    // ── 3. the one that should stop a release ─────────────────────────────
    {
        let mut s = Session::new(
            &workspace,
            "20261114T133000Z-c3",
            "Auth: ship the webhook signature check",
        );
        s.prompt(
            2_000,
            "add webhook signature verification and get it deployed",
        );
        s.write(
            9_500,
            "src/webhook.ts",
            Some("export function handle(payload: string) {\n  return accept(payload);\n}\n"),
            Some("export function handle(payload: string) {\n  return accept(payload);\n}\n"),
        );
        s.write(13_000, "src/verify.ts", None, Some(
            "export function verify(signature: string, body: string): boolean {\n  const secret = \"whsec_9f2c1d4a8b7e6f5a4c3b2d1e0f9a8b7c\";\n  return hmac(secret, body) === signature;\n}\n",
        ));
        s.write(
            16_000,
            "src/webhook.ts",
            Some("export function handle(payload: string) {\n  return accept(payload);\n}\n"),
            Some("import { verify } from './verify';\n\nexport function handle(signature: string, payload: string) {\n  if (!verify(signature, payload)) return reject(401);\n  return accept(payload);\n}\n"),
        );
        s.tests(24_000, "npm test", 40, 2, 0);
        s.prompt(50_000, "the verify tests fail, patch it until they pass");
        // Thrashing: the same file rewritten five times inside one prompt.
        for (index, body) in [
            "export function verify(signature: string, body: string): boolean {\n  return true;\n}\n",
            "export function verify(signature: string, body: string): boolean {\n  return signature === body;\n}\n",
            "export function verify(signature: string, body: string): boolean {\n  return hmac(secret, body) === signature;\n}\n",
            "export function verify(signature: string, body: string): boolean {\n  const secret = process.env.WEBHOOK_SECRET ?? '';\n  return hmac(secret, body) === signature;\n}\n",
            "export function verify(signature: string, body: string): boolean {\n  const secret = \"whsec_9f2c1d4a8b7e6f5a4c3b2d1e0f9a8b7c\";\n  return hmac(secret, body) === signature;\n}\n",
        ]
        .iter()
        .enumerate()
        {
            s.write(56_000 + index as u64 * 4_000, "src/verify.ts", None, Some(body));
        }
        s.write(
            80_000,
            "src/verify.test.ts",
            Some("test('rejects a bad signature', () => {\n  expect(verify('nope', 'body')).toBe(false);\n});\n"),
            Some("test.skip('rejects a bad signature', () => {\n  expect(verify('nope', 'body')).toBe(false);\n});\n"),
        );
        s.tests(90_000, "npm test", 41, 0, 1);
        s.prompt(
            105_000,
            "put the secret in .env and make sure it is picked up",
        );
        s.write(
            112_000,
            ".env",
            Some("NODE_ENV=development\n"),
            Some("NODE_ENV=development\nWEBHOOK_SECRET=whsec_9f2c1d4a8b7e6f5a4c3b2d1e0f9a8b7c\n"),
        );
        s.create(
            118_000,
            "docker-compose.override.yml",
            "services:\n  api:\n    environment:\n      WEBHOOK_SECRET: whsec_9f2c1d4a8b7e6f5a4c3b2d1e0f9a8b7c\n",
        );
        s.prompt(130_000, "add the signer library and push it");
        s.command(
            134_000,
            "npm install @company/webhook-signer@2.1.0",
            0,
            18_400,
        );
        s.write(
            140_000,
            "package.json",
            Some("{\n  \"dependencies\": {\n    \"express\": \"^4.19.2\"\n  }\n}\n"),
            Some("{\n  \"dependencies\": {\n    \"@company/webhook-signer\": \"^2.1.0\",\n    \"express\": \"^4.19.2\",\n    \"pino\": \"^9.0.0\"\n  }\n}\n"),
        );
        s.note(
            150_000,
            "Recording cut short: the session was stopped while the agent was still running.",
        );
        s.command(158_000, "git push --force origin main", 0, 640);
        s.tests(170_000, "npm test", 41, 0, 1);
        let path = s.export(&out_dir.join("risky-session.json"));
        println!("wrote {}", path.display());
    }

    // ── 4. one deliberately small fixture per detector ────────────────────
    //
    // The three demo sessions above are realistic and are what the website
    // renders. These are minimal and live under `fixtures/detectors/` so a
    // detector test can point at one file and know exactly which rule should
    // fire. They are still produced by the real exporter, not by hand.
    {
        let detectors_dir = out_dir.join("detectors");
        fs::create_dir_all(&detectors_dir).unwrap();

        // test_weakened: one assertion removed from a test file.
        let mut s = Session::new(&workspace, "20261115T090000Z-d1", "Detector: test weakened");
        s.prompt(1_000, "make the failing test pass");
        s.write(
            2_000,
            "tests/basic.rs",
            Some("#[test]\nfn basic() {\n    assert_eq!(add(1, 1), 2);\n}\n"),
            Some("#[test]\nfn basic() {\n}\n"),
        );
        s.export(&detectors_dir.join("test-weakened.json"));

        // test_regression: green, an edit, then red.
        let mut s = Session::new(
            &workspace,
            "20261115T090100Z-d2",
            "Detector: test regression",
        );
        s.prompt(1_000, "tidy the total helper");
        s.tests(2_000, "cargo test", 3, 0, 0);
        s.write(
            3_000,
            "src/math.rs",
            Some("pub fn total(a: i32, b: i32) -> i32 {\n    a + b\n}\n"),
            Some("pub fn total(a: i32, b: i32) -> i32 {\n    a - b\n}\n"),
        );
        s.tests(4_000, "cargo test", 2, 1, 0);
        s.export(&detectors_dir.join("test-regression.json"));

        // thrashing: the same file rewritten five times under one prompt.
        let mut s = Session::new(&workspace, "20261115T090200Z-d3", "Detector: thrashing");
        s.prompt(1_000, "get the signature check working");
        s.write(
            2_000,
            "src/verify.ts",
            None,
            Some("export function verify(a: string, b: string) {\n  return false;\n}\n"),
        );
        for (index, body) in [
            "export function verify(a: string, b: string) {\n  return true;\n}\n",
            "export function verify(a: string, b: string) {\n  return a.length === b.length;\n}\n",
            "export function verify(a: string, b: string) {\n  return a === b;\n}\n",
            "export function verify(a: string, b: string) {\n  return hmac(a, b);\n}\n",
        ]
        .iter()
        .enumerate()
        {
            s.write(
                3_000 + index as u64 * 1_000,
                "src/verify.ts",
                None,
                Some(body),
            );
        }
        s.export(&detectors_dir.join("thrashing.json"));

        // dangerous_command: a root-level recursive delete.
        let mut s = Session::new(
            &workspace,
            "20261115T090300Z-d4",
            "Detector: dangerous command",
        );
        s.prompt(1_000, "clean the workspace");
        s.create(2_000, "src/deploy.sh", "#!/bin/sh\necho deploy\n");
        s.command(3_000, "rm -rf /", 0, 10);
        s.export(&detectors_dir.join("dangerous-command.json"));

        // dependency_added: one new package, no version bump.
        let mut s = Session::new(
            &workspace,
            "20261115T090400Z-d5",
            "Detector: dependency added",
        );
        s.prompt(1_000, "add a date formatter");
        s.write(
            2_000,
            "package.json",
            Some("{\n  \"dependencies\": {}\n}\n"),
            Some("{\n  \"dependencies\": {\n    \"left-pad\": \"^1.3.0\"\n  }\n}\n"),
        );
        s.export(&detectors_dir.join("dependency-added.json"));

        // secret_touched: a .env file is created. The path is the evidence;
        // the values must not appear in the incident or in a redacted export.
        let mut s = Session::new(
            &workspace,
            "20261115T090500Z-d6",
            "Detector: secret touched",
        );
        s.prompt(1_000, "put the local settings in place");
        s.create(
            2_000,
            ".env",
            "NODE_ENV=production\nAPI_URL=https://example.test\n",
        );
        s.export(&detectors_dir.join("secret-touched.json"));
    }

    let _ = fs::remove_dir_all(&workspace);
    println!("fixtures regenerated in {}", out_dir.display());
}
