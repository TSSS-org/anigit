//! The interactive search screen for `anigit blame` (brainstorm.md 1.15).
//!
//! Same architecture and controls as `anigit add`'s search screen (see
//! `search.rs`, the template this deliberately mirrors): all interaction
//! state in a plain struct with pure `handle_key`/`handle_mouse` transitions
//! unit-tested without a terminal, [`compute_regions`] as the shared layout
//! truth for rendering and mouse hit-testing, terminal plumbing confined to
//! `run_blame_search_screen`/`event_loop`/`draw`.
//!
//! The one deliberate difference from `add`'s screen: NO catalog access
//! during typing. Blame only makes sense for anime this repo has actually
//! committed something about, so the caller pre-scopes the candidate list
//! (repo history keys → `Catalog::find_by_id`, once, at startup) and this
//! screen just live-filters that small fixed in-memory list — an
//! in-memory case-insensitive substring match per keystroke, not a SQL
//! query. The candidate set is bounded by how many distinct anime the user
//! has ever tracked, not the ~20k-entry catalog, so this is both simpler
//! and the right fit.

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

use crate::catalog::CatalogEntry;

// ---------------------------------------------------------------------------
// Screen state (plain data, no terminal involved)
// ---------------------------------------------------------------------------

/// What a key/mouse event asked the caller to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    None,
    /// The query text changed — re-filter the candidate list (`refilter`).
    QueryChanged,
    /// The highlighted entry was confirmed.
    Selected,
    Cancelled,
}

/// Named `BlameSearchScreen` (not `SearchScreen`) so the two screens can't
/// be confused if they ever end up imported into one scope — see search.rs
/// for the add-flow twin.
struct BlameSearchScreen {
    query: String,
    /// The full pre-scoped candidate set — never changes after construction.
    candidates: Vec<CatalogEntry>,
    /// Indices into `candidates` matching the current query. Indices rather
    /// than clones: the filter runs on every keystroke, and the candidate
    /// list is the stable owner.
    results: Vec<usize>,
    highlighted: usize,
    /// First visible result row (kept so the highlight stays on screen when
    /// the list is taller than the terminal).
    scroll: usize,
}

impl BlameSearchScreen {
    /// Starts with the whole candidate list visible (empty query matches
    /// everything) — a browsable default, same as `add`'s screen.
    fn new(candidates: Vec<CatalogEntry>) -> Self {
        let results = (0..candidates.len()).collect();
        Self {
            query: String::new(),
            candidates,
            results,
            highlighted: 0,
            scroll: 0,
        }
    }

    /// Recompute `results` from the current query: case-insensitive
    /// substring match against BOTH title fields (not just
    /// `display_title()`), so a romaji query still finds an entry whose
    /// display title is English and vice versa. Resets the highlight to the
    /// top — the old position is meaningless against a different list.
    fn refilter(&mut self) {
        let needle = self.query.to_lowercase();
        self.results = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                [&entry.title_english, &entry.title_romaji]
                    .into_iter()
                    .flatten()
                    .any(|title| title.to_lowercase().contains(&needle))
            })
            .map(|(i, _)| i)
            .collect();
        self.highlighted = 0;
        self.scroll = 0;
    }

    fn selected_entry(&self) -> Option<&CatalogEntry> {
        self.results
            .get(self.highlighted)
            .map(|&i| &self.candidates[i])
    }

    /// The entry shown on visible result row `row` (0 = topmost visible).
    fn entry_at_row(&self, row: usize) -> Option<&CatalogEntry> {
        self.results
            .get(self.scroll + row)
            .map(|&i| &self.candidates[i])
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
                    // row confirms it — same interaction shape as `add`'s
                    // screen.
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
// Layout (pure — shared by rendering and mouse hit-testing; same shape as
// search.rs's compute_regions, kept local so neither screen's layout can
// drift the other's by accident)
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
    // Centered horizontally, top-anchored vertically — same convention (and
    // reasoning) as add's screen: a vertically-centered list would jump as
    // results narrow with each keystroke.
    let x = area.x + area.width.saturating_sub(width) / 2;
    let input = Rect::new(x, area.y, width, INPUT_HEIGHT.min(area.height));
    // Everything between the input box and the one-line footer holds results.
    let max_rows = area.height.saturating_sub(INPUT_HEIGHT + 1) as usize;
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

/// Run the blame search screen over an ALREADY-SCOPED candidate list (the
/// anime this repo's history has actually touched — the caller builds it,
/// no catalog access happens in here). Returns the confirmed entry, or
/// `None` if the user cancelled (Esc).
pub fn run_blame_search_screen(candidates: Vec<CatalogEntry>) -> Result<Option<CatalogEntry>> {
    let mut screen = BlameSearchScreen::new(candidates);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let outcome = event_loop(&mut terminal, &mut screen);

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
    screen: &mut BlameSearchScreen,
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
            // Unlike add's screen (which re-queries SQLite here), the whole
            // candidate set is already in memory — just re-filter it.
            Action::QueryChanged => screen.refilter(),
            Action::Selected => return Ok(screen.selected_entry().cloned()),
            Action::Cancelled => return Ok(None),
            Action::None => {}
        }
    }
}

fn draw(frame: &mut Frame, screen: &mut BlameSearchScreen) -> Regions {
    let area = frame.area();
    let regions = compute_regions(area, screen.results.len());
    screen.clamp_scroll(regions.max_rows);

    let input = Paragraph::new(format!("> {}_", screen.query))
        .block(Block::bordered().title(" anigit blame — search your tracked anime "));
    render_clamped(frame, input, regions.input, area);

    if screen.results.is_empty() {
        // The caller guarantees a non-empty candidate list, so an empty
        // result set always means the query filtered everything out.
        let notice = format!(
            "No tracked anime matches '{}' — blame only searches anime in this repo's history.",
            screen.query
        );
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
        let Some(entry) = screen.entry_at_row(row) else {
            break;
        };
        let focused = screen.scroll + row == screen.highlighted;
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

    let hint = "type to filter · ↑/↓ move · Enter select · Esc cancel";
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
/// safety as the other screens' helper).
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
// Tests — pure state-transition coverage, no terminal or catalog required
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::AiringStatus;

    fn entry(id: i64, romaji: &str, english: Option<&str>) -> CatalogEntry {
        CatalogEntry {
            id,
            title_romaji: Some(romaji.to_string()),
            title_english: english.map(str::to_string),
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

    fn tracked_candidates() -> Vec<CatalogEntry> {
        vec![
            entry(1, "Cowboy Bebop", None),
            entry(
                2,
                "Mushoku Tensei: Isekai Ittara Honki Dasu",
                Some("Mushoku Tensei: Jobless Reincarnation"),
            ),
            entry(3, "Kaguya-sama wa Kokurasetai", Some("Kaguya-sama: Love is War")),
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn type_and_filter(screen: &mut BlameSearchScreen, text: &str) {
        for c in text.chars() {
            if screen.handle_key(key(KeyCode::Char(c))) == Action::QueryChanged {
                screen.refilter();
            }
        }
    }

    fn click(screen: &mut BlameSearchScreen, regions: &Regions, rect: Rect) -> Action {
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
    fn starts_with_all_candidates_visible() {
        let screen = BlameSearchScreen::new(tracked_candidates());
        assert_eq!(screen.results.len(), 3);
        assert_eq!(screen.selected_entry().unwrap().id, 1);
    }

    #[test]
    fn typing_filters_case_insensitively() {
        let mut screen = BlameSearchScreen::new(tracked_candidates());
        type_and_filter(&mut screen, "BEBOP");
        assert_eq!(screen.results.len(), 1);
        assert_eq!(screen.selected_entry().unwrap().display_title(), "Cowboy Bebop");
    }

    #[test]
    fn filter_matches_both_title_fields() {
        let mut screen = BlameSearchScreen::new(tracked_candidates());
        // Romaji-only query for an entry whose display title is English.
        type_and_filter(&mut screen, "kokurasetai");
        assert_eq!(screen.results.len(), 1);
        assert_eq!(
            screen.selected_entry().unwrap().display_title(),
            "Kaguya-sama: Love is War"
        );
    }

    #[test]
    fn backspace_widens_the_filter_again() {
        let mut screen = BlameSearchScreen::new(tracked_candidates());
        type_and_filter(&mut screen, "kaguyaz");
        assert!(screen.results.is_empty());
        assert_eq!(screen.handle_key(key(KeyCode::Backspace)), Action::QueryChanged);
        screen.refilter();
        assert_eq!(screen.results.len(), 1);
        // Backspace on an empty query changes nothing — no pointless refilter.
        screen.query.clear();
        assert_eq!(screen.handle_key(key(KeyCode::Backspace)), Action::None);
    }

    #[test]
    fn arrows_move_highlight_and_clamp() {
        let mut screen = BlameSearchScreen::new(tracked_candidates());
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
        let mut screen = BlameSearchScreen::new(tracked_candidates());
        screen.handle_key(key(KeyCode::Down));
        assert_eq!(screen.handle_key(key(KeyCode::Enter)), Action::Selected);
        assert_eq!(
            screen.selected_entry().unwrap().display_title(),
            "Mushoku Tensei: Jobless Reincarnation"
        );
    }

    #[test]
    fn enter_on_empty_results_does_nothing_and_esc_cancels() {
        let mut screen = BlameSearchScreen::new(tracked_candidates());
        type_and_filter(&mut screen, "no such anime");
        assert!(screen.results.is_empty());
        assert_eq!(screen.handle_key(key(KeyCode::Enter)), Action::None);
        assert!(screen.selected_entry().is_none());
        assert_eq!(screen.handle_key(key(KeyCode::Esc)), Action::Cancelled);
    }

    #[test]
    fn refilter_resets_highlight() {
        let mut screen = BlameSearchScreen::new(tracked_candidates());
        screen.handle_key(key(KeyCode::Down));
        assert_eq!(screen.highlighted, 1);
        type_and_filter(&mut screen, "kaguya");
        assert_eq!(screen.highlighted, 0);
    }

    #[test]
    fn mouse_click_highlights_then_confirms() {
        let mut screen = BlameSearchScreen::new(tracked_candidates());
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
        let mut screen = BlameSearchScreen::new(
            (0..20)
                .map(|i| entry(i, &format!("Show {i}"), None))
                .collect(),
        );
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
