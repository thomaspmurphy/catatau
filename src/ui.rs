use crate::constants::{
    DEFAULT_TERMINAL_HEIGHT, MAX_DISPLAY_LINE_LENGTH, MIN_SEARCH_LINE_LENGTH,
    SEARCH_RESULT_TOP_OFFSET, UI_RESERVED_HEIGHT,
};
use crate::epub::EpubReader;
use crate::error::UiError;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::io;

#[derive(Debug)]
struct SearchResultLocation {
    chapter: usize,
    line: usize,
}

#[derive(Debug)]
struct ChapterLocation {
    chapter: usize,
}

#[derive(Debug)]
enum FloatingPane {
    None,
    Search {
        query: String,
        results: Vec<String>,
        selected_index: usize,
    },
    Contents {
        selected_index: usize,
    },
}

#[derive(Debug)]
struct NavigationState {
    current_chapter: usize,
    scroll_offset: usize,
    highlighted_search_term: Option<String>,
}

impl NavigationState {
    fn new() -> Self {
        Self {
            current_chapter: 0,
            scroll_offset: 0,
            highlighted_search_term: None,
        }
    }

    fn clear_highlight(&mut self) {
        self.highlighted_search_term = None;
    }

    fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }
}

pub struct App {
    epub: EpubReader,
    nav_state: NavigationState,
    floating_pane: FloatingPane,
    terminal_height: usize,
    terminal: Option<Terminal<CrosstermBackend<std::io::Stdout>>>,
}

impl App {
    pub fn new(epub: EpubReader) -> Self {
        Self {
            epub,
            nav_state: NavigationState::new(),
            floating_pane: FloatingPane::None,
            terminal_height: DEFAULT_TERMINAL_HEIGHT,
            terminal: None,
        }
    }

    // Public accessors for testing
    #[allow(dead_code)]
    pub fn current_chapter(&self) -> usize {
        self.nav_state.current_chapter
    }

    #[allow(dead_code)]
    pub fn scroll_offset(&self) -> usize {
        self.nav_state.scroll_offset
    }

    #[allow(dead_code)]
    pub fn epub(&self) -> &EpubReader {
        &self.epub
    }

    pub fn run(&mut self) -> Result<(), UiError> {
        self.setup_terminal()?;

        loop {
            if let Some(terminal) = self.terminal.as_mut() {
                self.terminal_height = terminal.size()?.height as usize;
                let current_chapter = self.nav_state.current_chapter;
                let scroll_offset = self.nav_state.scroll_offset;
                let terminal_height = self.terminal_height;
                let epub = &self.epub;
                let highlighted_search_term = &self.nav_state.highlighted_search_term;
                let floating_pane = &self.floating_pane;

                terminal.draw(|f| {
                    Self::draw_ui(
                        f,
                        epub,
                        current_chapter,
                        scroll_offset,
                        terminal_height,
                        highlighted_search_term,
                        floating_pane,
                    );
                })?;
            }

            if let Event::Key(key) = event::read()? {
                // Handle floating pane interactions first
                if self.handle_floating_pane_input(key) {
                    continue;
                }

                // Regular navigation when no floating pane is active
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.scroll_down();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.scroll_up();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        self.page_down();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::PageUp | KeyCode::Char('b') => {
                        self.page_up();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.next_chapter();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.prev_chapter();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        self.go_to_beginning();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        self.go_to_end();
                        self.nav_state.clear_highlight();
                    }
                    KeyCode::Char('/') => self.open_search_pane(),
                    KeyCode::Char('-') => self.open_contents_pane(),
                    _ => {}
                }
            }
        }

        self.cleanup_terminal()?;

        Ok(())
    }

    fn setup_terminal(&mut self) -> Result<(), UiError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        self.terminal = Some(terminal);
        Ok(())
    }

    fn cleanup_terminal(&mut self) -> Result<(), UiError> {
        if let Some(mut terminal) = self.terminal.take() {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
        }
        Ok(())
    }

    fn draw_ui(
        f: &mut Frame,
        epub: &EpubReader,
        current_chapter: usize,
        scroll_offset: usize,
        terminal_height: usize,
        highlighted_search_term: &Option<String>,
        floating_pane: &FloatingPane,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Footer
            ])
            .split(f.area());

        // Header
        let header = Paragraph::new(vec![Line::from(vec![
            Span::styled(
                &epub.title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" by ", Style::default().fg(Color::White)),
            Span::styled(
                &epub.author,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::ITALIC),
            ),
        ])])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Catatau - EPUB Reader"),
        );
        f.render_widget(header, chunks[0]);

        // Content
        if let Some(chapter) = epub.chapters.get(current_chapter) {
            let lines: Vec<Line> = if let Some(search_term) = highlighted_search_term {
                // Create highlighted lines
                chapter
                    .content
                    .lines()
                    .skip(scroll_offset)
                    .take(terminal_height.saturating_sub(UI_RESERVED_HEIGHT))
                    .map(|line| Self::highlight_line(line, search_term))
                    .collect()
            } else {
                // Regular lines without highlighting
                chapter
                    .content
                    .lines()
                    .skip(scroll_offset)
                    .take(terminal_height.saturating_sub(UI_RESERVED_HEIGHT))
                    .map(|line| {
                        Line::from(vec![Span::styled(
                            line.to_string(),
                            Style::default().fg(Color::White),
                        )])
                    })
                    .collect()
            };

            let content = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(chapter.title.as_str()),
                )
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: false });
            f.render_widget(content, chunks[1]);
        }

        // Footer with navigation help
        let footer_text = format!(
            "{}/{} | q:quit, ↑↓/jk:scroll, ←→/hl:chapters, space/b:page, g/G:start/end, /:search, -:contents",
            current_chapter + 1,
            epub.chapters.len()
        );
        let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::Gray));
        f.render_widget(footer, chunks[2]);

        // IMPORTANT: Render floating panes on top of everything else
        Self::render_floating_pane(f, floating_pane, epub);
    }

    fn highlight_line(line: &str, search_term: &str) -> Line<'static> {
        let search_lower = search_term.to_lowercase();
        let line_lower = line.to_lowercase();

        if let Some(pos) = line_lower.find(&search_lower) {
            let mut spans = Vec::new();

            // Text before the match
            if pos > 0 {
                spans.push(Span::styled(
                    line[..pos].to_string(),
                    Style::default().fg(Color::White),
                ));
            }

            // The highlighted match
            let end_pos = pos + search_term.len();
            spans.push(Span::styled(
                line[pos..end_pos.min(line.len())].to_string(),
                Style::default().bg(Color::Yellow).fg(Color::Black),
            ));

            // Text after the match
            if end_pos < line.len() {
                spans.push(Span::styled(
                    line[end_pos..].to_string(),
                    Style::default().fg(Color::White),
                ));
            }

            Line::from(spans)
        } else {
            Line::from(vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            )])
        }
    }

    // Helper methods for scroll calculations
    fn get_page_size(&self) -> usize {
        self.terminal_height.saturating_sub(UI_RESERVED_HEIGHT)
    }

    fn get_max_scroll_for_chapter(&self, chapter_index: usize) -> usize {
        if let Some(chapter) = self.epub.chapters.get(chapter_index) {
            let total_lines = chapter.content.lines().count();
            total_lines.saturating_sub(self.get_page_size())
        } else {
            0
        }
    }

    fn get_current_chapter_max_scroll(&self) -> usize {
        self.get_max_scroll_for_chapter(self.nav_state.current_chapter)
    }

    fn clamp_scroll_to_limits(&mut self, chapter_index: usize) {
        let max_scroll = self.get_max_scroll_for_chapter(chapter_index);
        self.nav_state.scroll_offset = self.nav_state.scroll_offset.min(max_scroll);
    }

    fn scroll_down(&mut self) {
        let max_scroll = self.get_current_chapter_max_scroll();
        if self.nav_state.scroll_offset < max_scroll {
            self.nav_state.scroll_offset += 1;
        }
    }

    fn scroll_up(&mut self) {
        if self.nav_state.scroll_offset > 0 {
            self.nav_state.scroll_offset -= 1;
        }
    }

    fn page_down(&mut self) {
        let page_size = self.get_page_size();
        let max_scroll = self.get_current_chapter_max_scroll();
        self.nav_state.scroll_offset = (self.nav_state.scroll_offset + page_size).min(max_scroll);
    }

    fn page_up(&mut self) {
        let page_size = self.get_page_size();
        self.nav_state.scroll_offset = self.nav_state.scroll_offset.saturating_sub(page_size);
    }

    fn next_chapter(&mut self) {
        if self.nav_state.current_chapter < self.epub.chapters.len().saturating_sub(1) {
            self.nav_state.current_chapter += 1;
            self.nav_state.reset_scroll();
        }
    }

    fn prev_chapter(&mut self) {
        if self.nav_state.current_chapter > 0 {
            self.nav_state.current_chapter -= 1;
            self.nav_state.reset_scroll();
        }
    }

    fn go_to_beginning(&mut self) {
        self.nav_state.reset_scroll();
    }

    fn go_to_end(&mut self) {
        self.nav_state.scroll_offset = self.get_current_chapter_max_scroll();
    }

    fn build_search_items(&self) -> Vec<String> {
        let mut all_lines = Vec::new();
        for (chapter_index, chapter) in self.epub.chapters.iter().enumerate() {
            for (line_index, line) in chapter.content.lines().enumerate() {
                if !line.trim().is_empty() && line.trim().len() > MIN_SEARCH_LINE_LENGTH {
                    let truncated = self.truncate_line_for_display(line);
                    all_lines.push(format!(
                        "Ch{:2} L{:3}: {}",
                        chapter_index + 1,
                        line_index + 1,
                        truncated.trim()
                    ));
                }
            }
        }
        all_lines
    }

    fn truncate_line_for_display(&self, line: &str) -> String {
        if line.chars().count() > MAX_DISPLAY_LINE_LENGTH {
            let truncated_chars: String = line.chars().take(MAX_DISPLAY_LINE_LENGTH - 3).collect();
            format!("{}...", truncated_chars)
        } else {
            line.to_string()
        }
    }

    fn parse_and_jump_to_search_selection(&mut self, selected_text: &str, search_query: &str) {
        if let Some(location) = Self::parse_search_result_location(selected_text) {
            self.jump_to_search_location(location, search_query);
        }
    }

    fn parse_search_result_location(text: &str) -> Option<SearchResultLocation> {
        // Parse chapter and line from format: "ChXX LYYY: content"
        let ch_pos = text.find("Ch")?;
        let l_pos = text.find(" L")?;
        let colon_pos = text.find(": ")?;

        let chapter_str = text[ch_pos + 2..l_pos].trim();
        let line_str = text[l_pos + 2..colon_pos].trim();

        let chapter = chapter_str.parse().ok()?;
        let line = line_str.parse().ok()?;

        Some(SearchResultLocation { chapter, line })
    }

    fn jump_to_search_location(&mut self, location: SearchResultLocation, search_query: &str) {
        // Validate chapter exists
        if location.chapter == 0 || location.chapter > self.epub.chapters.len() {
            return;
        }

        self.nav_state.current_chapter = location.chapter - 1; // Convert to 0-based

        // Validate line exists and set scroll position
        if let Some(_chapter) = self.epub.chapters.get(self.nav_state.current_chapter) {
            let target_line = location.line.saturating_sub(1); // Convert to 0-based

            // Set scroll offset to show the target line near the top
            self.nav_state.scroll_offset = target_line.saturating_sub(SEARCH_RESULT_TOP_OFFSET);

            // Ensure we don't scroll past the end
            self.clamp_scroll_to_limits(self.nav_state.current_chapter);

            // Set the search term for highlighting if we have a query
            if !search_query.is_empty() {
                self.nav_state.highlighted_search_term = Some(search_query.to_string());
            }
        }
    }

    fn parse_and_jump_to_chapter(&mut self, selected_text: &str) {
        if let Some(location) = Self::parse_chapter_location(selected_text) {
            self.jump_to_chapter_location(location);
        }
    }

    fn parse_chapter_location(text: &str) -> Option<ChapterLocation> {
        // Parse chapter from format: "X: Title"
        let colon_pos = text.find(": ")?;

        let chapter_str = text[0..colon_pos].trim();
        let chapter = chapter_str.parse().ok()?;

        Some(ChapterLocation { chapter })
    }

    fn jump_to_chapter_location(&mut self, location: ChapterLocation) {
        // Validate chapter exists
        if location.chapter == 0 || location.chapter > self.epub.chapters.len() {
            return;
        }

        self.nav_state.current_chapter = location.chapter - 1; // Convert to 0-based
        self.nav_state.reset_scroll(); // Start at the beginning of the chapter
    }

    // Floating pane methods
    fn handle_floating_pane_input(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let floating_pane = std::mem::replace(&mut self.floating_pane, FloatingPane::None);

        match floating_pane {
            FloatingPane::None => {
                self.floating_pane = FloatingPane::None;
                false
            }
            FloatingPane::Search {
                mut query,
                results,
                mut selected_index,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        // Keep floating_pane as None
                        true
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                        let new_results = self.filter_search_results(&query);
                        self.floating_pane = FloatingPane::Search {
                            query,
                            results: new_results,
                            selected_index: 0,
                        };
                        true
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        let new_results = self.filter_search_results(&query);
                        self.floating_pane = FloatingPane::Search {
                            query,
                            results: new_results,
                            selected_index: 0,
                        };
                        true
                    }
                    KeyCode::Up => {
                        selected_index = selected_index.saturating_sub(1);
                        self.floating_pane = FloatingPane::Search {
                            query,
                            results,
                            selected_index,
                        };
                        true
                    }
                    KeyCode::Down => {
                        if selected_index < results.len().saturating_sub(1) {
                            selected_index += 1;
                        }
                        self.floating_pane = FloatingPane::Search {
                            query,
                            results,
                            selected_index,
                        };
                        true
                    }
                    KeyCode::Enter => {
                        if let Some(selected_text) = results.get(selected_index) {
                            let query_copy = query.clone();
                            let selected_text_copy = selected_text.clone();
                            // floating_pane remains None
                            self.parse_and_jump_to_search_selection(
                                &selected_text_copy,
                                &query_copy,
                            );
                        } else {
                            self.floating_pane = FloatingPane::Search {
                                query,
                                results,
                                selected_index,
                            };
                        }
                        true
                    }
                    _ => {
                        self.floating_pane = FloatingPane::Search {
                            query,
                            results,
                            selected_index,
                        };
                        true
                    }
                }
            }
            FloatingPane::Contents { mut selected_index } => {
                match key.code {
                    KeyCode::Esc => {
                        // Keep floating_pane as None
                        true
                    }
                    KeyCode::Up => {
                        selected_index = selected_index.saturating_sub(1);
                        self.floating_pane = FloatingPane::Contents { selected_index };
                        true
                    }
                    KeyCode::Down => {
                        if selected_index < self.epub.chapters.len().saturating_sub(1) {
                            selected_index += 1;
                        }
                        self.floating_pane = FloatingPane::Contents { selected_index };
                        true
                    }
                    KeyCode::Enter => {
                        let selected_text = format!(
                            "{}: {}",
                            selected_index + 1,
                            self.epub
                                .chapters
                                .get(selected_index)
                                .map_or("", |ch| &ch.title)
                        );
                        // floating_pane remains None
                        self.parse_and_jump_to_chapter(&selected_text);
                        true
                    }
                    _ => {
                        self.floating_pane = FloatingPane::Contents { selected_index };
                        true
                    }
                }
            }
        }
    }

    fn open_search_pane(&mut self) {
        let results = self.build_search_items();
        self.floating_pane = FloatingPane::Search {
            query: String::new(),
            results,
            selected_index: 0,
        };
    }

    fn open_contents_pane(&mut self) {
        self.floating_pane = FloatingPane::Contents {
            selected_index: self.nav_state.current_chapter,
        };
    }

    fn filter_search_results(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            self.build_search_items()
        } else {
            let all_items = self.build_search_items();
            let query_lower = query.to_lowercase();
            all_items
                .into_iter()
                .filter(|item| item.to_lowercase().contains(&query_lower))
                .collect()
        }
    }

    fn render_floating_pane(f: &mut Frame, floating_pane: &FloatingPane, epub: &EpubReader) {
        match floating_pane {
            FloatingPane::None => {}
            FloatingPane::Search {
                query,
                results,
                selected_index,
            } => {
                Self::render_search_pane(f, query, results, *selected_index);
            }
            FloatingPane::Contents { selected_index } => {
                Self::render_contents_pane(f, epub, *selected_index);
            }
        }
    }

    fn render_search_pane(f: &mut Frame, query: &str, results: &[String], selected_index: usize) {
        let area = f.area();

        // Create centered popup area (80% width, 60% height)
        let popup_width = area.width.saturating_mul(80).saturating_div(100);
        let popup_height = area.height.saturating_mul(60).saturating_div(100);
        let x = area.width.saturating_sub(popup_width).saturating_div(2);
        let y = area.height.saturating_sub(popup_height).saturating_div(2);

        let popup_area = Rect {
            x,
            y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the popup area
        f.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search input
                Constraint::Min(0),    // Results
            ])
            .split(popup_area);

        // Search input box
        let input = Paragraph::new(format!("Search: {}", query))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Search Content")
                    .style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(input, chunks[0]);

        // Results list
        let items: Vec<ListItem> = results
            .iter()
            .map(|result| ListItem::new(result.as_str()))
            .collect();

        let results_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Results ({}/{})",
                if results.is_empty() { 0 } else { selected_index + 1 },
                results.len()
            )))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black))
            .highlight_symbol("▶ ");

        let mut list_state = ListState::default();
        list_state.select(if results.is_empty() { None } else { Some(selected_index) });

        f.render_stateful_widget(results_list, chunks[1], &mut list_state);
    }

    fn render_contents_pane(f: &mut Frame, epub: &EpubReader, selected_index: usize) {
        let area = f.area();

        // Create centered popup area (60% width, 50% height)
        let popup_width = area.width.saturating_mul(60).saturating_div(100);
        let popup_height = area.height.saturating_mul(50).saturating_div(100);
        let x = area.width.saturating_sub(popup_width).saturating_div(2);
        let y = area.height.saturating_sub(popup_height).saturating_div(2);

        let popup_area = Rect {
            x,
            y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the popup area
        f.render_widget(Clear, popup_area);

        // Contents list
        let items: Vec<ListItem> = epub
            .chapters
            .iter()
            .enumerate()
            .map(|(i, chapter)| {
                let style = if i == selected_index {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}: {}", i + 1, chapter.title)).style(style)
            })
            .collect();

        let contents_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Table of Contents")
                    .style(Style::default().fg(Color::White)),
            )
            .style(Style::default().fg(Color::White));

        f.render_widget(contents_list, popup_area);
    }
}
