//! End-to-end: real files, real snapshot objects, real exporter, real detectors.
//!
//! This is the test that would notice if the wire format, the diff pipeline and
//! the detectors stopped agreeing with each other — the three places where a
//! refactor can be locally correct and collectively wrong.

use flightstrip_core::export::{export, ExportOptions};
use flightstrip_core::schema::{
    Command, CommandSource, EventKind, FileChange, FileOp, IncidentKind, PromptSource,
    SessionExport, SessionLabel, TestRun,
};
use flightstrip_core::session::EventLog;
use flightstrip_core::store::SnapshotStore;
use std::fs;
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("fs-e2e-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn a_whole_session_exports_with_diffs_and_incidents() {
    let root = temp_root("session");
    let store = SnapshotStore::at(&root);

    let mut log =
        EventLog::create(&root, "e2e", Some("claude".into()), SessionLabel::Recorded).unwrap();

    // A prompt, then the work it caused.
    log.mark_prompt("add a left-pad dependency", PromptSource::Mark)
        .unwrap();

    let old_test = "test('add', () => {\n  expect(add(1, 2)).toBe(3);\n});\n";
    let new_test = "test('add', () => {\n  expect(add(1, 2)).toBeTruthy();\n});\n";
    let before = store.put(old_test.as_bytes()).unwrap();
    let after = store.put(new_test.as_bytes()).unwrap();
    log.append(EventKind::FileChange(FileChange {
        path: "src/math.test.ts".into(),
        op: FileOp::Modify,
        before_hash: Some(before),
        after_hash: Some(after),
        lines_added: 1,
        lines_removed: 1,
        prompt_id: None,
    }))
    .unwrap();

    let old_manifest = "{\n  \"dependencies\": {}\n}\n";
    let new_manifest =
        "{\n  \"dependencies\": {\n    \"left-pad\": \"^1.3.0\",\n    \"lodash\": \"^4.17.21\"\n  }\n}\n";
    let manifest_before = store.put(old_manifest.as_bytes()).unwrap();
    let manifest_after = store.put(new_manifest.as_bytes()).unwrap();
    log.append(EventKind::FileChange(FileChange {
        path: "package.json".into(),
        op: FileOp::Modify,
        before_hash: Some(manifest_before),
        after_hash: Some(manifest_after),
        lines_added: 4,
        lines_removed: 1,
        prompt_id: None,
    }))
    .unwrap();

    log.append(EventKind::Command(Command {
        cmd: "npm install left-pad".into(),
        exit_code: Some(0),
        duration_ms: Some(1200),
        source: CommandSource::Exec,
        prompt_id: None,
    }))
    .unwrap();

    // Green, then red: the regression detector needs both ends.
    log.append(EventKind::TestRun(TestRun {
        cmd: "npm test".into(),
        passed: 3,
        failed: 0,
        skipped: 0,
        total: 3,
        ok: true,
        prompt_id: None,
    }))
    .unwrap();
    log.append(EventKind::TestRun(TestRun {
        cmd: "npm test".into(),
        passed: 2,
        failed: 1,
        skipped: 0,
        total: 3,
        ok: false,
        prompt_id: None,
    }))
    .unwrap();

    log.append(EventKind::Command(Command {
        cmd: "git push --force origin main".into(),
        exit_code: Some(0),
        duration_ms: Some(400),
        source: CommandSource::Exec,
        prompt_id: None,
    }))
    .unwrap();
    log.close().unwrap();

    let dir = log.dir().to_path_buf();
    let mut out = export(&root, &dir, &ExportOptions::default()).unwrap();

    // ── incidents ────────────────────────────────────────────────────────
    let kinds: Vec<IncidentKind> = out.incidents.iter().map(|i| i.kind).collect();
    for expected in [
        IncidentKind::TestWeakened,
        IncidentKind::DependencyAdded,
        IncidentKind::TestRegression,
        IncidentKind::DangerousCommand,
    ] {
        assert!(
            kinds.contains(&expected),
            "missing {expected:?} in {kinds:?}"
        );
    }

    let weakened = out
        .incidents
        .iter()
        .find(|i| i.kind == IncidentKind::TestWeakened)
        .unwrap();
    assert!(
        weakened.evidence.contains("toBe(3)"),
        "evidence should quote the removed assertion: {}",
        weakened.evidence
    );

    let deps = out
        .incidents
        .iter()
        .find(|i| i.kind == IncidentKind::DependencyAdded)
        .unwrap();
    assert!(deps.evidence.contains("left-pad"), "{}", deps.evidence);
    assert_eq!(out.incidents.len(), 4, "{:#?}", out.incidents);

    // ── attribution and hunks ────────────────────────────────────────────
    let with_hunks: Vec<_> = out
        .steps
        .iter()
        .filter(|step| step.hunks.as_ref().map(|h| !h.is_empty()).unwrap_or(false))
        .collect();
    assert_eq!(with_hunks.len(), 2, "both file changes should carry hunks");
    assert!(out
        .incidents
        .iter()
        .filter(|i| i.kind == IncidentKind::TestWeakened)
        .all(|i| i.prompt_id.as_deref() == Some("p-1")));

    // ── stats ────────────────────────────────────────────────────────────
    assert_eq!(out.stats.files_touched, 2);
    assert_eq!(out.stats.commands, 2);
    assert_eq!(out.stats.test_runs, 2);
    assert_eq!(out.stats.tests_first_ok, Some(true));
    assert_eq!(out.stats.tests_last_ok, Some(false));
    assert_eq!(out.files.len(), 2);

    // ── the wire format round-trips ──────────────────────────────────────
    let text = out.to_pretty_string();
    let back = SessionExport::parse(&text).unwrap();
    assert_eq!(back, out);

    // ── redaction keeps the finding, hides the value ─────────────────────
    out.steps.push(flightstrip_core::schema::ExportStep {
        seq: 999,
        t_ms: 0,
        kind: EventKind::Note(flightstrip_core::schema::Note {
            text: "api_key = \"abcdef1234567890abcdef\"".into(),
        }),
        hunks: None,
    });
    let mut redacted = out.clone();
    flightstrip_core::redact::redact_export(&mut redacted);
    let last = redacted.steps.last().unwrap();
    let EventKind::Note(note) = &last.kind else {
        panic!("expected a note");
    };
    assert!(
        !note.text.contains("abcdef1234567890abcdef"),
        "{}",
        note.text
    );

    let _ = fs::remove_dir_all(&root);
}
