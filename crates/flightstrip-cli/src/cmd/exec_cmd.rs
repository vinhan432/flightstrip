//! `flightstrip exec`: run a command the way a shell would, record it, and read
//! the test summary out of its output.
//!
//! Output is written to this process's own stdout/stderr as it arrives, so a
//! recorded command behaves like a normal one. It is also captured for the parser
//! and capped, because a runaway build log should not become an event line.

use crate::cmd;
use crate::error::{Context, Result};
use crate::interrupt;
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

pub fn run(root: &Path, argv: &[String]) -> Result<i32> {
    let id = cmd::active_session(root)?;
    let dir = session::session_dir(root, &id);
    let cmdline = argv.join(" ");

    let mut log = EventLog::open(root, &id).context("while opening the session")?;
    let prompt_id = session::current_prompt(&dir);

    // Keep this process alive through Ctrl+C so the command is still recorded;
    // the child shares the console and gets the signal itself.
    let _ = interrupt::install();

    let (program, args) = if cfg!(target_os = "windows") {
        ("cmd", vec!["/C".to_string(), cmdline.clone()])
    } else {
        ("sh", vec!["-c".to_string(), cmdline.clone()])
    };

    let started = Instant::now();
    let mut child = match Process::new(program)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
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
    let exit_code = status.code();

    log.append(EventKind::Command(Command {
        cmd: cmdline.clone(),
        exit_code,
        duration_ms: Some(duration_ms),
        source: CommandSource::Exec,
        prompt_id: prompt_id.clone(),
    }))
    .context("while recording the command")?;

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
            if parsed.ok { "PASS" } else { "FAIL" },
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
