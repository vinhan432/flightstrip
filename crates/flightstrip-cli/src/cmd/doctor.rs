//! `flightstrip doctor`: the checks that are cheap to automate and expensive to
//! discover in the middle of a recording.

use crate::config::Config;
use crate::error::Result;
use crate::style::{self, Style};
use flightstrip_core::json;
use flightstrip_core::paths;
use flightstrip_core::session;
use std::path::Path;
use std::process::Command as Process;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

struct Check {
    name: &'static str,
    status: CheckStatus,
    detail: String,
    fix: Option<String>,
}

pub fn run(root: &Path, as_json: bool, quiet: bool, style: Style) -> Result<bool> {
    let checks = checks(root);
    let failed = checks.iter().any(|check| check.status == CheckStatus::Fail);

    if as_json {
        let items = json::Json::Arr(
            checks
                .iter()
                .map(|check| {
                    json::obj(vec![
                        ("check", json::Json::from(check.name)),
                        (
                            "status",
                            json::Json::from(match check.status {
                                CheckStatus::Pass => "pass",
                                CheckStatus::Warn => "warn",
                                CheckStatus::Fail => "fail",
                            }),
                        ),
                        ("detail", json::Json::from(&check.detail)),
                        (
                            "fix",
                            match &check.fix {
                                Some(fix) => json::Json::from(fix),
                                None => json::Json::Null,
                            },
                        ),
                    ])
                })
                .collect(),
        );
        style::emit(&json::to_pretty(&json::obj(vec![
            ("ok", json::Json::Bool(!failed)),
            ("checks", items),
        ])));
        return Ok(!failed);
    }

    let mut out = String::new();
    for check in &checks {
        if quiet && check.status == CheckStatus::Pass {
            continue;
        }
        let label = match check.status {
            CheckStatus::Pass => style.pass(check.name),
            CheckStatus::Warn => style.warn(check.name),
            CheckStatus::Fail => style.fail(check.name),
        };
        out.push_str(&format!("{label}: {}\n", check.detail));
        if check.status != CheckStatus::Pass {
            if let Some(fix) = &check.fix {
                out.push_str(&format!("       fix: {fix}\n"));
            }
        }
    }
    out.push_str(&format!(
        "\n{} pass · {} warning · {} failure\n",
        checks
            .iter()
            .filter(|c| c.status == CheckStatus::Pass)
            .count(),
        checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warn)
            .count(),
        checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .count()
    ));
    style::emit(&out);
    Ok(!failed)
}

fn checks(root: &Path) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(Check {
        name: "os",
        status: CheckStatus::Pass,
        detail: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        fix: None,
    });

    checks.push(match Process::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => Check {
            name: "git",
            status: CheckStatus::Pass,
            detail: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            fix: None,
        },
        _ => Check {
            name: "git",
            status: CheckStatus::Warn,
            detail: "git was not found on PATH (optional, but session ids and .gitignore \
                     checks are less useful without it)"
                .into(),
            fix: Some("install Git and reopen the terminal".into()),
        },
    });

    let readable = std::fs::read_dir(root).is_ok();
    checks.push(Check {
        name: "watch permissions",
        status: if readable {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        detail: if readable {
            format!(
                "can read {}",
                paths::to_slashes(&root.display().to_string())
            )
        } else {
            format!(
                "cannot read {}; the recorder would see no files",
                paths::to_slashes(&root.display().to_string())
            )
        },
        fix: if readable {
            None
        } else {
            Some("check the directory permissions or run from the repository root".into())
        },
    });

    let state = session::state_dir(root);
    let probe = state.join(".doctor-probe");
    let writable = std::fs::create_dir_all(&state).is_ok()
        && flightstrip_core::write_atomic(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    checks.push(Check {
        name: "state write access",
        status: if writable {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        detail: if writable {
            format!(
                "can write {}",
                paths::to_slashes(&state.display().to_string())
            )
        } else {
            format!(
                "cannot write {}; no session can be recorded",
                paths::to_slashes(&state.display().to_string())
            )
        },
        fix: if writable {
            None
        } else {
            Some("make the repository directory writable, or move it to a local drive".into())
        },
    });

    checks.push(match session::read_active(root) {
        Some(active) if session::pid_alive(active.pid) => Check {
            name: "active session",
            status: CheckStatus::Pass,
            detail: format!("{} is recording (pid {})", active.id, active.pid),
            fix: None,
        },
        Some(active) => Check {
            name: "active session",
            status: CheckStatus::Fail,
            detail: format!(
                "{} points at pid {}, which is no longer running",
                active.id, active.pid
            ),
            fix: Some(
                "start a new recording; a dead pointer is cleared by watch, mark and exec".into(),
            ),
        },
        None => Check {
            name: "active session",
            status: CheckStatus::Pass,
            detail: "no active session".into(),
            fix: None,
        },
    });

    checks.push(match Config::load(root) {
        Ok(_) if Config::exists(root) => Check {
            name: "config",
            status: CheckStatus::Pass,
            detail: paths::to_slashes(&Config::path(root).display().to_string()),
            fix: None,
        },
        Ok(_) => Check {
            name: "config",
            status: CheckStatus::Warn,
            detail: "no .flightstrip/config.toml; using built-in defaults".into(),
            fix: Some("run `flightstrip init` to write the documented defaults".into()),
        },
        Err(error) => Check {
            name: "config",
            status: CheckStatus::Fail,
            detail: error.to_string(),
            fix: Some("fix the reported line, or run `flightstrip init --force`".into()),
        },
    });

    checks.push({
        let path = root.join(".gitignore");
        if !path.is_file() {
            Check {
                name: "gitignore",
                status: CheckStatus::Warn,
                detail: "no .gitignore; .flightstrip/ would be visible to git".into(),
                fix: Some("create a .gitignore, then run `flightstrip init`".into()),
            }
        } else {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let ignored = text.lines().any(crate::config::ignores_state_dir);
            if ignored {
                Check {
                    name: "gitignore",
                    status: CheckStatus::Pass,
                    detail: ".flightstrip/ is ignored".into(),
                    fix: None,
                }
            } else {
                Check {
                    name: "gitignore",
                    status: CheckStatus::Warn,
                    detail: ".gitignore does not list .flightstrip/".into(),
                    fix: Some("run `flightstrip init` to add it".into()),
                }
            }
        }
    });

    // A config that is valid but has a glob that matches everything would
    // silently record nothing; that is worth a warning.
    if let Ok(cfg) = Config::load(root) {
        if cfg
            .ignore_globs
            .iter()
            .any(|glob| glob == "**" || glob == "*")
        {
            checks.push(Check {
                name: "ignore globs",
                status: CheckStatus::Warn,
                detail: "a config glob matches every path; the recording may be empty".into(),
                fix: Some("narrow ignore_globs, for example [\"*.log\", \"tmp/**\"]".into()),
            });
        }
        if cfg.poll_ms < 100 {
            checks.push(Check {
                name: "poll interval",
                status: CheckStatus::Warn,
                detail: format!(
                    "poll_ms = {} is fast enough to keep a core busy on a large repository",
                    cfg.poll_ms
                ),
                fix: Some("use 250-1000 ms unless you need very low latency".into()),
            });
        }
    }

    checks
}
