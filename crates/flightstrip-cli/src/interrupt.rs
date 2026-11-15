//! Ctrl+C, so a recording ends with a clean summary instead of a dead process.
//!
//! The handler only touches atomics, which is async-signal-safe; the first press
//! asks the loop to stop, the second forces the process down because something
//! is wedged. This is the whole reason `ctrlc` exists as a crate — forty lines on
//! two platforms is cheaper than the dependency.

use crate::error::Result;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static STOP: AtomicBool = AtomicBool::new(false);
static PRESSES: AtomicU32 = AtomicU32::new(0);

/// Set once the first Ctrl+C has been seen. Handed to the recorder loop.
pub fn flag() -> &'static AtomicBool {
    &STOP
}

/// How many Ctrl+C presses have arrived.
pub fn presses() -> u32 {
    PRESSES.load(Ordering::SeqCst)
}

fn note_press() {
    let count = PRESSES.fetch_add(1, Ordering::SeqCst) + 1;
    if count == 1 {
        STOP.store(true, Ordering::SeqCst);
    } else {
        std::process::exit(130);
    }
}

#[cfg(windows)]
pub fn install() -> Result<()> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }

    // CTRL_C_EVENT / CTRL_BREAK_EVENT. Closing the window (CTRL_CLOSE_EVENT) is
    // deliberately not handled: the OS gives a process very little time then.
    unsafe extern "system" fn handler(event: u32) -> i32 {
        if event == 0 || event == 1 {
            note_press();
            if presses() == 1 {
                eprintln!("\nflightstrip: closing the session (Ctrl+C again to force quit)");
            }
            return 1;
        }
        0
    }

    let installed = unsafe { SetConsoleCtrlHandler(Some(handler), 1) };
    if installed == 0 {
        return Err("could not install the Ctrl+C handler".into());
    }
    Ok(())
}

#[cfg(unix)]
pub fn install() -> Result<()> {
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }

    extern "C" fn handler(_signal: i32) {
        note_press();
        if presses() == 1 {
            // write(2) is async-signal-safe; eprintln! is close enough in
            // practice and makes the second-press warning visible.
            eprintln!("\nflightstrip: closing the session (Ctrl+C again to force quit)");
        }
    }

    // SIG_ERR is (sighandler_t)-1.
    let previous = unsafe {
        signal(2 /* SIGINT */, handler as usize)
    };
    if previous == usize::MAX {
        return Err("could not install the Ctrl+C handler".into());
    }
    Ok(())
}

#[cfg(not(any(windows, unix)))]
pub fn install() -> Result<()> {
    // Nothing to install; the process simply dies on Ctrl+C.
    Ok(())
}
