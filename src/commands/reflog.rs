use anyhow::Result;
use std::env;

use crate::repo::Repo;

/// `anigit reflog` — every branch-ref update, newest first, read from the
/// append-only `.anigit/logs/HEAD` audit trail (see `Repo::read_reflog`).
///
/// Real git's reflog exists to survive history rewrites; anigit's history is
/// append-only (brainstorm.md 1.3) so there's nothing to rewrite, but the
/// trail still answers "what did this branch point at, and when" — notably
/// after a `commit --amend`, which moves the branch head sideways without
/// the replaced tip appearing in `anigit log`'s chain.
pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let entries = repo.read_reflog()?;

    if entries.is_empty() {
        println!("Reflog is empty — no branch ref has moved yet.");
        return Ok(());
    }

    // Newest first, indexed git-style: branch@{0} is the most recent move.
    for (i, entry) in entries.iter().rev().enumerate() {
        let new_short = &entry.new_id[..entry.new_id.len().min(11)];
        let was = match &entry.old_id {
            Some(old) => format!("was {}", &old[..old.len().min(11)]),
            None => "first commit on branch".to_string(),
        };
        println!(
            "{new_short} {}@{{{i}}}: {}: {was}  ({})",
            entry.branch, entry.reason, entry.timestamp
        );
    }

    Ok(())
}
