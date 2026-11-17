//! Terminal styling. Small enough to keep std-only; respects `NO_COLOR`,
//! `CLICOLOR=0`, `TERM=dumb` and non-TTY output.

use std::io::{IsTerminal, Write};

#[derive(Debug, Clone, Copy)]
pub struct Style {
    enabled: bool,
}

impl Style {
    pub fn auto() -> Style {
        let disabled = std::env::var_os("NO_COLOR").is_some()
            || std::env::var("CLICOLOR").ok().as_deref() == Some("0")
            || std::env::var("TERM").ok().as_deref() == Some("dumb")
            || !std::io::stdout().is_terminal();
        Style { enabled: !disabled }
    }

    #[cfg(test)]
    pub fn plain() -> Style {
        Style { enabled: false }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    pub fn green(&self, text: &str) -> String {
        self.paint("32", text)
    }

    pub fn red(&self, text: &str) -> String {
        self.paint("31", text)
    }

    pub fn yellow(&self, text: &str) -> String {
        self.paint("33", text)
    }

    pub fn cyan(&self, text: &str) -> String {
        self.paint("36", text)
    }

    /// `[ok]`, `[!]`, `[x]` — ASCII so a redirected log stays readable.
    pub fn pass(&self, label: &str) -> String {
        format!("{} {label}", self.green("[ok]"))
    }

    pub fn warn(&self, label: &str) -> String {
        format!("{} {label}", self.yellow("[!]"))
    }

    pub fn fail(&self, label: &str) -> String {
        format!("{} {label}", self.red("[x]"))
    }

    pub fn info(&self, label: &str) -> String {
        format!("{} {label}", self.cyan("[-]"))
    }
}

/// Print a line and ignore a closed pipe (`flightstrip list | head`). A CLI that
/// panics on SIGPIPE is a bug in the shell, not in the data.
pub fn emit(text: &str) {
    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(text.as_bytes()).is_err() {
        return;
    }
    if !text.ends_with('\n') {
        let _ = stdout.write_all(b"\n");
    }
    let _ = stdout.flush();
}
