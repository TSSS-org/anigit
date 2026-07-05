//! Interactive TUI for `anigit add` — the only anigit command with a TUI
//! (brainstorm.md 1.7a).
//!
//! Accepted interaction pattern (exact visual layout left to implementation
//! judgment per user's explicit sign-off — easy to change later):
//!   - Dropdown for fixed-choice fields (`status`).
//!   - Spinner/stepper for numeric fields (`episode_progress`, `score`):
//!     focus -> Enter to activate -> arrow keys adjust -> Enter to confirm
//!     and exit the control.
//!   - Free-text field for comments/message.
//!   - Must support both arrow-key navigation AND mouse clicks.
//!
//! Built on `ratatui` + `crossterm` (see Cargo.toml).

use anyhow::Result;

use crate::repo::commit::{Changes, WatchStatus};

/// Result of a completed `anigit add` session — the staged Changes to be
/// picked up by a subsequent `anigit commit`.
pub struct AddMenuResult {
    pub changes: Changes,
}

/// Launch the interactive add menu, pre-populated with `existing` state if
/// this anime already has entries in the current repo.
pub fn run_add_menu(_anime_title: &str, existing: Option<Changes>) -> Result<AddMenuResult> {
    // TODO: actual ratatui event loop. Sketch:
    // 1. Enter raw mode / alternate screen via crossterm.
    // 2. Render a form: status dropdown, episode_progress spinner, score
    //    spinner, free-text message field.
    // 3. Handle KeyEvent (arrow keys, Enter, Tab between fields, Esc to
    //    cancel) AND MouseEvent (click to focus a field / click dropdown
    //    options / click spinner up-down arrows).
    // 4. On confirm, restore terminal and return the collected Changes.
    let _ = existing;
    Ok(AddMenuResult {
        changes: Changes::default(),
    })
}

/// Placeholder for the WatchStatus dropdown's fixed option list, so the TUI
/// implementation doesn't need to hardcode this separately from the enum.
pub fn status_options() -> Vec<WatchStatus> {
    vec![
        WatchStatus::Planning,
        WatchStatus::Watching,
        WatchStatus::Completed,
        WatchStatus::Dropped,
    ]
}
