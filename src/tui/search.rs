//! Phase 1 of `anigit add`: the interactive incremental-search screen
//! (brainstorm.md 1.7a, update 2026-07-06).
//!
//! Type into the input to live-filter the catalog (`Catalog::find_by_name`
//! re-queried on every edit — a local SQLite LIKE query, fast enough to skip
//! debouncing), arrow keys / mouse to highlight a result (rows show title +
//! format + year to disambiguate look-alikes, e.g. the 11 real "Kaguya"
//! matches), Enter (or clicking the highlighted row) confirms, Esc cancels
//! the whole add flow. Only after this resolves to exactly one entry does
//! phase 2 — the existing part-6 edit form in `tui::run_add_menu` — open.
//!
//! Same architecture as the edit form (see mod.rs): all interaction state
//! lives in [`SearchScreen`], a plain struct with pure `handle_key`/
//! `handle_mouse` transitions unit-tested below without a terminal, results
//! injected via `set_results` so tests never need SQLite. [`compute_regions`]
//! is the shared layout truth for rendering and mouse hit-testing. Terminal
//! plumbing is confined to `run_search_screen`/`event_loop`/`draw`.

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
use ratatui::text::Span;
use ratatui::widgets::{Block, Paragraph};
use ratatui::{Frame, Terminal};
use std::io;

use crate::catalog::{Catalog, CatalogEntry};

// ---------------------------------------------------------------------------
// Screen state (plain data, no terminal involved)
// ---------------------------------------------------------------------------

/// What a key/mouse event asked the caller to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    None,
    /// The query text changed — re-run the catalog search and `set_results`.
    QueryChanged,
    /// The highlighted entry was confirmed.
    Selected,
    Cancelled,
}

struct SearchScreen {
    query: String,
    results: Vec<CatalogEntry>,
    highlighted: usize,
    /// First visible result row (kept so the highlight stays on screen when
    /// the list is taller than the terminal).
    scroll: usize,
}

impl SearchScreen {
    fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            highlighted: 0,
            scroll: 0,
        }
    }

    /// Replace the result list (after a catalog re-query). Resets the
    /// highlight to the top — the old position is meaningless against a
    /// different list.
    fn set_results(&mut self, results: Vec<CatalogEntry>) {
        self.results = results;
        self.highlighted = 0;
        self.scroll = 0;
    }

    fn selected_entry(&self) -> Option<&CatalogEntry> {
        self.results.get(self.highlighted)
    }

    fn move_highlight(&mut self, delta: i64) {
        if self.results.is_empty() {
            return;
        }
        let last = self.results.len() as i64 - 1;
        self.highlighted = (self.highlighted as i64 + delta).clamp(0, last) as usize;
    }

    /// Keep the highlighted row within the visible window of `max_rows`.
    fn clamp_scroll(&mut self, max_rows: usize) {
        if max_rows == 0 {
            return;
        }
        if self.highlighted < self.scroll {
            self.scroll = self.highlighted;
        } else if self.highlighted >= self.scroll + max_rows {
            self.scroll = self.highlighted + 1 - max_rows;
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::Cancelled,
            KeyCode::Enter => {
                if self.results.is_empty() {
                    Action::None
                } else {
                    Action::Selected
                }
            }
            KeyCode::Up => {
                self.move_highlight(-1);
                Action::None
            }
            KeyCode::Down => {
                self.move_highlight(1);
                Action::None
            }
            KeyCode::Backspace => {
                if self.query.pop().is_some() {
                    Action::QueryChanged
                } else {
                    Action::None
                }
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.query.push(c);
                Action::QueryChanged
            }
            _ => Action::None,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, regions: &Regions) -> Action {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.move_highlight(-1);
                Action::None
            }
            MouseEventKind::ScrollDown => {
                self.move_highlight(1);
                Action::None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = Position::new(mouse.column, mouse.row);
                if let Some(row) = regions.result_rows.iter().position(|r| r.contains(pos)) {
                    let index = self.scroll + row;
                    if index >= self.results.len() {
                        return Action::None;
                    }
                    // First click highlights; clicking the already-highlighted
                    // row confirms it — "click the highlighted/clicked row to
                    // select" per the accepted interaction shape.
                    if index == self.highlighted {
                        return Action::Selected;
                    }
                    self.highlighted = index;
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Layout (pure — shared by rendering and mouse hit-testing)
// ---------------------------------------------------------------------------

const INPUT_HEIGHT: u16 = 3; // bordered single-line input
const MAX_WIDTH: u16 = 70;

struct Regions {
    input: Rect,
    /// One rect per VISIBLE result row (window of `max_rows` starting at the
    /// screen's scroll offset).
    result_rows: Vec<Rect>,
    footer: Rect,
    max_rows: usize,
}

fn compute_regions(area: Rect, visible_results: usize) -> Regions {
    let width = area.width.min(MAX_WIDTH);
    // Centered horizontally, recomputed on every draw so resizes re-center
    // for free. Deliberately NOT centered vertically: the list's height
    // changes with every keystroke, and a vertically-centered list would
    // jump up and down as results narrow — top-anchored is the standard
    // search-palette shape (fzf/Spotlight) for exactly that reason.
    let x = area.x + area.width.saturating_sub(width) / 2;
    let input = Rect::new(x, area.y, width, INPUT_HEIGHT.min(area.height));
    // Everything between the input box and the one-line footer holds results.
    let max_rows = area
        .height
        .saturating_sub(INPUT_HEIGHT + 1) as usize;
    let shown = visible_results.min(max_rows);
    let result_rows = (0..shown as u16)
        .map(|i| Rect::new(x, area.y + INPUT_HEIGHT + i, width, 1))
        .collect();
    let footer = Rect::new(
        x,
        area.y + INPUT_HEIGHT + shown as u16,
        area.width.saturating_sub(x - area.x),
        1,
    );
    Regions {
        input,
        result_rows,
        footer,
        max_rows,
    }
}

// ---------------------------------------------------------------------------
// Terminal plumbing + rendering
// ---------------------------------------------------------------------------

/// Run the search screen against the catalog. Returns the confirmed entry,
/// or `None` if the user cancelled (Esc) — the caller must not proceed to
/// the edit form in that case.
pub fn run_search_screen(catalog: &Catalog) -> Result<Option<CatalogEntry>> {
    let mut screen = SearchScreen::new();
    // Before any typing: show the first page of the whole catalog (an empty
    // query LIKE-matches everything) — a browsable default beats a blank
    // screen, and it demonstrates the list is live.
    screen.set_results(catalog.find_by_name("")?);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let outcome = event_loop(&mut terminal, &mut screen, catalog);

    disable_raw_mode().ok();
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();

    outcome
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    screen: &mut SearchScreen,
    catalog: &Catalog,
) -> Result<Option<CatalogEntry>> {
    loop {
        let mut regions = None;
        terminal.draw(|frame| regions = Some(draw(frame, screen)))?;
        let regions = regions.expect("draw closure always runs");

        let action = match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => screen.handle_key(key),
            Event::Mouse(mouse) => screen.handle_mouse(mouse, &regions),
            _ => Action::None,
        };
        match action {
            Action::QueryChanged => {
                screen.set_results(catalog.find_by_name(&screen.query)?);
            }
            Action::Selected => return Ok(screen.selected_entry().cloned()),
            Action::Cancelled => return Ok(None),
            Action::None => {}
        }
    }
}

fn draw(frame: &mut Frame, screen: &mut SearchScreen) -> Regions {
    let area = frame.area();
    let regions = compute_regions(area, screen.results.len());
    screen.clamp_scroll(regions.max_rows);

    let input = Paragraph::new(format!("> {}_", screen.query))
        .block(Block::bordered().title(" anigit add — search the catalog "));
    render_clamped(frame, input, regions.input, area);

    if screen.results.is_empty() {
        let notice = if screen.query.is_empty() {
            "The catalog is empty — run `anigit refresh` first.".to_string()
        } else {
            format!("No matches for '{}' — try different wording.", screen.query)
        };
        let rect = Rect::new(
            regions.input.x + 2,
            area.y + INPUT_HEIGHT,
            area.width.saturating_sub(regions.input.x + 2 - area.x),
            1,
        );
        render_clamped(
            frame,
            Paragraph::new(Span::styled(notice, Style::default().fg(Color::Yellow))),
            rect,
            area,
        );
    }

    for (row, rect) in regions.result_rows.iter().enumerate() {
        let index = screen.scroll + row;
        let Some(entry) = screen.results.get(index) else {
            break;
        };
        let focused = index == screen.highlighted;
        let marker = if focused { "▶ " } else { "  " };
        let style = if focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let line = format!(
            "{marker}{:<44} {:<7} {}",
            truncate(entry.display_title(), 44),
            entry.format.as_deref().unwrap_or("-"),
            entry
                .start_date
                .as_deref()
                .map(|d| d.get(..4).unwrap_or(d))
                .unwrap_or("-")
        );
        render_clamped(frame, Paragraph::new(Span::styled(line, style)), *rect, area);
    }

    let hint = "type to search · ↑/↓ move · Enter select · Esc cancel";
    render_clamped(
        frame,
        Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        regions.footer,
        area,
    );

    regions
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// Render only the part of `rect` that fits the frame (same tiny-terminal
/// safety as the edit form's helper).
fn render_clamped<W: ratatui::widgets::Widget>(
    frame: &mut Frame,
    widget: W,
    rect: Rect,
    area: Rect,
) {
    let clamped = rect.intersection(area);
    if !clamped.is_empty() {
        frame.render_widget(widget, clamped);
    }
}

// ---------------------------------------------------------------------------
// Tests — pure state-transition coverage, no terminal or SQLite required
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::AiringStatus;

    fn entry(id: i64, title: &str) -> CatalogEntry {
        CatalogEntry {
            id,
            title_romaji: Some(title.to_string()),
            title_english: None,
            format: Some("TV".into()),
            episodes: None,
            description: None,
            status: AiringStatus::Finished,
            start_date: Some("2019-01-12".into()),
            end_date: None,
            genres: Vec::new(),
            tags: Vec::new(),
            last_updated: "2026-07-06T00:00:00+00:00".into(),
        }
    }

    fn kaguya_results() -> Vec<CatalogEntry> {
        vec![
            entry(1, "Kaguya-sama: Love is War"),
            entry(2, "Kaguya-sama: Love is War Season 2"),
            entry(3, "Kaguya-sama: Love is War -Ultra Romantic-"),
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn click(screen: &mut SearchScreen, regions: &Regions, rect: Rect) -> Action {
        screen.handle_mouse(
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
    fn typing_updates_query_and_requests_requery() {
        let mut screen = SearchScreen::new();
        assert_eq!(screen.handle_key(key(KeyCode::Char('k'))), Action::QueryChanged);
        assert_eq!(screen.handle_key(key(KeyCode::Char('a'))), Action::QueryChanged);
        assert_eq!(screen.query, "ka");
        assert_eq!(screen.handle_key(key(KeyCode::Backspace)), Action::QueryChanged);
        assert_eq!(screen.query, "k");
        // Backspace on an empty query changes nothing — no pointless re-query.
        screen.query.clear();
        assert_eq!(screen.handle_key(key(KeyCode::Backspace)), Action::None);
    }

    #[test]
    fn arrows_move_highlight_and_clamp() {
        let mut screen = SearchScreen::new();
        screen.set_results(kaguya_results());
        assert_eq!(screen.highlighted, 0);
        screen.handle_key(key(KeyCode::Up)); // clamps at top
        assert_eq!(screen.highlighted, 0);
        screen.handle_key(key(KeyCode::Down));
        screen.handle_key(key(KeyCode::Down));
        screen.handle_key(key(KeyCode::Down)); // clamps at bottom
        assert_eq!(screen.highlighted, 2);
    }

    #[test]
    fn enter_selects_highlighted_entry() {
        let mut screen = SearchScreen::new();
        screen.set_results(kaguya_results());
        screen.handle_key(key(KeyCode::Down));
        assert_eq!(screen.handle_key(key(KeyCode::Enter)), Action::Selected);
        assert_eq!(
            screen.selected_entry().unwrap().display_title(),
            "Kaguya-sama: Love is War Season 2"
        );
    }

    #[test]
    fn enter_on_empty_results_does_nothing_and_esc_cancels() {
        let mut screen = SearchScreen::new();
        screen.set_results(Vec::new()); // the "no matches" state
        assert_eq!(screen.handle_key(key(KeyCode::Enter)), Action::None);
        assert!(screen.selected_entry().is_none());
        assert_eq!(screen.handle_key(key(KeyCode::Esc)), Action::Cancelled);
    }

    #[test]
    fn new_results_reset_highlight() {
        let mut screen = SearchScreen::new();
        screen.set_results(kaguya_results());
        screen.handle_key(key(KeyCode::Down));
        assert_eq!(screen.highlighted, 1);
        screen.set_results(vec![entry(9, "Something Else")]);
        assert_eq!(screen.highlighted, 0);
    }

    #[test]
    fn mouse_click_highlights_then_confirms() {
        let mut screen = SearchScreen::new();
        screen.set_results(kaguya_results());
        let regions = compute_regions(Rect::new(0, 0, 80, 24), screen.results.len());
        // First click on row 2: highlight moves, no selection yet.
        assert_eq!(click(&mut screen, &regions, regions.result_rows[2]), Action::None);
        assert_eq!(screen.highlighted, 2);
        // Second click on the same (now highlighted) row confirms.
        assert_eq!(
            click(&mut screen, &regions, regions.result_rows[2]),
            Action::Selected
        );
    }

    #[test]
    fn layout_is_horizontally_centered() {
        // Wide terminal: input + rows sit centered, not flush-left.
        let r = compute_regions(Rect::new(0, 0, 200, 24), 3);
        assert_eq!(r.input.width, 70);
        assert_eq!(r.input.x, (200 - 70) / 2);
        assert!(r.result_rows.iter().all(|row| row.x == r.input.x));
        // Terminal narrower than MAX_WIDTH: full width, no underflow.
        let narrow = compute_regions(Rect::new(0, 0, 40, 24), 3);
        assert_eq!((narrow.input.x, narrow.input.width), (0, 40));
    }

    #[test]
    fn scroll_window_follows_highlight() {
        let mut screen = SearchScreen::new();
        screen.set_results((0..20).map(|i| entry(i, &format!("Show {i}"))).collect());
        for _ in 0..12 {
            screen.handle_key(key(KeyCode::Down));
        }
        screen.clamp_scroll(5); // pretend only 5 rows fit
        assert_eq!(screen.highlighted, 12);
        assert_eq!(screen.scroll, 8); // 12 is the last visible row of 8..13
        // Mouse hit-testing accounts for the scroll offset.
        let regions = compute_regions(Rect::new(0, 0, 80, 9), screen.results.len());
        assert_eq!(regions.max_rows, 5);
        assert_eq!(click(&mut screen, &regions, regions.result_rows[0]), Action::None);
        assert_eq!(screen.highlighted, 8);
    }
}
