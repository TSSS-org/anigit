//! CLI surface for anigit.
//!
//! Command set matches the v1 scope confirmed in `brainstorm.md` section 4:
//! everything git can do fully offline, renamed `anigit <verb>`. No
//! anime-flavored aliases (e.g. no `watch` alias for `commit`) — see 1.7a
//! for the reasoning. `rebase`/`cherry-pick`/`reset`/`stash` are deferred to
//! v2 and deliberately not listed here yet.
//!
//! NOTE: `///` doc comments in this file become user-facing `--help` text
//! via clap. Keep internal pointers (brainstorm.md section numbers, build
//! history) in plain `//` comments above the doc comment instead
//! (brainstorm.md 1.15).

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "anigit",
    version,
    about = "Git, but for your anime history.",
    disable_version_flag = true
)]
pub struct Cli {
    // Clap's auto-generated version flag only accepts `-V`; declaring it
    // manually lets lowercase `-v` work too (brainstorm.md 1.15). No
    // subcommand uses a top-level `-v`, so there's no collision.
    /// Print version
    #[arg(short = 'V', short_alias = 'v', long = "version", action = clap::ArgAction::Version)]
    version: Option<bool>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new, empty anigit repo in the current directory.
    Init,

    // The only anigit command with a TUI — see brainstorm.md 1.7a (updated
    // 2026-07-06: the `<anime name>` argument was removed; ambiguous
    // first-match resolution picked wrong entries in practice).
    /// Stage changes to an anime entry: an interactive search screen picks
    /// the anime (type to filter, arrows/mouse to choose), then the edit
    /// menu opens.
    Add,

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

    /// List, create, or delete branches.
    Branch {
        /// Delete the named branch. Refuses if the branch has commits not
        /// reachable from any other branch (they'd become orphaned).
        #[arg(short = 'd', conflicts_with = "force_delete", requires = "name")]
        delete: bool,

        /// Force-delete the named branch, even if its commits aren't
        /// reachable from any other branch.
        #[arg(short = 'D', requires = "name")]
        force_delete: bool,

        name: Option<String>,
    },

    /// Switch to an existing branch.
    Checkout {
        #[arg(short = 'b')]
        create: bool,
        branch: String,
    },

    // Kept as its own real command, not an alias — see brainstorm.md 1.7a
    // naming policy.
    /// Switch to an existing branch (alias-equivalent to modern git usage).
    Switch {
        #[arg(short = 'b')]
        create: bool,
        branch: String,
    },

    // Gating rationale: brainstorm.md 1.6 / 1.7.
    /// Merge another branch into the current one. Only allowed on shared
    /// repos (single-user repos are fork-only).
    Merge { branch: String },

    /// Create a tag at the current commit.
    Tag { name: String },

    /// Copy an existing repo. `fork` is the same underlying operation but
    /// tagged with fork provenance — see the `fork` command.
    Clone { url: String, destination: Option<String> },

    // See brainstorm.md 1.7a.
    /// anigit's own invented command (no real git equivalent — `fork` is a
    /// GitHub concept, not a git one). Functions like `clone` but tagged
    /// with fork status/provenance; will integrate with AniHub once that
    /// exists.
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

    // The `<anime name>` argument was removed 2026-07-06 (brainstorm.md
    // 1.15) for the same reason `add` lost its argument in uxFix1:
    // ambiguous first-match resolution picked wrong entries.
    /// Show which commit last changed each field for an anime entry: an
    /// interactive search screen picks the anime, scoped to anime this
    /// repo's history has actually tracked.
    Blame,

    /// Show a log of all ref updates (recovery/undo safety net).
    Reflog,

    /// Undo a commit by creating a new commit that reverses it (safe undo,
    /// compatible with the append-only history model).
    Revert { commit_id: String },

    // See brainstorm.md 1.7 (Option D).
    /// anigit's own invented command (no git equivalent). Manual,
    /// human-driven comparison between two repos/lists for merge-conflict
    /// resolution on subjective fields.
    Compare { other_repo: String },

    // See brainstorm.md 1.11-1.13a for the delta/checkpoint sync design and
    // the reasoning behind the offline exception.
    /// anigit's own invented command (no git equivalent). Manually sync the
    /// local anime metadata catalog cache against the metadata server,
    /// pulling deltas/checkpoints rather than the whole catalog each time.
    /// NOTE: this is the one v1 command that requires network access — a
    /// deliberate exception to "v1 is fully offline," since it syncs shared
    /// catalog metadata rather than personal repo data.
    Refresh,

    // Settings scope per brainstorm.md 1.6. (Added in part 7 of the v1
    // build; earlier parts had to hand-edit .anigit/config to test merge
    // gating.)
    /// View or change repo settings: the shared/single-user repo kind
    /// toggle and the visibility level. Deliberately narrower than real
    /// `git config` — just these two settings, not arbitrary key-value
    /// pairs.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show repo_kind, visibility, owner, and fork provenance (if any).
    Show,
    // Toggleable at any time by the owner (brainstorm.md 1.6). value_name
    // spells out the valid values so they appear in the usage line itself —
    // including the missing-argument error — instead of only after an
    // invalid attempt (brainstorm.md 1.15).
    /// Set the repo kind. Toggleable at any time by the owner.
    SetKind {
        #[arg(value_enum, value_name = "shared|single-user")]
        kind: KindArg,
    },
    // Same value_name reasoning as set-kind above (brainstorm.md 1.15).
    /// Set the visibility level. Toggleable at any time by the owner.
    SetVisibility {
        #[arg(
            value_enum,
            value_name = "public-contributable|public-view-only|private|private-shared-with-specific-people"
        )]
        visibility: VisibilityArg,
    },
}

/// CLI-layer mirrors of `repo::config::RepoKind`/`Visibility`, so clap can
/// parse/validate/tab-complete them without the repo data model needing to
/// know about clap.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum KindArg {
    Shared,
    SingleUser,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum VisibilityArg {
    PublicContributable,
    PublicViewOnly,
    Private,
    PrivateSharedWithSpecificPeople,
}

#[derive(Subcommand)]
pub enum RemoteAction {
    Add { name: String, url: String },
    Remove { name: String },
    List,
}
