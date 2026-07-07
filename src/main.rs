use anyhow::Result;
use clap::Parser;

use anigit::cli::{Cli, Command, RemoteAction};
use anigit::commands;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => commands::init::run(),
        Command::Add => commands::add::run(),
        Command::Commit { message, amend } => commands::commit::run(&message, amend),
        Command::Status => commands::status::run(),
        Command::Log { oneline, graph } => commands::log::run(oneline, graph),
        Command::Diff { from, to } => commands::diff::run(from.as_deref(), to.as_deref()),
        Command::Show { commit_id } => commands::show::run(&commit_id),
        Command::Branch {
            name,
            delete,
            force_delete,
        } => commands::branch::run(name.as_deref(), delete, force_delete),
        Command::Checkout { create, branch } => commands::checkout::run(&branch, create),
        Command::Switch { create, branch } => commands::checkout::run(&branch, create),
        Command::Merge { branch } => commands::merge::run(&branch),
        Command::Tag { name } => commands::tag::run(&name),
        Command::Clone { url, destination } => {
            commands::clone_fork::run_clone(&url, destination.as_deref())
        }
        Command::Fork { url, destination } => {
            commands::clone_fork::run_fork(&url, destination.as_deref())
        }
        Command::Remote { action } => match action {
            RemoteAction::Add { name, url } => commands::remote::add(&name, &url),
            RemoteAction::Remove { name } => commands::remote::remove(&name),
            RemoteAction::List => commands::remote::list(),
        },
        Command::Push { .. } | Command::Pull { .. } | Command::Fetch { .. } => {
            // Network sync sequencing: brainstorm.md 1.8.
            anyhow::bail!(
                "push/pull/fetch require network sync via AniHub, which is \
                 planned for v2+. Not available in v1."
            )
        }
        Command::Blame => commands::blame::run(),
        Command::Reflog => commands::reflog::run(),
        Command::Revert { commit_id } => commands::revert::run(&commit_id),
        Command::Compare { other_repo } => commands::compare::run(&other_repo),
        Command::Refresh => commands::refresh::run(),
        Command::Config { action } => commands::config::run(action),
        Command::Uninstall { confirm } => commands::uninstall::run(confirm),
    }
}
