use anyhow::Result;
use colored::Colorize;
use std::env;

use crate::cli::{ConfigAction, KindArg, VisibilityArg};
use crate::repo::config::{RepoKind, Visibility};
use crate::repo::Repo;

/// `anigit config` — view or toggle `repo_kind` and `visibility`
/// (brainstorm.md 1.6). Both are changeable at any time; there is no
/// gating on the current value (merge gating on repo_kind is merge.rs's
/// job, not this command's).
///
/// Owner-only enforcement is deliberately NOT faked here: `RepoConfig.owner`
/// is always "local-user" until AniHub/multi-user accounts exist (v2+), so
/// on a single machine there is no second identity to enforce against.
/// Real enforcement becomes meaningful — and gets built — with AniHub.
pub fn run(action: ConfigAction) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let mut config = repo.config()?;

    match action {
        ConfigAction::Show => {
            println!("{}  {:?}", "Repo kind:".cyan(), config.repo_kind);
            println!("{} {:?}", "Visibility:".cyan(), config.visibility);
            println!("{}      {}", "Owner:".cyan(), config.owner);
            if let Some(fork) = &config.forked_from {
                println!("{} {} (at {})", "Forked from:".cyan(), fork.source, fork.forked_at);
            }
            for remote in &config.remotes {
                println!("{}     {} -> {}", "Remote:".cyan(), remote.name, remote.url);
            }
        }
        ConfigAction::SetKind { kind } => {
            config.repo_kind = match kind {
                KindArg::Shared => RepoKind::Shared,
                KindArg::SingleUser => RepoKind::SingleUser,
            };
            repo.write_config(&config)?;
            println!("{}", format!("repo_kind set to {:?}.", config.repo_kind).green());
        }
        ConfigAction::SetVisibility { visibility } => {
            config.visibility = match visibility {
                VisibilityArg::PublicContributable => Visibility::PublicContributable,
                VisibilityArg::PublicViewOnly => Visibility::PublicViewOnly,
                VisibilityArg::Private => Visibility::Private,
                VisibilityArg::PrivateSharedWithSpecificPeople => {
                    Visibility::PrivateSharedWithSpecificPeople
                }
            };
            repo.write_config(&config)?;
            println!("{}", format!("visibility set to {:?}.", config.visibility).green());
        }
    }

    Ok(())
}
