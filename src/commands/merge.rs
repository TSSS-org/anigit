use anyhow::{bail, Result};
use std::env;
use std::io::{BufRead, Write};

use super::compare::{
    display_name, net_changes, open_catalog, print_comparison, status_label, EntryKey,
};
use crate::repo::commit::{CatalogRef, Changes, Commit, CommitAction};
use crate::repo::config::RepoKind;
use crate::repo::Repo;

/// `anigit merge <branch>` — merge `branch` into the current branch.
///
/// Gated by `repo_kind` (brainstorm.md 1.6): refuse outright on
/// `RepoKind::SingleUser` repos. On `RepoKind::Shared` repos, follows the
/// accepted conflict model from brainstorm.md 1.7:
///   - Objective/numeric fields (episode_progress, rewatch_count)
///     auto-resolve via max() — watching more is objectively "further
///     along" (Option B).
///   - Subjective fields (status, score) are real conflicts — never
///     silently auto-picked. Surfaced via the same comparison display as
///     `anigit compare` (Option D) and resolved by an explicit prompt.
///
/// Does NOT go through `.anigit/STAGED` — a merge commit is constructed
/// directly from the resolution, not from something staged via `anigit add`.
pub fn run(branch: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    let config = repo.config()?;
    if config.repo_kind == RepoKind::SingleUser {
        bail!(
            "merge is not allowed on a SingleUser repo (brainstorm.md 1.6).\n\
             Use `anigit compare <other_repo>` to see differences without \
             merging, or have the owner toggle repo_kind to Shared."
        );
    }

    let current = repo.current_branch()?;
    if branch == current {
        bail!("cannot merge branch '{branch}' into itself");
    }
    if !repo.branch_exists(branch) {
        bail!("no such branch: {branch}");
    }
    let Some(our_tip) = repo.branch_head(&current)? else {
        bail!("current branch '{current}' has no commits yet — nothing to merge into");
    };
    let Some(their_tip) = repo.branch_head(branch)? else {
        bail!("branch '{branch}' has no commits — nothing to merge");
    };
    if our_tip == their_tip || repo.common_ancestor(&current, branch)? == Some(their_tip.clone())
    {
        println!("Already up to date.");
        return Ok(());
    }
    let Some(ancestor) = repo.common_ancestor(&current, branch)? else {
        bail!(
            "branches '{current}' and '{branch}' share no common history — \
             refusing to merge disconnected branches"
        );
    };

    // Net state each side accumulated since the fork point (same replay
    // logic compare uses on full histories, anchored at the ancestor here).
    let our_net = net_changes(&commits_since(&repo, &current, &ancestor)?);
    let their_net = net_changes(&commits_since(&repo, branch, &ancestor)?);

    let catalog = open_catalog();

    // Only anime THEIR side touched need merge commits — if only our side
    // changed something, the merge brings in nothing new for it.
    let mut keys: Vec<EntryKey> = their_net.keys().cloned().collect();
    keys.sort();

    let mut resolved: Vec<(EntryKey, Changes)> = Vec::new();
    for key in keys {
        let name = display_name(catalog.as_ref(), &key);
        let merged = resolve_entry(
            &name,
            our_net.get(&key),
            &their_net[&key],
            &current,
            branch,
        )?;
        resolved.push((key, merged));
    }

    // The Commit shape holds exactly one catalog_ref (frozen data model,
    // brainstorm.md 1.3a), so a merge touching several anime becomes a
    // short chain: the first commit is the real merge (two parents,
    // action=Merge), each further anime lands as a follow-up commit on top.
    if resolved.is_empty() {
        // Their side's commits set no fields at all; still record the merge
        // point so future ancestor lookups know these histories joined.
        let their_ref = repo.read_commit(&their_tip)?.catalog_ref;
        resolved.push((
            (their_ref.source, their_ref.id),
            Changes::default(),
        ));
    }
    let mut prev_id = None;
    for (i, (key, changes)) in resolved.iter().enumerate() {
        let catalog_ref = CatalogRef {
            source: key.0.clone(),
            id: key.1,
        };
        let (parents, message) = match prev_id {
            None => (
                vec![our_tip.clone(), their_tip.clone()],
                format!("merge branch '{branch}' into {current}"),
            ),
            Some(prev) => (
                vec![prev],
                format!(
                    "merge branch '{branch}': resolve {}",
                    display_name(catalog.as_ref(), key)
                ),
            ),
        };
        let mut commit = Commit::new(parents, current.clone(), catalog_ref, changes.clone(), message);
        if i == 0 {
            commit.action = CommitAction::Merge;
        }
        repo.write_commit(&commit)?;
        println!(
            "[{current} {}] {}",
            &commit.id[..commit.id.len().min(11)],
            commit.message
        );
        prev_id = Some(commit.id);
    }

    Ok(())
}

/// Commits on `branch` between its tip (exclusive of nothing) and the common
/// ancestor (exclusive), newest first.
fn commits_since(repo: &Repo, branch: &str, ancestor: &str) -> Result<Vec<Commit>> {
    Ok(repo
        .history(branch)?
        .into_iter()
        .take_while(|c| c.id != ancestor)
        .collect())
}

/// Merge one anime's two sides into a resolved `Changes` per the Option B
/// rules: max() for objective fields, explicit prompt for subjective ones.
fn resolve_entry(
    name: &str,
    ours: Option<&Changes>,
    theirs: &Changes,
    our_branch: &str,
    their_branch: &str,
) -> Result<Changes> {
    let empty = Changes::default();
    let ours = ours.unwrap_or(&empty);

    // Lazily print the Option D comparison view once, before the first
    // prompt for this anime, so the user decides with full context.
    let mut context_shown = false;
    let mut show_context = || {
        if !context_shown {
            println!("\nCONFLICT in {name} — both branches changed subjective fields:");
            print_comparison(
                &format!("yours({our_branch})"),
                &format!("theirs({their_branch})"),
                ours,
                theirs,
            );
            context_shown = true;
        }
    };

    let status = match (ours.status, theirs.status) {
        (Some(o), Some(t)) if o != t => {
            show_context();
            Some(choose(
                "status",
                &status_label(o).to_string(),
                &status_label(t).to_string(),
                our_branch,
                their_branch,
            )?
            .pick(o, t))
        }
        (o, t) => o.or(t),
    };
    let score = match (ours.score, theirs.score) {
        (Some(o), Some(t)) if o != t => {
            show_context();
            Some(
                choose("score", &o.to_string(), &t.to_string(), our_branch, their_branch)?
                    .pick(o, t),
            )
        }
        (o, t) => o.or(t),
    };

    Ok(Changes {
        status,
        // Objective fields: further along wins, no prompt (Option B).
        episode_progress: max_opt(ours.episode_progress, theirs.episode_progress),
        score,
        rewatch_count: max_opt(ours.rewatch_count, theirs.rewatch_count),
    })
}

enum Side {
    Ours,
    Theirs,
}

impl Side {
    fn pick<T>(&self, ours: T, theirs: T) -> T {
        match self {
            Side::Ours => ours,
            Side::Theirs => theirs,
        }
    }
}

/// Prompt for one subjective-field conflict; loops until the user picks a
/// side explicitly — the merge never proceeds past a real conflict on its
/// own (brainstorm.md 1.7, Option D).
fn choose(
    field: &str,
    ours: &str,
    theirs: &str,
    our_branch: &str,
    their_branch: &str,
) -> Result<Side> {
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        print!("Keep which {field}? [1] yours ({our_branch}): {ours}  [2] theirs ({their_branch}): {theirs} > ");
        std::io::stdout().flush()?;
        let Some(line) = lines.next() else {
            bail!("no input available to resolve {field} conflict — merge aborted, nothing written");
        };
        match line?.trim() {
            "1" => return Ok(Side::Ours),
            "2" => return Ok(Side::Theirs),
            other => println!("Please enter 1 or 2 (got '{other}')."),
        }
    }
}

fn max_opt<T: Ord>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, None) => a,
        (None, b) => b,
    }
}
