use anyhow::{bail, Result};
use std::env;

use super::compare::{net_changes, status_label};
use crate::repo::commit::{Changes, Commit, CommitAction};
use crate::repo::Repo;

/// `anigit revert <commit_id>` — safe undo: creates a NEW commit that
/// reverses the given commit's changes, rather than deleting/rewriting
/// history. This is the only "undo" mechanism in v1 — `reset` (which would
/// rewrite history) is deferred to v2 and may end up restricted even then,
/// since it conflicts with the append-only philosophy (brainstorm.md 1.3,
/// 1.7a).
///
/// "The value before commit X" for each field = walk X's own ancestry
/// (from its parent backward, same catalog_ref) to the most recent prior
/// commit that set that field — blame.rs's search, anchored at X's parent
/// instead of the branch tip. Design choices, documented here:
///   - A field the target was FIRST to set has no prior value to restore.
///     The append-only event log has no "unset" concept (Changes fields are
///     optional; omitted means "not changed by this commit," and inventing
///     a reset-to-0 would be `reset`'s territory, v2). Such fields are
///     skipped with an explanatory note.
///   - Fields whose revert-to value already matches the branch's current
///     state are dropped, and if NOTHING remains the whole revert is
///     refused: an empty-changes commit would be pure log noise recording
///     no actual change.
///   - Merge commits (two parents) are refused outright: which parent's
///     side to revert to is genuinely ambiguous (real git needs an explicit
///     `-m <parent>` flag for this) — an honest v1 limitation, logged in
///     brainstorm.md section 4, rather than silently picking a side.
///   - The message is auto-generated (`Revert "<msg>" (<id>)`); no `-m`
///     override, since adding the flag would touch cli.rs/main.rs beyond
///     this part's scope and the auto message is always meaningful. The
///     reverted commit's ID is also recorded as `reverts` in `metadata`
///     (same open-ended-field pattern as merge_group_id).
pub fn run(commit_id: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let target = repo.read_commit(commit_id)?;

    if target.action == CommitAction::Merge || target.parent_ids.len() >= 2 {
        bail!(
            "reverting a merge commit isn't supported in v1 — with two \
             parents, which side to revert to is ambiguous (real git needs \
             an explicit `-m <parent>` choice for this). See brainstorm.md \
             section 4 for the logged limitation."
        );
    }

    let branch = repo.current_branch()?;
    let Some(head) = repo.branch_head(&branch)? else {
        bail!("branch '{branch}' has no commits — nothing to revert on");
    };

    // Prior state: the target's own ancestry (first-parent, like history()),
    // starting at its parent, filtered to the same anime. Newest first, so
    // find_map returns the most recent prior setting of a field.
    let mut prior: Vec<Commit> = Vec::new();
    let mut cursor = target.parent_ids.first().cloned();
    while let Some(id) = cursor {
        let commit = repo.read_commit(&id)?;
        cursor = commit.parent_ids.first().cloned();
        if commit.catalog_ref == target.catalog_ref {
            prior.push(commit);
        }
    }

    // Current branch state for this anime — used both to skip fields that
    // already match and to show "old -> restored" in the summary.
    let key = (target.catalog_ref.source.clone(), target.catalog_ref.id);
    let current = net_changes(&repo.history(&branch)?)
        .remove(&key)
        .unwrap_or_default();

    let mut restored = Changes::default();
    let mut summary: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    // One pass per Changes field: only fields the target actually set need
    // reverting — it never claimed to change the rest.
    macro_rules! revert_field {
        ($field:ident, $name:expr, $show:expr) => {
            if target.changes.$field.is_some() {
                let display = |v| ($show)(v);
                let current_str = current
                    .$field
                    .map(display)
                    .unwrap_or_else(|| "(unset)".to_string());
                match prior.iter().find_map(|c| c.changes.$field) {
                    None => notes.push(format!(
                        "  {}: skipped — {} was the first commit to ever set it, and \
                         append-only history has no way to un-set a field (that's \
                         reset's territory, deferred to v2)",
                        $name, target.id
                    )),
                    Some(value) if current.$field == Some(value) => notes.push(format!(
                        "  {}: already back at {} — nothing to change",
                        $name,
                        display(value)
                    )),
                    Some(value) => {
                        restored.$field = Some(value);
                        summary.push(format!(
                            "  {}: {} -> {}",
                            $name,
                            current_str,
                            display(value)
                        ));
                    }
                }
            }
        };
    }
    revert_field!(status, "status", |v| status_label(v).to_string());
    revert_field!(episode_progress, "episode_progress", |v: u32| v.to_string());
    revert_field!(score, "score", |v: u8| v.to_string());
    revert_field!(rewatch_count, "rewatch_count", |v: u32| v.to_string());

    if restored == Changes::default() {
        for note in &notes {
            println!("{note}");
        }
        bail!(
            "nothing to revert from {} — no field would actually change \
             (an empty revert commit would only add log noise)",
            target.id
        );
    }

    let short_target = &target.id[..target.id.len().min(11)];
    let mut commit = Commit::new(
        vec![head],
        branch.clone(),
        target.catalog_ref.clone(),
        restored,
        format!("Revert \"{}\" ({short_target})", target.message),
    );
    commit.metadata.insert(
        "reverts".to_string(),
        serde_json::Value::String(target.id.clone()),
    );
    repo.write_commit(&commit)?;

    println!(
        "[{branch} {}] {}",
        &commit.id[..commit.id.len().min(11)],
        commit.message
    );
    for line in summary.iter().chain(notes.iter()) {
        println!("{line}");
    }
    Ok(())
}
