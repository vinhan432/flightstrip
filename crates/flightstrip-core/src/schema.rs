//! Wire format. Every field here appears verbatim in `session.json`, and the
//! TypeScript types in `web/src/types.ts` mirror it exactly.
//!
//! Encoding and decoding are hand-written against [`crate::json`]. It is more
//! lines than `derive(Serialize, Deserialize)` and it is worth it: the shape is
//! the product's contract, and this keeps the whole recorder dependency-free.

use crate::json::{self, Json};
use crate::{Error, Result};
use std::collections::BTreeMap;

/// Bumped only on a breaking change to the export shape.
pub const SCHEMA_VERSION: u8 = 1;

// ── field helpers ─────────────────────────────────────────────────────────

fn missing(key: &str) -> Error {
    Error::Json(format!("missing field `{key}`"))
}

fn wrong_type(key: &str) -> Error {
    Error::Json(format!("field `{key}` has the wrong type"))
}

fn field<'a>(value: &'a Json, key: &str) -> Result<&'a Json> {
    value.get(key).ok_or_else(|| missing(key))
}

fn text_field(value: &Json, key: &str) -> Result<String> {
    field(value, key)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| wrong_type(key))
}

fn opt_text_field(value: &Json, key: &str) -> Option<String> {
    value.get(key).and_then(Json::as_str).map(str::to_string)
}

fn uint_field(value: &Json, key: &str) -> Result<u64> {
    field(value, key)?.as_u64().ok_or_else(|| wrong_type(key))
}

fn opt_uint_field(value: &Json, key: &str) -> Option<u64> {
    value.get(key).and_then(Json::as_u64)
}

fn opt_bool_field(value: &Json, key: &str) -> Option<bool> {
    value.get(key).and_then(Json::as_bool)
}

fn array_field<'a>(value: &'a Json, key: &str) -> Result<&'a [Json]> {
    field(value, key)?.as_array().ok_or_else(|| wrong_type(key))
}

fn put(fields: &mut Vec<(String, Json)>, key: &str, value: Json) {
    fields.push((key.to_string(), value));
}

fn wrap(fields: Vec<(String, Json)>) -> Json {
    Json::Obj(fields)
}

// ── Events, as stored one-per-line in events.jsonl ─────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub v: u8,
    /// Monotonic, starts at 0.
    pub seq: u64,
    /// Milliseconds since the session started.
    pub t_ms: u64,
    pub kind: EventKind,
}

impl Event {
    pub fn new(seq: u64, t_ms: u64, kind: EventKind) -> Self {
        Event {
            v: SCHEMA_VERSION,
            seq,
            t_ms,
            kind,
        }
    }

    pub fn to_json(&self) -> Json {
        let mut fields = Vec::new();
        put(&mut fields, "v", Json::from(self.v));
        put(&mut fields, "seq", Json::from(self.seq));
        put(&mut fields, "t_ms", Json::from(self.t_ms));
        fields.extend(self.kind.fields());
        wrap(fields)
    }

    pub fn from_json(value: &Json) -> Result<Self> {
        Ok(Event {
            v: uint_field(value, "v").unwrap_or(u64::from(SCHEMA_VERSION)) as u8,
            seq: uint_field(value, "seq")?,
            t_ms: uint_field(value, "t_ms")?,
            kind: EventKind::from_json(value)?,
        })
    }

    /// The prompt this event is attributed to, if any.
    pub fn prompt_id(&self) -> Option<&str> {
        match &self.kind {
            EventKind::FileChange(change) => change.prompt_id.as_deref(),
            EventKind::Command(command) => command.prompt_id.as_deref(),
            EventKind::TestRun(run) => run.prompt_id.as_deref(),
            _ => None,
        }
    }

    pub fn set_prompt_id(&mut self, id: Option<String>) {
        match &mut self.kind {
            EventKind::FileChange(change) => change.prompt_id = id,
            EventKind::Command(command) => command.prompt_id = id,
            EventKind::TestRun(run) => run.prompt_id = id,
            _ => {}
        }
    }

    /// Events that carry attribution at all.
    pub fn is_attributable(&self) -> bool {
        matches!(
            self.kind,
            EventKind::FileChange(_) | EventKind::Command(_) | EventKind::TestRun(_)
        )
    }

    pub fn kind_name(&self) -> &'static str {
        self.kind.name()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    SessionStart(SessionStart),
    Prompt(Prompt),
    FileChange(FileChange),
    Command(Command),
    TestRun(TestRun),
    Note(Note),
    SessionEnd,
}

impl EventKind {
    pub fn name(&self) -> &'static str {
        match self {
            EventKind::SessionStart(_) => "session_start",
            EventKind::Prompt(_) => "prompt",
            EventKind::FileChange(_) => "file_change",
            EventKind::Command(_) => "command",
            EventKind::TestRun(_) => "test_run",
            EventKind::Note(_) => "note",
            EventKind::SessionEnd => "session_end",
        }
    }

    fn fields(&self) -> Vec<(String, Json)> {
        let mut fields = Vec::new();
        put(&mut fields, "kind", Json::from(self.name()));
        match self {
            EventKind::SessionStart(start) => {
                put(&mut fields, "title", Json::from(&start.title));
                put(&mut fields, "agent", Json::from(start.agent.clone()));
                put(&mut fields, "os", Json::from(&start.os));
                put(&mut fields, "repo_name", Json::from(&start.repo_name));
            }
            EventKind::Prompt(prompt) => {
                put(&mut fields, "id", Json::from(&prompt.id));
                put(&mut fields, "text", Json::from(&prompt.text));
                put(&mut fields, "source", Json::from(prompt.source.as_str()));
            }
            EventKind::FileChange(change) => {
                put(&mut fields, "path", Json::from(&change.path));
                put(&mut fields, "op", Json::from(change.op.as_str()));
                put(
                    &mut fields,
                    "before_hash",
                    Json::from(change.before_hash.clone()),
                );
                put(
                    &mut fields,
                    "after_hash",
                    Json::from(change.after_hash.clone()),
                );
                put(&mut fields, "lines_added", Json::from(change.lines_added));
                put(
                    &mut fields,
                    "lines_removed",
                    Json::from(change.lines_removed),
                );
                put(
                    &mut fields,
                    "prompt_id",
                    Json::from(change.prompt_id.clone()),
                );
            }
            EventKind::Command(command) => {
                put(&mut fields, "cmd", Json::from(&command.cmd));
                put(&mut fields, "exit_code", Json::from(command.exit_code));
                put(&mut fields, "duration_ms", Json::from(command.duration_ms));
                put(&mut fields, "source", Json::from(command.source.as_str()));
                put(
                    &mut fields,
                    "prompt_id",
                    Json::from(command.prompt_id.clone()),
                );
            }
            EventKind::TestRun(run) => {
                put(&mut fields, "cmd", Json::from(&run.cmd));
                put(&mut fields, "passed", Json::from(run.passed));
                put(&mut fields, "failed", Json::from(run.failed));
                put(&mut fields, "skipped", Json::from(run.skipped));
                put(&mut fields, "total", Json::from(run.total));
                put(&mut fields, "ok", Json::from(run.ok));
                put(&mut fields, "prompt_id", Json::from(run.prompt_id.clone()));
            }
            EventKind::Note(note) => {
                put(&mut fields, "text", Json::from(&note.text));
            }
            EventKind::SessionEnd => {}
        }
        fields
    }

    pub fn from_json(value: &Json) -> Result<Self> {
        let kind = text_field(value, "kind")?;
        Ok(match kind.as_str() {
            "session_start" => EventKind::SessionStart(SessionStart {
                title: text_field(value, "title")?,
                agent: opt_text_field(value, "agent"),
                os: text_field(value, "os").unwrap_or_else(|_| "unknown".into()),
                repo_name: opt_text_field(value, "repo_name").unwrap_or_default(),
            }),
            "prompt" => EventKind::Prompt(Prompt {
                id: text_field(value, "id")?,
                text: text_field(value, "text")?,
                source: PromptSource::from_str(opt_text_field(value, "source").as_deref()),
            }),
            "file_change" => EventKind::FileChange(FileChange {
                path: text_field(value, "path")?,
                op: FileOp::from_str(opt_text_field(value, "op").as_deref()),
                before_hash: opt_text_field(value, "before_hash"),
                after_hash: opt_text_field(value, "after_hash"),
                lines_added: opt_uint_field(value, "lines_added").unwrap_or(0) as u32,
                lines_removed: opt_uint_field(value, "lines_removed").unwrap_or(0) as u32,
                prompt_id: opt_text_field(value, "prompt_id"),
            }),
            "command" => EventKind::Command(Command {
                cmd: text_field(value, "cmd")?,
                exit_code: value
                    .get("exit_code")
                    .and_then(Json::as_i64)
                    .map(|code| code as i32),
                duration_ms: opt_uint_field(value, "duration_ms"),
                source: CommandSource::from_str(opt_text_field(value, "source").as_deref()),
                prompt_id: opt_text_field(value, "prompt_id"),
            }),
            "test_run" => EventKind::TestRun(TestRun {
                cmd: text_field(value, "cmd")?,
                passed: opt_uint_field(value, "passed").unwrap_or(0) as u32,
                failed: opt_uint_field(value, "failed").unwrap_or(0) as u32,
                skipped: opt_uint_field(value, "skipped").unwrap_or(0) as u32,
                total: opt_uint_field(value, "total").unwrap_or(0) as u32,
                ok: opt_bool_field(value, "ok").unwrap_or(false),
                prompt_id: opt_text_field(value, "prompt_id"),
            }),
            "note" => EventKind::Note(Note {
                text: text_field(value, "text")?,
            }),
            "session_end" => EventKind::SessionEnd,
            other => {
                return Err(Error::Json(format!("unknown event kind `{other}`")));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionStart {
    pub title: String,
    pub agent: Option<String>,
    pub os: String,
    pub repo_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    pub id: String,
    pub text: String,
    pub source: PromptSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileChange {
    /// Forward-slash, repo-relative.
    pub path: String,
    pub op: FileOp,
    /// Content hash, `None` for create or when content was not stored.
    pub before_hash: Option<String>,
    /// Content hash, `None` for delete or when content was not stored.
    pub after_hash: Option<String>,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub prompt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub cmd: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub source: CommandSource,
    pub prompt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestRun {
    pub cmd: String,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub total: u32,
    pub ok: bool,
    pub prompt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptSource {
    Mark,
    Pty,
    Import,
}

impl PromptSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptSource::Mark => "mark",
            PromptSource::Pty => "pty",
            PromptSource::Import => "import",
        }
    }

    fn from_str(text: Option<&str>) -> Self {
        match text {
            Some("pty") => PromptSource::Pty,
            Some("import") => PromptSource::Import,
            _ => PromptSource::Mark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOp {
    Create,
    Modify,
    Delete,
    Rename,
}

impl FileOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileOp::Create => "create",
            FileOp::Modify => "modify",
            FileOp::Delete => "delete",
            FileOp::Rename => "rename",
        }
    }

    fn from_str(text: Option<&str>) -> Self {
        match text {
            Some("create") => FileOp::Create,
            Some("delete") => FileOp::Delete,
            Some("rename") => FileOp::Rename,
            _ => FileOp::Modify,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSource {
    Exec,
    Pty,
    Import,
}

impl CommandSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommandSource::Exec => "exec",
            CommandSource::Pty => "pty",
            CommandSource::Import => "import",
        }
    }

    fn from_str(text: Option<&str>) -> Self {
        match text {
            Some("pty") => CommandSource::Pty,
            Some("import") => CommandSource::Import,
            _ => CommandSource::Exec,
        }
    }
}

// ── Export, as written to session.json ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct SessionExport {
    pub version: u8,
    pub meta: SessionMeta,
    pub prompts: Vec<PromptSummary>,
    pub steps: Vec<ExportStep>,
    pub incidents: Vec<Incident>,
    /// Sorted, so the same recording always produces the same bytes.
    pub files: BTreeMap<String, FileSummary>,
    pub stats: SessionStats,
}

impl SessionExport {
    pub fn to_json(&self) -> Json {
        wrap(vec![
            ("version".into(), Json::from(self.version)),
            ("meta".into(), self.meta.to_json()),
            (
                "prompts".into(),
                Json::Arr(self.prompts.iter().map(PromptSummary::to_json).collect()),
            ),
            (
                "steps".into(),
                Json::Arr(self.steps.iter().map(ExportStep::to_json).collect()),
            ),
            (
                "incidents".into(),
                Json::Arr(self.incidents.iter().map(Incident::to_json).collect()),
            ),
            (
                "files".into(),
                Json::Obj(
                    self.files
                        .iter()
                        .map(|(path, summary)| (path.clone(), summary.to_json()))
                        .collect(),
                ),
            ),
            ("stats".into(), self.stats.to_json()),
        ])
    }

    pub fn from_json(value: &Json) -> Result<Self> {
        // `meta` is checked first: a document without it is not a session at
        // all, and that is the message a reader needs.
        let meta = SessionMeta::from_json(field(value, "meta")?)?;
        let mut files = BTreeMap::new();
        if let Some(fields) = field(value, "files")?.as_object() {
            for (path, summary) in fields {
                files.insert(path.clone(), FileSummary::from_json(summary)?);
            }
        }
        Ok(SessionExport {
            version: uint_field(value, "version").unwrap_or(u64::from(SCHEMA_VERSION)) as u8,
            meta,
            prompts: array_field(value, "prompts")?
                .iter()
                .map(PromptSummary::from_json)
                .collect::<Result<_>>()?,
            steps: array_field(value, "steps")?
                .iter()
                .map(ExportStep::from_json)
                .collect::<Result<_>>()?,
            incidents: array_field(value, "incidents")?
                .iter()
                .map(Incident::from_json)
                .collect::<Result<_>>()?,
            files,
            stats: SessionStats::from_json(field(value, "stats")?)?,
        })
    }

    /// Read a `session.json`. Errors name the field that was wrong, because the
    /// usual failure is a file that is not a session at all.
    pub fn parse(text: &str) -> Result<Self> {
        SessionExport::from_json(&json::parse(text)?)
    }

    pub fn to_pretty_string(&self) -> String {
        json::to_pretty(&self.to_json())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub agent: Option<String>,
    /// RFC 3339, UTC.
    pub started_at: String,
    pub duration_ms: u64,
    pub os: String,
    pub repo_name: String,
    pub label: SessionLabel,
}

impl SessionMeta {
    fn to_json(&self) -> Json {
        wrap(vec![
            ("id".into(), Json::from(&self.id)),
            ("title".into(), Json::from(&self.title)),
            ("agent".into(), Json::from(self.agent.clone())),
            ("started_at".into(), Json::from(&self.started_at)),
            ("duration_ms".into(), Json::from(self.duration_ms)),
            ("os".into(), Json::from(&self.os)),
            ("repo_name".into(), Json::from(&self.repo_name)),
            ("label".into(), Json::from(self.label.as_str())),
        ])
    }

    fn from_json(value: &Json) -> Result<Self> {
        Ok(SessionMeta {
            id: text_field(value, "id")?,
            title: text_field(value, "title")?,
            agent: opt_text_field(value, "agent"),
            started_at: text_field(value, "started_at")?,
            duration_ms: opt_uint_field(value, "duration_ms").unwrap_or(0),
            os: opt_text_field(value, "os").unwrap_or_else(|| "unknown".into()),
            repo_name: opt_text_field(value, "repo_name").unwrap_or_default(),
            label: SessionLabel::from_str(opt_text_field(value, "label").as_deref()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLabel {
    Recorded,
    Fixture,
}

impl SessionLabel {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionLabel::Recorded => "recorded",
            SessionLabel::Fixture => "fixture",
        }
    }

    fn from_str(text: Option<&str>) -> Self {
        match text {
            Some("fixture") => SessionLabel::Fixture,
            _ => SessionLabel::Recorded,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptSummary {
    pub id: String,
    pub text: String,
    /// Index into `steps` of the prompt event itself.
    pub step: u64,
    pub t_ms: u64,
    pub files_touched: u32,
    pub commands_run: u32,
    pub incidents: u32,
}

impl PromptSummary {
    fn to_json(&self) -> Json {
        wrap(vec![
            ("id".into(), Json::from(&self.id)),
            ("text".into(), Json::from(&self.text)),
            ("step".into(), Json::from(self.step)),
            ("t_ms".into(), Json::from(self.t_ms)),
            ("files_touched".into(), Json::from(self.files_touched)),
            ("commands_run".into(), Json::from(self.commands_run)),
            ("incidents".into(), Json::from(self.incidents)),
        ])
    }

    fn from_json(value: &Json) -> Result<Self> {
        Ok(PromptSummary {
            id: text_field(value, "id")?,
            text: text_field(value, "text")?,
            step: uint_field(value, "step")?,
            t_ms: uint_field(value, "t_ms")?,
            files_touched: opt_uint_field(value, "files_touched").unwrap_or(0) as u32,
            commands_run: opt_uint_field(value, "commands_run").unwrap_or(0) as u32,
            incidents: opt_uint_field(value, "incidents").unwrap_or(0) as u32,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportStep {
    pub seq: u64,
    pub t_ms: u64,
    pub kind: EventKind,
    /// Inline for file changes, truncated.
    pub hunks: Option<Vec<DiffHunk>>,
}

impl ExportStep {
    pub fn to_json(&self) -> Json {
        let mut fields = Vec::new();
        put(&mut fields, "seq", Json::from(self.seq));
        put(&mut fields, "t_ms", Json::from(self.t_ms));
        fields.extend(self.kind.fields());
        if let Some(hunks) = &self.hunks {
            put(
                &mut fields,
                "hunks",
                Json::Arr(hunks.iter().map(DiffHunk::to_json).collect()),
            );
        }
        wrap(fields)
    }

    fn from_json(value: &Json) -> Result<Self> {
        let hunks = value
            .get("hunks")
            .and_then(Json::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(DiffHunk::from_json)
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        Ok(ExportStep {
            seq: uint_field(value, "seq")?,
            t_ms: uint_field(value, "t_ms")?,
            kind: EventKind::from_json(value)?,
            hunks,
        })
    }

    pub fn kind_name(&self) -> &'static str {
        self.kind.name()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffHunk {
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    fn to_json(&self) -> Json {
        wrap(vec![
            ("old_start".into(), Json::from(self.old_start)),
            ("new_start".into(), Json::from(self.new_start)),
            (
                "lines".into(),
                Json::Arr(self.lines.iter().map(DiffLine::to_json).collect()),
            ),
        ])
    }

    fn from_json(value: &Json) -> Result<Self> {
        Ok(DiffHunk {
            old_start: opt_uint_field(value, "old_start").unwrap_or(0) as u32,
            new_start: opt_uint_field(value, "new_start").unwrap_or(0) as u32,
            lines: array_field(value, "lines")?
                .iter()
                .map(DiffLine::from_json)
                .collect::<Result<_>>()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    pub tag: DiffTag,
    /// Truncated to 200 characters.
    pub text: String,
}

impl DiffLine {
    fn to_json(&self) -> Json {
        wrap(vec![
            ("tag".into(), Json::from(self.tag.as_str())),
            ("text".into(), Json::from(&self.text)),
        ])
    }

    fn from_json(value: &Json) -> Result<Self> {
        Ok(DiffLine {
            tag: DiffTag::from_str(value.get("tag").and_then(Json::as_str)),
            text: text_field(value, "text")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTag {
    Context,
    Add,
    Remove,
}

impl DiffTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiffTag::Context => "context",
            DiffTag::Add => "add",
            DiffTag::Remove => "remove",
        }
    }

    fn from_str(text: Option<&str>) -> Self {
        match text {
            Some("add") => DiffTag::Add,
            Some("remove") => DiffTag::Remove,
            _ => DiffTag::Context,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Incident {
    pub id: String,
    pub kind: IncidentKind,
    pub severity: Severity,
    /// Index into `steps` of the event the incident points at.
    pub step: u64,
    pub t_ms: u64,
    pub title: String,
    /// The actual line, command, or fact. Never a summary of a summary.
    pub evidence: String,
    pub why_it_matters: String,
    pub prompt_id: Option<String>,
    pub path: Option<String>,
}

impl Incident {
    fn to_json(&self) -> Json {
        wrap(vec![
            ("id".into(), Json::from(&self.id)),
            ("kind".into(), Json::from(self.kind.as_str())),
            ("severity".into(), Json::from(self.severity.as_str())),
            ("step".into(), Json::from(self.step)),
            ("t_ms".into(), Json::from(self.t_ms)),
            ("title".into(), Json::from(&self.title)),
            ("evidence".into(), Json::from(&self.evidence)),
            ("why_it_matters".into(), Json::from(&self.why_it_matters)),
            ("prompt_id".into(), Json::from(self.prompt_id.clone())),
            ("path".into(), Json::from(self.path.clone())),
        ])
    }

    fn from_json(value: &Json) -> Result<Self> {
        Ok(Incident {
            id: text_field(value, "id")?,
            kind: IncidentKind::from_str(value.get("kind").and_then(Json::as_str)),
            severity: Severity::from_str(value.get("severity").and_then(Json::as_str)),
            step: opt_uint_field(value, "step").unwrap_or(0),
            t_ms: opt_uint_field(value, "t_ms").unwrap_or(0),
            title: opt_text_field(value, "title").unwrap_or_default(),
            evidence: opt_text_field(value, "evidence").unwrap_or_default(),
            why_it_matters: opt_text_field(value, "why_it_matters").unwrap_or_default(),
            prompt_id: opt_text_field(value, "prompt_id"),
            path: opt_text_field(value, "path"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IncidentKind {
    TestWeakened,
    TestRegression,
    Thrashing,
    DangerousCommand,
    DependencyAdded,
    SecretTouched,
    ScopeCreep,
}

impl IncidentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            IncidentKind::TestWeakened => "test_weakened",
            IncidentKind::TestRegression => "test_regression",
            IncidentKind::Thrashing => "thrashing",
            IncidentKind::DangerousCommand => "dangerous_command",
            IncidentKind::DependencyAdded => "dependency_added",
            IncidentKind::SecretTouched => "secret_touched",
            IncidentKind::ScopeCreep => "scope_creep",
        }
    }

    /// Human label used in the report and the web UI.
    pub fn label(&self) -> &'static str {
        match self {
            IncidentKind::TestWeakened => "Test weakened",
            IncidentKind::TestRegression => "Test regression",
            IncidentKind::Thrashing => "Thrashing",
            IncidentKind::DangerousCommand => "Dangerous command",
            IncidentKind::DependencyAdded => "Dependency added",
            IncidentKind::SecretTouched => "Secret exposed",
            IncidentKind::ScopeCreep => "Scope creep",
        }
    }

    fn from_str(text: Option<&str>) -> Self {
        match text {
            Some("test_regression") => IncidentKind::TestRegression,
            Some("thrashing") => IncidentKind::Thrashing,
            Some("dangerous_command") => IncidentKind::DangerousCommand,
            Some("dependency_added") => IncidentKind::DependencyAdded,
            Some("secret_touched") => IncidentKind::SecretTouched,
            Some("scope_creep") => IncidentKind::ScopeCreep,
            _ => IncidentKind::TestWeakened,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warn,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Critical => "critical",
        }
    }

    fn from_str(text: Option<&str>) -> Self {
        match text {
            Some("critical") => Severity::Critical,
            Some("warn") => Severity::Warn,
            _ => Severity::Info,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileSummary {
    pub churn: u32,
    pub prompts: Vec<String>,
    pub ops: BTreeMap<String, u32>,
}

impl FileSummary {
    fn to_json(&self) -> Json {
        wrap(vec![
            ("churn".into(), Json::from(self.churn)),
            (
                "prompts".into(),
                Json::Arr(self.prompts.iter().map(Json::from).collect()),
            ),
            (
                "ops".into(),
                Json::Obj(
                    self.ops
                        .iter()
                        .map(|(op, count)| (op.clone(), Json::from(*count)))
                        .collect(),
                ),
            ),
        ])
    }

    fn from_json(value: &Json) -> Result<Self> {
        let mut ops = BTreeMap::new();
        if let Some(fields) = value.get("ops").and_then(Json::as_object) {
            for (op, count) in fields {
                ops.insert(op.clone(), count.as_u64().unwrap_or(0) as u32);
            }
        }
        Ok(FileSummary {
            churn: opt_uint_field(value, "churn").unwrap_or(0) as u32,
            prompts: value
                .get("prompts")
                .and_then(Json::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Json::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            ops,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionStats {
    pub steps: u64,
    pub files_touched: u32,
    pub commands: u32,
    pub test_runs: u32,
    pub incidents_by_kind: BTreeMap<String, u32>,
    pub tests_first_ok: Option<bool>,
    pub tests_last_ok: Option<bool>,
}

impl SessionStats {
    fn to_json(&self) -> Json {
        wrap(vec![
            ("steps".into(), Json::from(self.steps)),
            ("files_touched".into(), Json::from(self.files_touched)),
            ("commands".into(), Json::from(self.commands)),
            ("test_runs".into(), Json::from(self.test_runs)),
            (
                "incidents_by_kind".into(),
                Json::Obj(
                    self.incidents_by_kind
                        .iter()
                        .map(|(kind, count)| (kind.clone(), Json::from(*count)))
                        .collect(),
                ),
            ),
            ("tests_first_ok".into(), Json::from(self.tests_first_ok)),
            ("tests_last_ok".into(), Json::from(self.tests_last_ok)),
        ])
    }

    fn from_json(value: &Json) -> Result<Self> {
        let mut incidents_by_kind = BTreeMap::new();
        if let Some(fields) = value.get("incidents_by_kind").and_then(Json::as_object) {
            for (kind, count) in fields {
                incidents_by_kind.insert(kind.clone(), count.as_u64().unwrap_or(0) as u32);
            }
        }
        Ok(SessionStats {
            steps: opt_uint_field(value, "steps").unwrap_or(0),
            files_touched: opt_uint_field(value, "files_touched").unwrap_or(0) as u32,
            commands: opt_uint_field(value, "commands").unwrap_or(0) as u32,
            test_runs: opt_uint_field(value, "test_runs").unwrap_or(0) as u32,
            incidents_by_kind,
            tests_first_ok: opt_bool_field(value, "tests_first_ok"),
            tests_last_ok: opt_bool_field(value, "tests_last_ok"),
        })
    }
}

/// Sidecar written next to `events.jsonl`. Carries the wall-clock start, which
/// the event stream itself deliberately does not (all event times are relative).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub agent: Option<String>,
    pub started_at: String,
    pub os: String,
    pub repo_name: String,
    pub label: SessionLabel,
}

impl SessionRecord {
    pub fn to_json(&self) -> Json {
        wrap(vec![
            ("id".into(), Json::from(&self.id)),
            ("title".into(), Json::from(&self.title)),
            ("agent".into(), Json::from(self.agent.clone())),
            ("started_at".into(), Json::from(&self.started_at)),
            ("os".into(), Json::from(&self.os)),
            ("repo_name".into(), Json::from(&self.repo_name)),
            ("label".into(), Json::from(self.label.as_str())),
        ])
    }

    pub fn from_json(value: &Json) -> Result<Self> {
        Ok(SessionRecord {
            id: text_field(value, "id")?,
            title: text_field(value, "title")?,
            agent: opt_text_field(value, "agent"),
            started_at: text_field(value, "started_at")?,
            os: opt_text_field(value, "os").unwrap_or_else(|| "unknown".into()),
            repo_name: opt_text_field(value, "repo_name").unwrap_or_default(),
            label: SessionLabel::from_str(opt_text_field(value, "label").as_deref()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SessionExport {
        let mut files = BTreeMap::new();
        files.insert(
            "src/a.rs".to_string(),
            FileSummary {
                churn: 2,
                prompts: vec!["p-1".into()],
                ops: BTreeMap::from([("modify".to_string(), 2)]),
            },
        );
        SessionExport {
            version: SCHEMA_VERSION,
            meta: SessionMeta {
                id: "20260101T000000Z-0001".into(),
                title: "Fixture".into(),
                agent: Some("claude".into()),
                started_at: "2026-01-01T00:00:00.000Z".into(),
                duration_ms: 4200,
                os: "windows".into(),
                repo_name: "repo".into(),
                label: SessionLabel::Fixture,
            },
            prompts: vec![PromptSummary {
                id: "p-1".into(),
                text: "fix it".into(),
                step: 1,
                t_ms: 10,
                files_touched: 1,
                commands_run: 0,
                incidents: 0,
            }],
            steps: vec![ExportStep {
                seq: 1,
                t_ms: 10,
                kind: EventKind::Prompt(Prompt {
                    id: "p-1".into(),
                    text: "fix it".into(),
                    source: PromptSource::Mark,
                }),
                hunks: None,
            }],
            incidents: vec![Incident {
                id: "i-0".into(),
                kind: IncidentKind::Thrashing,
                severity: Severity::Warn,
                step: 2,
                t_ms: 30,
                title: "rewritten".into(),
                evidence: "5 changes".into(),
                why_it_matters: "unclear".into(),
                prompt_id: Some("p-1".into()),
                path: Some("src/a.rs".into()),
            }],
            files,
            stats: SessionStats {
                steps: 3,
                files_touched: 1,
                commands: 0,
                test_runs: 1,
                incidents_by_kind: BTreeMap::from([("thrashing".to_string(), 1)]),
                tests_first_ok: Some(true),
                tests_last_ok: Some(false),
            },
        }
    }

    #[test]
    fn export_round_trips_through_pretty_json() {
        let export = sample();
        let text = export.to_pretty_string();
        let back = SessionExport::parse(&text).unwrap();
        assert_eq!(export, back);
    }

    #[test]
    fn events_round_trip_with_a_flattened_kind() {
        let event = Event::new(
            4,
            120,
            EventKind::FileChange(FileChange {
                path: "src/a.rs".into(),
                op: FileOp::Modify,
                before_hash: None,
                after_hash: Some("ab".repeat(16)),
                lines_added: 2,
                lines_removed: 1,
                prompt_id: Some("p-2".into()),
            }),
        );
        let text = json::to_compact(&event.to_json());
        assert!(text.contains("\"kind\":\"file_change\""), "{text}");
        assert_eq!(
            Event::from_json(&json::parse(&text).unwrap()).unwrap(),
            event
        );
    }

    #[test]
    fn absent_hunks_are_omitted_and_empty_ones_kept() {
        let mut step = sample().steps[0].clone();
        assert!(!step.to_json().get("hunks").is_some());
        step.hunks = Some(Vec::new());
        assert_eq!(
            step.to_json().get("hunks").and_then(Json::as_array),
            Some(&Vec::new()[..])
        );
    }

    #[test]
    fn a_non_session_document_fails_with_a_readable_message() {
        let error = SessionExport::parse(r#"{"hello":1}"#).unwrap_err();
        assert!(error.to_string().contains("meta"), "{error}");
    }
}
