//! `flightstrip init`: write the project config and keep state out of git.

use crate::config;
use crate::error::Result;
use crate::style::Style;
use std::path::Path;

pub fn run(root: &Path, force: bool, style: Style) -> Result<()> {
    let (path, created) = config::Config::write(root, force)?;
    let gitignore_changed = config::ensure_gitignore(root)?;

    if created {
        println!("{} {}", style.pass("Created"), config::display_path(&path));
    } else {
        println!(
            "{} {} already exists (use --force to replace it)",
            style.info("Kept"),
            config::display_path(&path)
        );
    }

    if gitignore_changed {
        println!(
            "{} added .flightstrip/ to {}",
            style.pass("Updated"),
            config::display_path(&root.join(".gitignore"))
        );
    } else if root.join(".gitignore").is_file() {
        println!("{} .flightstrip/ is already ignored", style.pass("Checked"));
    } else {
        println!(
            "{} no .gitignore found; add .flightstrip/ before committing",
            style.warn("Note")
        );
    }

    println!();
    println!("Next:");
    println!("  flightstrip watch --agent \"your agent\"");
    println!("  flightstrip mark \"what you asked for\"");
    println!("  flightstrip exec -- cargo test");
    println!("  flightstrip export --out session.json");
    println!("  flightstrip verify session.json");
    Ok(())
}
