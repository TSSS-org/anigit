//! Interactive TUI for `anigit add` — the only anigit command with a TUI
//! (brainstorm.md 1.7a).
//!
//! Accepted interaction pattern (exact visual layout left to implementation
//! judgment per user's explicit sign-off — easy to change later):
//!   - Dropdown for fixed-choice fields (`status`).
//!   - Spinner/stepper for numeric fields (`episode_progress`, `score`):
//!     focus -> Enter to activate -> arrow keys adjust -> Enter to confirm
//!     and exit the control. `rewatch_count` wasn't named in 1.7a's list of
//!     numeric fields but is part of `Changes` and behaves identically, so
//!     it gets the same spinner treatment.
//!   - Supports both arrow-key navigation AND mouse clicks (click a field
//!     to focus it, click `[-]`/`[+]` to step, click a dropdown option to
//!     select it, click `[ Save ]` to submit).
//!   - No free-text message field: per 1.7a the commit message is supplied
//!     later by `anigit commit -m "..."` — this menu only collects
//!     `Changes` fields.
//!
//! Structure note: all interaction state lives in [`AddForm`], a plain data
//! struct whose `handle_key`/`handle_mouse` transitions are pure and unit
//! tested below without a terminal. [`compute_regions`] is the single
//! source of layout truth shared by rendering and mouse hit-testing, so the
//! two can't drift apart. The actual terminal plumbing is confined to
//! `run_add_menu`/`event_loop`/`draw`.
//!
//! Scoring convention: 0-10 (whole numbers), matching the draft commit
//! example in brainstorm.md 1.3a (`"score": 8`) — not the 0-100 AniList
//! scale.

pub mod search;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::{Frame, Terminal};
use std::io;

use crate::repo::commit::{Changes, WatchStatus};

/// Result of a completed `anigit add` session — the staged Changes to be
/// picked up by a subsequent `anigit commit`.
pub struct AddMenuResult {
    pub changes: Changes,
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

fn status_name(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Dropped => "dropped",
        WatchStatus::Planning => "planning",
        WatchStatus::Watching => "watching",
        WatchStatus::Completed => "completed",
    }
}

// ---------------------------------------------------------------------------
// Form state (plain data, no terminal involved)
// ---------------------------------------------------------------------------

/// Field/focus indices, in visual top-to-bottom order.
const STATUS: usize = 0;
const EPISODE: usize = 1;
const SCORE: usize = 2;
const REWATCH: usize = 3;
const SAVE: usize = 4;
const FOCUS_COUNT: usize = 5;

/// 0-10 scoring scale — see module doc comment.
const SCORE_MAX: u8 = 10;
const EPISODE_MAX: u32 = 99_999;
const REWATCH_MAX: u32 = 9_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Moving focus between fields.
    Nav,
    /// The status dropdown is open, with one option highlighted.
    Dropdown { highlighted: usize },
    /// A numeric field is in adjustment mode (Enter'd into).
    Spinner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Submit,
    Cancel,
}

struct AddForm {
    title: String,
    status: Option<WatchStatus>,
    episode: Option<u32>,
    score: Option<u8>,
    rewatch: Option<u32>,
    focus: usize,
    mode: Mode,
    /// The selected anime's real episode count (`CatalogEntry.episodes`) —
    /// the spinner's cap, so progress can't exceed the show's actual length.
    /// `None` (count unknown/unconfirmed, e.g. currently airing) falls back
    /// to the EPISODE_MAX safety ceiling.
    episode_max: Option<u32>,
    /// State at open time (from prior history). `changes()` only reports
    /// fields that now differ from this, so an untouched pre-populated form
    /// stages an empty delta — matching 1.3a's "a commit only records what
    /// actually changed."
    baseline: Changes,
}

impl AddForm {
    fn new(title: &str, existing: Option<Changes>, episode_max: Option<u32>) -> Self {
        let baseline = existing.unwrap_or_default();
        Self {
            title: title.to_string(),
            status: baseline.status,
            episode: baseline.episode_progress,
            score: baseline.score,
            rewatch: baseline.rewatch_count,
            focus: STATUS,
            mode: Mode::Nav,
            episode_max,
            baseline,
        }
    }

    /// The delta to stage: only fields that differ from the opening state.
    /// Clearing a previously-set field records nothing — the append-only
    /// event log has no "unset" concept, a field just stops being changed.
    fn changes(&self) -> Changes {
        let b = &self.baseline;
        Changes {
            status: self.status.filter(|_| self.status != b.status),
            episode_progress: self.episode.filter(|_| self.episode != b.episode_progress),
            score: self.score.filter(|_| self.score != b.score),
            rewatch_count: self.rewatch.filter(|_| self.rewatch != b.rewatch_count),
        }
    }

    fn dropdown_open(&self) -> bool {
        matches!(self.mode, Mode::Dropdown { .. })
    }

    /// Step the focused numeric field by `delta`, clamping to its bounds.
    /// Stepping an unset field starts it at 0 (then applies the step).
    fn bump(&mut self, delta: i64) {
        fn step_u32(current: Option<u32>, delta: i64, max: u32) -> Option<u32> {
            Some((current.unwrap_or(0) as i64 + delta).clamp(0, max as i64) as u32)
        }
        match self.focus {
            // The real per-anime episode count caps progress when known;
            // EPISODE_MAX stays as the absolute safety ceiling either way.
            EPISODE => {
                let cap = self.episode_max.unwrap_or(EPISODE_MAX).min(EPISODE_MAX);
                self.episode = step_u32(self.episode, delta, cap);
            }
            REWATCH => self.rewatch = step_u32(self.rewatch, delta, REWATCH_MAX),
            SCORE => {
                let next = (self.score.unwrap_or(0) as i64 + delta).clamp(0, SCORE_MAX as i64);
                self.score = Some(next as u8);
            }
            _ => {}
        }
    }

    fn clear_focused(&mut self) {
        match self.focus {
            STATUS => self.status = None,
            EPISODE => self.episode = None,
            SCORE => self.score = None,
            REWATCH => self.rewatch = None,
            _ => {}
        }
    }

    fn open_dropdown(&mut self) {
        let highlighted = self
            .status
            .and_then(|s| status_options().iter().position(|o| *o == s))
            .unwrap_or(0);
        self.mode = Mode::Dropdown { highlighted };
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Outcome> {
        match self.mode {
            Mode::Dropdown { highlighted } => {
                let count = status_options().len();
                match key.code {
                    KeyCode::Up => {
                        self.mode = Mode::Dropdown {
                            highlighted: highlighted.checked_sub(1).unwrap_or(count - 1),
                        }
                    }
                    KeyCode::Down => {
                        self.mode = Mode::Dropdown {
                            highlighted: (highlighted + 1) % count,
                        }
                    }
                    KeyCode::Enter => {
                        self.status = Some(status_options()[highlighted]);
                        self.mode = Mode::Nav;
                    }
                    KeyCode::Esc => self.mode = Mode::Nav,
                    _ => {}
                }
            }
            Mode::Spinner => match key.code {
                KeyCode::Up | KeyCode::Right | KeyCode::Char('+') => self.bump(1),
                KeyCode::Down | KeyCode::Left | KeyCode::Char('-') => self.bump(-1),
                KeyCode::Enter | KeyCode::Esc => self.mode = Mode::Nav,
                _ => {}
            },
            Mode::Nav => {
                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    return Some(Outcome::Submit);
                }
                match key.code {
                    KeyCode::Up | KeyCode::BackTab => {
                        self.focus = self.focus.checked_sub(1).unwrap_or(FOCUS_COUNT - 1);
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        self.focus = (self.focus + 1) % FOCUS_COUNT;
                    }
                    // Left/Right step a focused numeric field directly, no
                    // spinner mode needed — a keyboard shortcut on top of
                    // the 1.7a pattern, not a replacement for it.
                    KeyCode::Left => self.bump(-1),
                    KeyCode::Right => self.bump(1),
                    KeyCode::Enter => match self.focus {
                        STATUS => self.open_dropdown(),
                        EPISODE | SCORE | REWATCH => {
                            self.bump(0); // an unset field becomes Some(0)
                            self.mode = Mode::Spinner;
                        }
                        SAVE => return Some(Outcome::Submit),
                        _ => {}
                    },
                    KeyCode::Delete | KeyCode::Backspace => self.clear_focused(),
                    KeyCode::Esc => return Some(Outcome::Cancel),
                    _ => {}
                }
            }
        }
        None
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, regions: &FormRegions) -> Option<Outcome> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return None;
        }
        let pos = Position::new(mouse.column, mouse.row);

        // While the dropdown is open it owns the mouse: an option selects,
        // anywhere else just closes it.
        if self.dropdown_open() {
            if let Some(i) = regions
                .dropdown_items
                .iter()
                .position(|r| r.contains(pos))
            {
                self.status = Some(status_options()[i]);
            }
            self.mode = Mode::Nav;
            return None;
        }

        for field in [EPISODE, SCORE, REWATCH] {
            if regions.dec[field].is_some_and(|r| r.contains(pos)) {
                self.focus = field;
                self.bump(-1);
                return None;
            }
            if regions.inc[field].is_some_and(|r| r.contains(pos)) {
                self.focus = field;
                self.bump(1);
                return None;
            }
        }
        if regions.save.contains(pos) {
            return Some(Outcome::Submit);
        }
        if let Some(field) = regions.rows.iter().position(|r| r.contains(pos)) {
            self.focus = field;
            self.mode = Mode::Nav;
            if field == STATUS {
                self.open_dropdown();
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Layout (pure — shared by rendering and mouse hit-testing)
// ---------------------------------------------------------------------------

/// Column (relative to the block's inner area) where field values start;
/// everything left of it is the focus marker + label.
const VALUE_COL: u16 = 20;
/// Numeric value layout within the value column: `[-] <value11> [+]`.
/// 11 chars fits "episode/cap" displays like "1100/1122" and the worst-case
/// "99999/99999" exactly, keeping the `[+]` cell aligned with its hit region.
const VAL_W: u16 = 11;
const INC_OFF: u16 = 16;
const BTN_W: u16 = 3;
const DROPDOWN_W: u16 = 16;

struct FormRegions {
    block: Rect,
    /// One full-width row per field, top-to-bottom field order.
    rows: [Rect; 4],
    /// `[-]` / `[+]` cells for the numeric fields (None for STATUS).
    dec: [Option<Rect>; 4],
    inc: [Option<Rect>; 4],
    save: Rect,
    footer: Rect,
    /// One rect per status option while the dropdown is open, else empty.
    dropdown_items: Vec<Rect>,
}

fn compute_regions(area: Rect, dropdown_open: bool) -> FormRegions {
    // Center the (fixed-size) form in whatever area the terminal reports on
    // this draw — recomputed every redraw, so resizing re-centers for free.
    let width = area.width.min(58);
    let height = area.height.min(9);
    let block = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let inner = Rect::new(
        block.x + 1,
        block.y + 1,
        block.width.saturating_sub(2),
        block.height.saturating_sub(2),
    );
    let row = |i: u16| Rect::new(inner.x, inner.y + i, inner.width, 1);
    let rows = [row(0), row(1), row(2), row(3)];
    let value_x = inner.x + VALUE_COL;

    let mut dec = [None; 4];
    let mut inc = [None; 4];
    for field in [EPISODE, SCORE, REWATCH] {
        let y = rows[field].y;
        dec[field] = Some(Rect::new(value_x, y, BTN_W, 1));
        inc[field] = Some(Rect::new(value_x + INC_OFF, y, BTN_W, 1));
    }

    let dropdown_items = if dropdown_open {
        (0..status_options().len() as u16)
            .map(|i| Rect::new(value_x, rows[STATUS].y + 1 + i, DROPDOWN_W, 1))
            .collect()
    } else {
        Vec::new()
    };

    FormRegions {
        block,
        rows,
        dec,
        inc,
        save: Rect::new(value_x, inner.y + 5, 8, 1),
        footer: Rect::new(
            block.x,
            block.y + block.height,
            area.width.saturating_sub(block.x - area.x),
            1,
        ),
        dropdown_items,
    }
}

// ---------------------------------------------------------------------------
// Terminal plumbing + rendering
// ---------------------------------------------------------------------------

/// Launch the interactive add menu, pre-populated with `existing` state if
/// this anime already has entries in the current repo. `episode_max` is the
/// anime's real episode count (`CatalogEntry.episodes`), capping the episode
/// spinner; `None` means the count is unknown. Returns `None` if the user
/// cancelled (Esc) — the caller must not stage anything in that case.
pub fn run_add_menu(
    anime_title: &str,
    existing: Option<Changes>,
    episode_max: Option<u32>,
) -> Result<Option<AddMenuResult>> {
    let mut form = AddForm::new(anime_title, existing, episode_max);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let outcome = event_loop(&mut terminal, &mut form);

    // Always restore the terminal, even if the loop errored.
    disable_raw_mode().ok();
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();

    Ok(match outcome? {
        Outcome::Submit => Some(AddMenuResult {
            changes: form.changes(),
        }),
        Outcome::Cancel => None,
    })
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    form: &mut AddForm,
) -> Result<Outcome> {
    loop {
        let mut regions = None;
        terminal.draw(|frame| regions = Some(draw(frame, form)))?;
        let regions = regions.expect("draw closure always runs");

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if let Some(outcome) = form.handle_key(key) {
                    return Ok(outcome);
                }
            }
            Event::Mouse(mouse) => {
                if let Some(outcome) = form.handle_mouse(mouse, &regions) {
                    return Ok(outcome);
                }
            }
            _ => {}
        }
    }
}

fn draw(frame: &mut Frame, form: &AddForm) -> FormRegions {
    let area = frame.area();
    let regions = compute_regions(area, form.dropdown_open());

    let block = Block::bordered().title(format!(" anigit add — {} ", form.title));
    render_clamped(frame, block, regions.block, area);

    let labels = ["Status", "Episode progress", "Score (0-10)", "Rewatch count"];
    for (field, label) in labels.iter().enumerate() {
        let focused = form.focus == field;
        let marker = if focused { "› " } else { "  " };
        let label_style = if focused {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let mut spans = vec![
            Span::styled(format!("{marker}{label:<18}"), label_style),
        ];
        if field == STATUS {
            let value = form.status.map(status_name).unwrap_or("(unset)");
            spans.push(Span::styled(
                format!("{value} ▾"),
                value_style(focused, form.dropdown_open()),
            ));
        } else {
            let value = match field {
                // Show the real cap next to episode progress when known,
                // e.g. "12/24" — makes the clamp visible, not mysterious.
                EPISODE => form.episode.map(|v| match form.episode_max {
                    Some(max) => format!("{v}/{max}"),
                    None => v.to_string(),
                }),
                SCORE => form.score.map(|v| v.to_string()),
                _ => form.rewatch.map(|v| v.to_string()),
            };
            let active = focused && form.mode == Mode::Spinner;
            spans.push(Span::styled("[-]", button_style(focused)));
            spans.push(Span::styled(
                format!(" {:^w$} ", value.as_deref().unwrap_or("(unset)"), w = VAL_W as usize),
                value_style(focused, active),
            ));
            spans.push(Span::styled("[+]", button_style(focused)));
        }
        render_clamped(frame, Paragraph::new(Line::from(spans)), regions.rows[field], area);
    }

    let save_style = if form.focus == SAVE {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    render_clamped(
        frame,
        Paragraph::new(Span::styled("[ Save ]", save_style)),
        regions.save,
        area,
    );

    let hint = match form.mode {
        Mode::Nav => "↑/↓ move · Enter edit · ←/→ step · Del clear · Ctrl+S/Save submit · Esc cancel",
        Mode::Dropdown { .. } => "↑/↓ choose · Enter select · Esc close",
        Mode::Spinner => "↑/↓ adjust · Enter done",
    };
    render_clamped(
        frame,
        Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        regions.footer,
        area,
    );

    if let Mode::Dropdown { highlighted } = form.mode {
        for (i, (option, rect)) in status_options()
            .iter()
            .zip(&regions.dropdown_items)
            .enumerate()
        {
            let style = if i == highlighted {
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::REVERSED)
            };
            render_clamped(frame, Clear, *rect, area);
            render_clamped(
                frame,
                Paragraph::new(Span::styled(
                    format!(" {:<w$}", status_name(*option), w = DROPDOWN_W as usize - 1),
                    style,
                )),
                *rect,
                area,
            );
        }
    }

    regions
}

fn value_style(focused: bool, active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else if focused {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn button_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Render only the part of `rect` that fits the frame — tiny terminals must
/// degrade gracefully rather than panic on out-of-bounds rects.
fn render_clamped<W: ratatui::widgets::Widget>(frame: &mut Frame, widget: W, rect: Rect, area: Rect) {
    let clamped = rect.intersection(area);
    if !clamped.is_empty() {
        frame.render_widget(widget, clamped);
    }
}

// ---------------------------------------------------------------------------
// Tests — pure state-transition coverage, no terminal required
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn press(form: &mut AddForm, codes: &[KeyCode]) -> Option<Outcome> {
        let mut outcome = None;
        for code in codes {
            outcome = form.handle_key(key(*code));
        }
        outcome
    }

    fn click(form: &mut AddForm, regions: &FormRegions, rect: Rect) -> Option<Outcome> {
        form.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: rect.x,
                row: rect.y,
                modifiers: KeyModifiers::NONE,
            },
            regions,
        )
    }

    #[test]
    fn prepopulates_from_existing_state() {
        let existing = Changes {
            status: Some(WatchStatus::Watching),
            episode_progress: Some(12),
            score: Some(8),
            rewatch_count: None,
        };
        let form = AddForm::new("NGE", Some(existing), None);
        assert_eq!(form.status, Some(WatchStatus::Watching));
        assert_eq!(form.episode, Some(12));
        assert_eq!(form.score, Some(8));
        assert_eq!(form.rewatch, None);
        // Untouched form stages an empty delta (only what changed, 1.3a).
        assert_eq!(form.changes(), Changes::default());
    }

    #[test]
    fn dropdown_open_navigate_select() {
        let mut form = AddForm::new("x", None, None);
        assert_eq!(form.focus, STATUS);
        press(&mut form, &[KeyCode::Enter]);
        assert_eq!(form.mode, Mode::Dropdown { highlighted: 0 });
        press(&mut form, &[KeyCode::Down]);
        assert_eq!(form.mode, Mode::Dropdown { highlighted: 1 });
        press(&mut form, &[KeyCode::Enter]);
        assert_eq!(form.mode, Mode::Nav);
        assert_eq!(form.status, Some(WatchStatus::Watching));
    }

    #[test]
    fn dropdown_esc_closes_without_change() {
        let mut form = AddForm::new("x", None, None);
        press(&mut form, &[KeyCode::Enter, KeyCode::Down, KeyCode::Esc]);
        assert_eq!(form.mode, Mode::Nav);
        assert_eq!(form.status, None);
    }

    #[test]
    fn spinner_activate_adjust_confirm() {
        let mut form = AddForm::new("x", None, None);
        press(&mut form, &[KeyCode::Down]); // focus episode
        assert_eq!(form.focus, EPISODE);
        press(&mut form, &[KeyCode::Enter]);
        assert_eq!(form.mode, Mode::Spinner);
        assert_eq!(form.episode, Some(0)); // activating an unset field starts at 0
        press(&mut form, &[KeyCode::Up, KeyCode::Up, KeyCode::Up]);
        assert_eq!(form.episode, Some(3));
        press(&mut form, &[KeyCode::Enter]);
        assert_eq!(form.mode, Mode::Nav);
    }

    #[test]
    fn spinner_bounds_clamp() {
        let mut form = AddForm::new("x", None, None);
        form.focus = SCORE;
        press(&mut form, &[KeyCode::Enter, KeyCode::Down, KeyCode::Down]);
        assert_eq!(form.score, Some(0)); // can't go negative
        for _ in 0..15 {
            press(&mut form, &[KeyCode::Up]);
        }
        assert_eq!(form.score, Some(SCORE_MAX)); // clamps at 10
    }

    #[test]
    fn clear_and_cancel() {
        let existing = Changes {
            status: Some(WatchStatus::Completed),
            ..Default::default()
        };
        let mut form = AddForm::new("x", Some(existing), None);
        press(&mut form, &[KeyCode::Delete]);
        assert_eq!(form.status, None);
        assert_eq!(press(&mut form, &[KeyCode::Esc]), Some(Outcome::Cancel));
    }

    #[test]
    fn changes_reports_only_deltas_from_baseline() {
        let existing = Changes {
            status: Some(WatchStatus::Watching),
            episode_progress: Some(12),
            score: Some(8),
            rewatch_count: None,
        };
        let mut form = AddForm::new("x", Some(existing), None);
        // Bump score 8 -> 9; leave everything else untouched.
        form.focus = SCORE;
        press(&mut form, &[KeyCode::Enter, KeyCode::Up, KeyCode::Enter]);
        let changes = form.changes();
        assert_eq!(changes.score, Some(9));
        assert_eq!(changes.status, None);
        assert_eq!(changes.episode_progress, None);
        assert_eq!(changes.rewatch_count, None);
    }

    #[test]
    fn mouse_spinner_buttons_and_save() {
        let mut form = AddForm::new("x", None, None);
        let regions = compute_regions(Rect::new(0, 0, 60, 20), false);
        click(&mut form, &regions, regions.inc[EPISODE].unwrap());
        click(&mut form, &regions, regions.inc[EPISODE].unwrap());
        assert_eq!(form.focus, EPISODE);
        assert_eq!(form.episode, Some(2));
        click(&mut form, &regions, regions.dec[EPISODE].unwrap());
        assert_eq!(form.episode, Some(1));
        assert_eq!(
            click(&mut form, &regions, regions.save),
            Some(Outcome::Submit)
        );
    }

    #[test]
    fn mouse_dropdown_flow() {
        let mut form = AddForm::new("x", None, None);
        let closed = compute_regions(Rect::new(0, 0, 60, 20), false);
        click(&mut form, &closed, closed.rows[STATUS]); // click status field opens dropdown
        assert!(form.dropdown_open());
        let open = compute_regions(Rect::new(0, 0, 60, 20), true);
        click(&mut form, &open, open.dropdown_items[2]); // click "completed"
        assert_eq!(form.status, Some(WatchStatus::Completed));
        assert_eq!(form.mode, Mode::Nav);
    }

    #[test]
    fn episode_spinner_clamps_to_real_episode_count() {
        // A 24-episode show: the spinner must stop at exactly 24.
        let mut form = AddForm::new("x", None, Some(24));
        form.focus = EPISODE;
        press(&mut form, &[KeyCode::Enter]);
        for _ in 0..30 {
            press(&mut form, &[KeyCode::Up]);
        }
        assert_eq!(form.episode, Some(24));
        // A movie (episodes: Some(1)) — the real motivating case.
        let mut movie = AddForm::new("x", None, Some(1));
        movie.focus = EPISODE;
        press(&mut movie, &[KeyCode::Enter, KeyCode::Up, KeyCode::Up, KeyCode::Up]);
        assert_eq!(movie.episode, Some(1));
    }

    #[test]
    fn unknown_episode_count_falls_back_to_safety_ceiling() {
        let mut form = AddForm::new("x", None, None);
        form.focus = EPISODE;
        form.episode = Some(EPISODE_MAX - 1);
        press(&mut form, &[KeyCode::Enter, KeyCode::Up, KeyCode::Up, KeyCode::Up]);
        assert_eq!(form.episode, Some(EPISODE_MAX)); // old ceiling still holds
    }

    #[test]
    fn form_is_centered_in_large_areas() {
        // Wide + tall terminal: the block sits centered, not top-left.
        let r = compute_regions(Rect::new(0, 0, 200, 50), false);
        assert_eq!(r.block.width, 58);
        assert_eq!(r.block.x, (200 - 58) / 2);
        assert_eq!(r.block.y, (50 - 9) / 2);
        // Terminal smaller than the form: no underflow, anchored at origin.
        let tiny = compute_regions(Rect::new(0, 0, 20, 5), false);
        assert_eq!((tiny.block.x, tiny.block.y), (0, 0));
        assert_eq!(tiny.block.width, 20);
        // Rows stay inside the centered block.
        assert!(r.rows[0].x > 0 && r.rows[0].x >= r.block.x);
    }

    /// Strongest available smoke test: a full synthetic session — keyboard
    /// only, blank form to submitted `Changes` — checked end to end.
    #[test]
    fn full_synthetic_session() {
        let mut form = AddForm::new("Solo Leveling", None, None);
        let outcome = press(
            &mut form,
            &[
                // status -> watching
                KeyCode::Enter,
                KeyCode::Down,
                KeyCode::Enter,
                // episode -> 12
                KeyCode::Down,
                KeyCode::Enter,
                KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up,
                KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up,
                KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up,
                KeyCode::Enter,
                // score -> 8
                KeyCode::Down,
                KeyCode::Enter,
                KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up,
                KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up,
                KeyCode::Enter,
                // skip rewatch, land on Save, submit
                KeyCode::Down,
                KeyCode::Down,
                KeyCode::Enter,
            ],
        );
        assert_eq!(outcome, Some(Outcome::Submit));
        let changes = form.changes();
        assert_eq!(changes.status, Some(WatchStatus::Watching));
        assert_eq!(changes.episode_progress, Some(12));
        assert_eq!(changes.score, Some(8));
        assert_eq!(changes.rewatch_count, None);
    }
}
