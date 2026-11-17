//! `flightstrip exec`: run a command, record it, and read the test summary out
//! of its output.
//!
//! One argument is treated as a shell command so `flightstrip exec -- "cargo
//! test | tail"` still works. Several arguments are spawned directly, which
//! keeps spaces and quotes in arguments intact instead of asking a shell to
//! re-parse them. Output is echoed as it arrives and captured (capped) for the
//! test parser, so a runaway build log cannot become one enormous event.

use crate::cmd;
use crate::error::{Context, Result};
use crate::interrupt;
use crate::style::Style;
use flightstrip_core::paths;
use flightstrip_core::schema::{Command, CommandSource, EventKind, TestRun};
use flightstrip_core::session::{self, EventLog};
use flightstrip_core::testparse;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command as Process, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Longest captured output kept for parsing. Enough for a full test summary at
/// the end; anything beyond is only echoed to the terminal.
const CAPTURE_LIMIT: usize = 2 * 1024 * 1024;

pub fn run(root: &Path, argv: &[String], style: Style) -> Result<i32> {
    if argv.is_empty() {
        crate::bail!("exec needs a command: flightstrip exec -- cargo test");
    }
    let id = cmd::active_session(root)?;
    let dir = session::session_dir(root, &id);
    let cmdline = argv
        .iter()
        .map(|arg| quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let cwd = std::env::current_dir().context("while reading the current directory")?;
    let cwd = paths::relative(root, &cwd)
        .map(|relative| {
            if relative == "." {
                ".".to_string()
            } else {
                relative
            }
        })
        .unwrap_or_else(|| paths::to_slashes(&cwd.display().to_string()));

    let mut log = EventLog::open(root, &id).context("while opening the session")?;
    let prompt_id = session::current_prompt(&dir);

    // Keep this process alive through Ctrl+C so the command is still recorded;
    // the child shares the console and receives the signal too.
    let _ = interrupt::install();

    let started = Instant::now();
    let mut child = match spawn(argv) {
        Ok(child) => child,
        Err(error) => crate::bail!("could not run `{cmdline}`: {error}"),
    };

    let out_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let err_buf = Arc::new(Mutex::new(Vec::<u8>::new()));

    let out_thread = child.stdout.take().map(|stdout| {
        let buffer = Arc::clone(&out_buf);
        std::thread::spawn(move || pump(stdout, std::io::stdout(), buffer))
    });
    let err_thread = child.stderr.take().map(|stderr| {
        let buffer = Arc::clone(&err_buf);
        std::thread::spawn(move || pump(stderr, std::io::stderr(), buffer))
    });

    let status = child.wait().context("while waiting for the command")?;
    if let Some(handle) = out_thread {
        let _ = handle.join();
    }
    if let Some(handle) = err_thread {
        let _ = handle.join();
    }
    let duration_ms = started.elapsed().as_millis() as u64;
    let interrupted = interrupt::presses() > 0;
    let exit_code = if interrupted { None } else { status.code() };

    log.append(EventKind::Command(Command {
        cmd: cmdline.clone(),
        exit_code,
        duration_ms: Some(duration_ms),
        source: CommandSource::Exec,
        cwd: Some(cwd),
        prompt_id: prompt_id.clone(),
    }))
    .context("while recording the command")?;

    if interrupted {
        println!(
            "\n{} `{cmdline}` was interrupted after {}ms; the recording is intact.",
            style.warn("Interrupted"),
            duration_ms
        );
        return Ok(130);
    }

    let captured = {
        let stdout =
            String::from_utf8_lossy(&out_buf.lock().unwrap_or_else(|e| e.into_inner())).to_string();
        let stderr =
            String::from_utf8_lossy(&err_buf.lock().unwrap_or_else(|e| e.into_inner())).to_string();
        format!("{stdout}\n{stderr}")
    };

    if let Some(parsed) = testparse::parse(&cmdline, &captured, exit_code) {
        log.append(EventKind::TestRun(TestRun {
            cmd: cmdline.clone(),
            passed: parsed.passed,
            failed: parsed.failed,
            skipped: parsed.skipped,
            total: parsed.total,
            ok: parsed.ok,
            prompt_id,
        }))
        .context("while recording the test run")?;
        println!(
            "\nflightstrip: {} → {} in {}ms · {} passed, {} failed, {} skipped",
            if parsed.ok {
                style.pass("PASS")
            } else {
                style.fail("FAIL")
            },
            exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "no exit code".into()),
            duration_ms,
            parsed.passed,
            parsed.failed,
            parsed.skipped
        );
    } else {
        println!(
            "\nflightstrip: recorded `{cmdline}` → {}",
            exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "no exit code".into())
        );
    }

    // Mirror the child's status so `flightstrip exec` can sit in a script.
    Ok(exit_code.unwrap_or(1))
}

fn spawn(argv: &[String]) -> std::io::Result<std::process::Child> {
    let mut command = if argv.len() == 1 {
        if cfg!(target_os = "windows") {
            let mut command = Process::new("cmd");
            command.args(["/C", &argv[0]]);
            command
        } else {
            let mut command = Process::new("sh");
            command.args(["-c", &argv[0]]);
            command
        }
    } else {
        let mut command = Process::new(&argv[0]);
        command.args(&argv[1..]);
        command
    };
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Display form, not a shell-escaping guarantee: arguments with spaces are
/// quoted so the recorded command reads back the way it was run.
fn quote(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| !c.is_whitespace() && c != '"' && c != '\'')
    {
        arg.to_string()
    } else {
        format!("\"{}\"", arg.replace('"', "\\\""))
    }
}

fn pump<R, W>(mut source: R, mut echo: W, buffer: Arc<Mutex<Vec<u8>>>)
where
    R: Read,
    W: Write,
{
    let mut chunk = [0u8; 8192];
    loop {
        match source.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                let bytes = &chunk[..read];
                let _ = echo.write_all(bytes);
                let _ = echo.flush();
                if let Ok(mut captured) = buffer.lock() {
                    if captured.len() < CAPTURE_LIMIT {
                        let room = CAPTURE_LIMIT - captured.len();
                        captured.extend_from_slice(&bytes[..bytes.len().min(room)]);
                    }
                }
            }
            Err(_) => break,
        }
    }
}
