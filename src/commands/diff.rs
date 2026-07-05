use anyhow::{bail, Result};
use std::env;

use crate::repo::commit::{CatalogRef, Changes, WatchStatus};
use crate::repo::Repo;

/// `anigit diff [from] [to]` — show changes between two commits, or (with no
/// args) between the current branch head and whatever is staged in
/// `.anigit/STAGED`.
///
/// Deliberately NOT a real 3-way/common-ancestor diff yet: with both args
/// given, this compares the two commits' `changes` fields directly,
/// field by field. Good enough for the append-only event-log model
/// (brainstorm.md 1.3) where each commit is already a small delta.
pub fn run(from: Option<&str>, to: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    match (from, to) {
        // Two commits: compare their changes fields directly.
        (Some(from_id), Some(to_id)) => {
            let a = repo.read_commit(from_id)?;
            let b = repo.read_commit(to_id)?;
            println!("diff {} {}", a.id, b.id);
            print_diff(&a.catalog_ref, &a.changes, &b.catalog_ref, &b.changes);
        }
        // One commit given: compare it against the staging area.
        (Some(from_id), None) => {
            let a = repo.read_commit(from_id)?;
            let Some(staged) = repo.read_staged()? else {
                println!("Nothing staged — no changes to compare against {from_id}.");
                return Ok(());
            };
            println!("diff {} STAGED ({})", a.id, staged.anime_title);
            print_diff(&a.catalog_ref, &a.changes, &staged.catalog_ref, &staged.changes);
        }
        // No args: current branch head vs. the staging area.
        (None, None) => {
            let branch = repo.current_branch()?;
            let Some(staged) = repo.read_staged()? else {
                println!("Nothing staged — no changes to show.");
                return Ok(());
            };
            let Some(head_id) = repo.branch_head(&branch)? else {
                println!("No commits yet on branch '{branch}'; staged changes:");
                for line in changes_lines(&staged.changes) {
                    println!("+ {line}");
                }
                return Ok(());
            };
            let head = repo.read_commit(&head_id)?;
            println!("diff {head_id} STAGED ({})", staged.anime_title);
            print_diff(&head.catalog_ref, &head.changes, &staged.catalog_ref, &staged.changes);
        }
        // clap fills positionals in order, so `to` without `from` can't happen.
        (None, Some(_)) => bail!("diff: cannot give <to> without <from>"),
    }

    Ok(())
}

/// Print a field-by-field comparison of two `Changes` deltas.
fn print_diff(from_ref: &CatalogRef, from: &Changes, to_ref: &CatalogRef, to: &Changes) {
    if from_ref != to_ref {
        println!(
            "anime: {}/{} -> {}/{}",
            from_ref.source, from_ref.id, to_ref.source, to_ref.id
        );
    }

    let lines: Vec<String> = [
        field_line("status", from.status.map(status_str), to.status.map(status_str)),
        field_line(
            "episode_progress",
            from.episode_progress.map(|v| v.to_string()),
            to.episode_progress.map(|v| v.to_string()),
        ),
        field_line(
            "score",
            from.score.map(|v| v.to_string()),
            to.score.map(|v| v.to_string()),
        ),
        field_line(
            "rewatch_count",
            from.rewatch_count.map(|v| v.to_string()),
            to.rewatch_count.map(|v| v.to_string()),
        ),
    ]
    .into_iter()
    .flatten()
    .collect();

    if lines.is_empty() && from_ref == to_ref {
        println!("No changes.");
    } else {
        for line in lines {
            println!("{line}");
        }
    }
}

/// One diff line for a single field, or `None` if both sides agree.
fn field_line(
    name: &str,
    from: Option<impl ToString>,
    to: Option<impl ToString>,
) -> Option<String> {
    let from = from.map(|v| v.to_string());
    let to = to.map(|v| v.to_string());
    if from == to {
        return None;
    }
    Some(format!(
        "  {name}: {} -> {}",
        from.as_deref().unwrap_or("(unset)"),
        to.as_deref().unwrap_or("(unset)")
    ))
}

/// Human-readable lines for the fields set in a `Changes` delta. Shared with
/// `anigit status` for its "Changes staged for commit" section.
pub fn changes_lines(changes: &Changes) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(status) = changes.status {
        lines.push(format!("status: {}", status_str(status)));
    }
    if let Some(ep) = changes.episode_progress {
        lines.push(format!("episode_progress: {ep}"));
    }
    if let Some(score) = changes.score {
        lines.push(format!("score: {score}"));
    }
    if let Some(rw) = changes.rewatch_count {
        lines.push(format!("rewatch_count: {rw}"));
    }
    lines
}

/// Same snake_case names the on-disk JSON uses (commit.rs serde rename).
fn status_str(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Dropped => "dropped",
        WatchStatus::Planning => "planning",
        WatchStatus::Watching => "watching",
        WatchStatus::Completed => "completed",
    }
}
