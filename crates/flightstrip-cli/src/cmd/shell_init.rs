//! `flightstrip shell-init <shell>`: print a snippet, never edit a profile.

use crate::ShellArg;

pub fn run(shell: ShellArg) {
    match shell {
        ShellArg::Powershell => {
            print!(
                r#"# Add this to your PowerShell profile yourself; Flightstrip does not edit it.
function fsmark {{ flightstrip mark @args }}
function fsx {{ flightstrip exec -- @args }}
"#
            );
        }
        ShellArg::Bash | ShellArg::Zsh => {
            print!(
                r#"# Add this to ~/.bashrc or ~/.zshrc yourself; Flightstrip does not edit it.
fsmark() {{ flightstrip mark "$@"; }}
fsx() {{ flightstrip exec -- "$@"; }}
"#
            );
        }
    }
}
