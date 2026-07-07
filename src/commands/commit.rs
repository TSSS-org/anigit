use anyhow::{bail, Result};
use std::env;

use crate::repo::commit::Commit;
use crate::repo::Repo;

/// `anigit commit -m "message"` — plain, flag-based, no TUI (brainstorm.md
/// 1.7a). Finalizes whatever was staged by a prior `anigit add` call into a
/// real commit object, then clears the staging area.
pub fn run(message: &str, amend: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    let Some(staged) = repo.read_staged()? else {
        bail!(
            "nothing staged to commit.\n\
             Use `anigit add <anime name>` to stage changes first."
        );
    };

    let branch = repo.current_branch()?;
    let head = repo.branch_head(&branch)?;

    // A normal commit chains onto the current branch head (empty parent list
    // only for the repo's very first commit). `--amend` instead reuses the
    // head's OWN parents, so the new commit replaces the old tip in the
    // branch chain — honest about append-only history (brainstorm.md 1.3):
    // the old commit file stays on disk untouched, it just stops being
    // reachable from the branch ref.
    let parent_ids = if amend {
        let Some(head_id) = head else {
            bail!("cannot --amend: branch '{branch}' has no commits yet");
        };
        repo.read_commit(&head_id)?.parent_ids
    } else {
        head.into_iter().collect()
    };

    let commit = Commit::new(
        parent_ids,
        branch.clone(),
        staged.catalog_ref,
        staged.changes,
        message,
    );
    repo.write_commit(&commit)?;
    repo.clear_staged()?;

    // Regenerate the working-directory folder-tree view (brainstorm.md
    // 1.16). Both the normal and --amend paths converge on the single
    // write_commit call above, so this one hook covers both. Failure here
    // is a WARNING, not a command failure: the commit — the real data —
    // already landed safely in .anigit/, and a broken derived view
    // shouldn't mask that success or make the user think the commit
    // didn't happen.
    if let Err(err) = crate::tree::regenerate(&repo) {
        println!("warning: commit succeeded, but regenerating the folder-tree view failed: {err:#}");
    }

    println!(
        "[{branch} {}] {message}",
        &commit.id[..commit.id.len().min(11)]
    );
    println!(
        " {} '{}'",
        if amend { "amended tip with" } else { "committed" },
        staged.anime_title
    );
    Ok(())
}
