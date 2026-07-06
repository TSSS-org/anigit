use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::repo::config::ForkProvenance;
use crate::repo::{Repo, ANIGIT_DIR};

/// `anigit clone <url> [destination]` — copy an existing repo.
///
/// v1 note: only local-path "URLs" are meaningful until AniHub/remote sync
/// exists (v2+) — see brainstorm.md 1.8. A `<url.anigit>` scheme for
/// AniHub-hosted repos is anticipated but not implemented yet.
pub fn run_clone(url: &str, destination: Option<&str>) -> Result<()> {
    let dest = clone_into(url, destination)?;
    println!("Cloned '{url}' into {}.", dest.display());
    Ok(())
}

/// `anigit fork <url.anigit>` — anigit's own invented command (no real git
/// equivalent; "fork" is a GitHub concept). Functions like `clone` but tags
/// the resulting repo with fork status/provenance (source + timestamp in
/// `.anigit/config`), so forked repos can show their lineage on AniHub once
/// that exists. See brainstorm.md 1.7a.
pub fn run_fork(url: &str, destination: Option<&str>) -> Result<()> {
    let dest = clone_into(url, destination)?;
    let repo = Repo::discover(&dest)?;
    let mut config = repo.config()?;
    config.forked_from = Some(ForkProvenance {
        source: url.to_string(),
        forked_at: Utc::now(),
    });
    repo.write_config(&config)?;
    println!(
        "Forked '{url}' into {} — provenance recorded (see `anigit config show`).",
        dest.display()
    );
    Ok(())
}

/// The shared copy mechanism: validate the source repo, pick a destination,
/// copy the `.anigit/` tree. Returns the destination working directory.
fn clone_into(url: &str, destination: Option<&str>) -> Result<PathBuf> {
    if url.contains("://") {
        bail!(
            "remote URLs aren't supported yet — v1 clone/fork is local-only \
             (brainstorm.md 1.8); pass a filesystem path to another repo"
        );
    }
    let source = PathBuf::from(url);
    let source_repo = source.join(ANIGIT_DIR);
    if !source_repo.is_dir() {
        bail!("'{url}' is not an anigit repo (no {ANIGIT_DIR} directory inside)");
    }

    let dest = match destination {
        Some(d) => PathBuf::from(d),
        // No destination given: use the source directory's own name, in the
        // current working directory (same convention as real git clone).
        None => {
            let canonical = source
                .canonicalize()
                .with_context(|| format!("cannot resolve source path '{url}'"))?;
            match canonical.file_name() {
                Some(name) => PathBuf::from(name),
                None => bail!("cannot derive a destination name from '{url}' — pass one explicitly"),
            }
        }
    };
    if dest.exists() {
        bail!("destination '{}' already exists", dest.display());
    }

    copy_repo_tree(&source_repo, &dest.join(ANIGIT_DIR))?;
    Ok(dest)
}

/// Copy the `.anigit/` tree. `STAGED` is deliberately skipped: it's the
/// source repo's transient, uncommitted working state (see repo/staging.rs),
/// not history — the same reason real `git clone` doesn't carry over the
/// source's index.
fn copy_repo_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_name().to_str() == Some("STAGED")
            && src.file_name().and_then(|n| n.to_str()) == Some(ANIGIT_DIR)
        {
            continue;
        }
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_repo_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
