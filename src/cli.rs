//! CLI surface for anigit.
//!
//! Command set matches the v1 scope confirmed in `brainstorm.md` section 4:
//! everything git can do fully offline, renamed `anigit <verb>`. No
//! anime-flavored aliases (e.g. no `watch` alias for `commit`) — see 1.7a
//! for the reasoning. `rebase`/`cherry-pick`/`reset`/`stash` are deferred to
//! v2 and deliberately not listed here yet.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anigit", version, about = "Git, but for your anime history.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new, empty anigit repo in the current directory.
    Init,

    /// Stage changes to an anime entry via an interactive menu.
    /// The only anigit command with a TUI — see brainstorm.md 1.7a.
    Add {
        /// Anime name to search the local catalog for.
        anime_name: String,
    },

    /// Record staged changes as a new commit. Plain flag-based, no TUI —
    /// mirrors `git commit -m "..."`.
    Commit {
        #[arg(short = 'm', long = "message")]
        message: String,

        /// Amend the most recent commit instead of creating a new one.
        #[arg(long)]
        amend: bool,
    },

    /// Show the working state: what's staged, current branch, etc.
    Status,

    /// Show commit history for the current branch.
    Log {
        #[arg(long)]
        oneline: bool,

        #[arg(long)]
        graph: bool,
    },

    /// Show changes between two commits (or a commit and the working state).
    Diff {
        from: Option<String>,
        to: Option<String>,
    },

    /// Show a single commit's full details.
    Show { commit_id: String },

    /// List, create branches.
    Branch {
        name: Option<String>,
    },

    /// Switch to an existing branch.
    Checkout {
        #[arg(short = 'b')]
        create: bool,
        branch: String,
    },

    /// Switch to an existing branch (alias-equivalent to modern git usage;
    /// kept as its own real command, not an alias — see 1.7a naming policy).
    Switch {
        #[arg(short = 'b')]
        create: bool,
        branch: String,
    },

    /// Merge another branch into the current one. Gated by repo_kind — see
    /// brainstorm.md 1.6 / 1.7.
    Merge { branch: String },

    /// Create a tag at the current commit.
    Tag { name: String },

    /// Copy an existing repo. `fork` is the same underlying operation but
    /// tagged with fork provenance — see the `Fork` command below.
    Clone { url: String, destination: Option<String> },

    /// anigit's own invented command (no real git equivalent — `fork` is a
    /// GitHub concept, not a git one). Functions like `clone` but tagged
    /// with fork status/provenance; will integrate with AniHub once that
    /// exists. See brainstorm.md 1.7a.
    Fork {
        url: String,
        destination: Option<String>,
    },

    /// Manage remote repo URLs (for future push/pull, v2+).
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },

    /// Push local commits to a remote (v2+, requires network/AniHub).
    Push { remote: Option<String> },

    /// Pull commits from a remote (v2+, requires network/AniHub).
    Pull { remote: Option<String> },

    /// Fetch from a remote without merging (v2+, requires network/AniHub).
    Fetch { remote: Option<String> },

    /// Show which commit last changed a given field for an anime entry.
    Blame { anime_name: String },

    /// Show a log of all ref updates (recovery/undo safety net).
    Reflog,

    /// Undo a commit by creating a new commit that reverses it (safe undo,
    /// compatible with the append-only history model).
    Revert { commit_id: String },

    /// anigit's own invented command (no git equivalent). Manual,
    /// human-driven comparison between two repos/lists for merge-conflict
    /// resolution on subjective fields — see brainstorm.md 1.7 (Option D).
    Compare { other_repo: String },

    /// anigit's own invented command (no git equivalent). Manually sync the
    /// local anime metadata catalog cache against the animetaScraper VM,
    /// pulling deltas/checkpoints rather than the whole catalog each time.
    /// See brainstorm.md 1.11-1.13a. NOTE: this is the one v1 command that
    /// requires network access — a deliberate, documented exception to "v1
    /// is fully offline," since it syncs shared catalog metadata rather than
    /// personal repo data.
    Refresh,
}

#[derive(Subcommand)]
pub enum RemoteAction {
    Add { name: String, url: String },
    Remove { name: String },
    List,
}
