//! `anigit uninstall` — remove the anigit install itself (brainstorm.md
//! 1.17): the running binary plus the bundled/synced catalog SQLite file,
//! and NOTHING else. `.anigit/` repos and their generated folder trees are
//! user data, not install artifacts, and are never touched here.
//!
//! Confirmation design (decided in 1.17): prompt once, and on a literal
//! `y` delete IMMEDIATELY in the same invocation — an earlier "prompt,
//! then ask the user to re-run with a flag" shape was rejected as
//! pointless friction for something just confirmed. `--confirm` skips the
//! prompt entirely (the motivating use case is scripted repeat
//! install/uninstall cycles while testing package managers).
//!
//! Deleting the binary is the platform-sensitive part: on Unix, unlinking
//! a running executable is fine (the inode stays alive until the process
//! exits), so a plain `remove_file` works. On Windows, the OS locks an
//! executing image and a direct delete fails — instead we spawn a tiny
//! detached `cmd` that waits a moment for this process to exit and then
//! deletes the unlocked file.

use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::catalog::catalog_path_for_sync;

pub fn run(confirm: bool) -> Result<()> {
    let binary = env::current_exe()
        .context("could not determine the path of the running anigit binary")?;
    let catalog = catalog_path_for_sync()?;

    if !confirm {
        print!(
            "This will permanently remove:\n\
             \x20 - The anigit binary ({})\n\
             \x20 - The bundled anime catalog ({})\n\
             \n\
             Your .anigit repos and generated folder trees are NOT affected and will remain untouched.\n\
             \n\
             Tip: use --confirm next time to skip this prompt.\n\
             \n\
             Type 'y' to confirm, anything else cancels: ",
            binary.display(),
            catalog.display()
        );
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        if !is_confirmed(&line) {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Catalog first: it deletes synchronously on every platform, so if the
    // binary step fails the summary still reflects reality. Missing catalog
    // (e.g. `anigit refresh` never ran) is skipped silently — nothing to
    // remove isn't an error.
    if catalog.is_file() {
        fs::remove_file(&catalog)
            .with_context(|| format!("failed to remove the catalog file {}", catalog.display()))?;
        println!(
            "{}",
            format!("Removed the anime catalog ({}).", catalog.display()).green()
        );
    }

    let binary_summary = remove_binary(&binary)?;
    println!("{}", binary_summary.green());
    println!("anigit is uninstalled. Your .anigit repos were not touched.");

    Ok(())
}

/// Only a literal lowercase `y` proceeds — matching the prompt's own
/// wording. `Y`, `yes`, empty input (bare Enter must NOT default to yes),
/// or anything else cancels. Only the line ending is stripped before
/// comparing, so even ` y ` cancels.
fn is_confirmed(line: &str) -> bool {
    line.trim_end_matches(['\r', '\n']) == "y"
}

/// Unix: unlinking a running executable is safe — the kernel keeps the
/// underlying inode alive until the process exits, then reclaims it — so a
/// plain synchronous delete works (verified live on macOS).
#[cfg(unix)]
fn remove_binary(binary: &Path) -> Result<String> {
    fs::remove_file(binary)
        .with_context(|| format!("failed to remove the anigit binary {}", binary.display()))?;
    Ok(format!("Removed the anigit binary ({}).", binary.display()))
}

/// Windows: the OS locks an actively-executing image, so deleting it from
/// within the process fails. Standard workaround (same shape rustup's
/// self-uninstall uses): spawn a detached `cmd` helper that outlives this
/// process, waits ~2 seconds for it to exit and release the lock (`ping`
/// is cmd's portable sleep), then deletes the now-unlocked file.
#[cfg(windows)]
fn remove_binary(binary: &Path) -> Result<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    // Detach fully so the helper survives this process exiting and never
    // flashes a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    Command::new("cmd")
        .args([
            "/C",
            &format!(
                "ping 127.0.0.1 -n 3 > nul & del /f /q \"{}\"",
                binary.display()
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .with_context(|| {
            format!(
                "failed to schedule deletion of the anigit binary {}",
                binary.display()
            )
        })?;
    Ok(format!(
        "The anigit binary ({}) will be deleted a moment after this command exits.",
        binary.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::is_confirmed;

    #[test]
    fn only_literal_lowercase_y_confirms() {
        assert!(is_confirmed("y\n"));
        assert!(is_confirmed("y\r\n")); // Windows line ending
        assert!(is_confirmed("y")); // EOF without newline
    }

    #[test]
    fn everything_else_cancels() {
        assert!(!is_confirmed("Y\n")); // uppercase — prompt says lowercase y
        assert!(!is_confirmed("yes\n"));
        assert!(!is_confirmed("\n")); // bare Enter must not default to yes
        assert!(!is_confirmed("")); // closed stdin
        assert!(!is_confirmed(" y\n")); // padded
        assert!(!is_confirmed("y \n"));
        assert!(!is_confirmed("n\n"));
        assert!(!is_confirmed("anything else\n"));
    }
}
