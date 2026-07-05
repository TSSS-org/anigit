use anyhow::{bail, Result};

/// `anigit add <anime name>` — the ONLY anigit command with an interactive
/// TUI (brainstorm.md 1.7a). Opens a menu for editing status/episode/score/
/// comments before a subsequent `anigit commit -m "..."` finalizes it.
///
/// Exact visual layout delegated to implementation-time judgment (user
/// confirmed this is low-stakes and easy to change later — see brainstorm.md
/// section 4). Accepted interaction pattern:
///   - dropdown for fixed-choice fields (status: watching/completed/
///     dropped/planning)
///   - spinner/stepper for numeric fields (episode_progress, score):
///     focus -> enter -> arrow keys adjust -> enter to confirm
///   - free-text field for comments/message
/// Must support both arrow-key navigation and mouse clicks.
///
/// See `crate::tui` for the actual widget implementation once built.
pub fn run(anime_name: &str) -> Result<()> {
    // TODO:
    // 1. Look up `anime_name` in the local SQLite catalog cache (fuzzy match,
    //    show a picker if multiple results).
    // 2. Launch the ratatui TUI (crate::tui::add_menu) pre-populated with
    //    any existing state for this anime in the current repo.
    // 3. On confirm, write the result to the staging area (same TODO as
    //    commands/commit.rs — staging-area format not yet decided).
    let _ = anime_name;
    bail!("anigit add: not yet implemented — see TODOs in commands/add.rs and src/tui/")
}
