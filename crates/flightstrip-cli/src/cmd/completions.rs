//! `flightstrip completions <shell>`.

use clap::CommandFactory;

pub fn run(shell: clap_complete::Shell) {
    let mut command = crate::Cli::command();
    clap_complete::generate(shell, &mut command, "flightstrip", &mut std::io::stdout());
}
