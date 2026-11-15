//! The fixtures under `fixtures/` are what the website renders, so they are
//! checked against the schema on every test run. If the exporter changes shape,
//! this fails here rather than in the browser.

use flightstrip_core::schema::{IncidentKind, SessionExport, SessionLabel};
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn load(name: &str) -> SessionExport {
    let path = fixtures_dir().join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    SessionExport::parse(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures_dir())
        .expect("fixtures directory exists")
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".json"))
        .collect();
    names.sort();
    names
}

#[test]
fn every_fixture_matches_the_schema() {
    let names = fixture_names();
    assert!(
        names.len() >= 3,
        "expected the three demo fixtures: {names:?}"
    );
    for name in names {
        let session = load(&name);
        assert_eq!(session.version, 1, "{name}");
        assert_eq!(session.meta.label, SessionLabel::Fixture, "{name}");
        assert!(!session.steps.is_empty(), "{name} has no steps");
        assert!(!session.prompts.is_empty(), "{name} has no prompts");
        assert_eq!(
            session.stats.steps,
            session.steps.len() as u64,
            "{name}: stats disagree with steps"
        );
        for incident in &session.incidents {
            assert!(
                (incident.step as usize) < session.steps.len(),
                "{name}: incident points at a missing step"
            );
            assert!(!incident.why_it_matters.is_empty(), "{name}");
            assert!(!incident.evidence.is_empty(), "{name}");
        }
        assert!(
            session.steps.iter().any(|step| step
                .hunks
                .as_ref()
                .map(|hunks| !hunks.is_empty())
                .unwrap_or(false)),
            "{name}: no step carries a diff, the drawer would be empty"
        );
    }
}

#[test]
fn the_clean_fixture_has_no_incidents() {
    let session = load("clean.json");
    assert!(session.incidents.is_empty(), "{:#?}", session.incidents);
    assert_eq!(session.stats.tests_first_ok, Some(true));
    assert_eq!(session.stats.tests_last_ok, Some(true));
}

#[test]
fn the_weakened_fixture_shows_a_regression_and_lost_assertions() {
    let session = load("weakened-tests.json");
    let kinds: Vec<IncidentKind> = session.incidents.iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&IncidentKind::TestRegression), "{kinds:?}");
    assert!(kinds.contains(&IncidentKind::TestWeakened), "{kinds:?}");
    assert_eq!(session.stats.tests_first_ok, Some(true));
    assert_eq!(session.stats.tests_last_ok, Some(true));
}

#[test]
fn the_risky_fixture_covers_the_other_detectors() {
    let session = load("risky-session.json");
    let kinds: Vec<IncidentKind> = session.incidents.iter().map(|i| i.kind).collect();
    for expected in [
        IncidentKind::DangerousCommand,
        IncidentKind::SecretTouched,
        IncidentKind::DependencyAdded,
        IncidentKind::Thrashing,
    ] {
        assert!(
            kinds.contains(&expected),
            "missing {expected:?} in {kinds:?}"
        );
    }
    assert!(
        session.stats.incidents_by_kind.len() >= 4,
        "{:?}",
        session.stats.incidents_by_kind
    );
}
