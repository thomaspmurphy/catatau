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
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Padding, Paragraph,
        Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
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
                if self.handle_floating_pane_input(key) {
                    continue;
                }

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
                Constraint::Length(4), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Footer with progress
            ])
            .split(f.area());

        // Modern header with rounded borders and better styling
        let title_line = Line::from(vec![
            Span::styled(
                "📖 ",
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                &epub.title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let author_line = Line::from(vec![
            Span::styled("   by ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &epub.author,
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]);

        let header = Paragraph::new(vec![title_line, author_line])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .padding(Padding::horizontal(1)),
            )
            .alignment(Alignment::Left);
        f.render_widget(header, chunks[0]);

        if let Ok(chapter) = epub.get_chapter(current_chapter) {
            let total_lines = chapter.content.lines().count();
            let visible_lines = terminal_height.saturating_sub(UI_RESERVED_HEIGHT);

            let lines: Vec<Line> = if let Some(search_term) = highlighted_search_term {
                chapter
                    .content
                    .lines()
                    .skip(scroll_offset)
                    .take(visible_lines)
                    .map(|line| Self::highlight_line(line, search_term))
                    .collect()
            } else {
                chapter
                    .content
                    .lines()
                    .skip(scroll_offset)
                    .take(visible_lines)
                    .map(|line| Self::style_line(line))
                    .collect()
            };

            let chapter_title = format!("│ {} ", chapter.title);
            let content = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Blue))
                        .title(chapter_title)
                        .title_style(Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD))
                        .padding(Padding::new(2, 1, 0, 0)),
                )
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: false });
            f.render_widget(content, chunks[1]);

            // Render scrollbar indicator
            if total_lines > visible_lines {
                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"))
                    .track_symbol(Some("│"))
                    .thumb_symbol("█")
                    .style(Style::default().fg(Color::Cyan));

                let mut scrollbar_state = ScrollbarState::new(total_lines.saturating_sub(visible_lines))
                    .position(scroll_offset);

                let scrollbar_area = Rect {
                    x: chunks[1].x + chunks[1].width.saturating_sub(1),
                    y: chunks[1].y + 1,
                    width: 1,
                    height: chunks[1].height.saturating_sub(2),
                };

                f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
            }
        }

        // Modern footer with progress bar and icons
        let chapter_progress = if epub.chapter_count() > 0 {
            ((current_chapter + 1) as f64 / epub.chapter_count() as f64) * 100.0
        } else {
            0.0
        };

        let footer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Progress bar
                Constraint::Length(2), // Help text
            ])
            .split(chunks[2]);

        // Progress bar
        let progress_label = format!("Chapter {}/{}", current_chapter + 1, epub.chapter_count());
        let progress = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .percent(chapter_progress as u16)
            .label(progress_label);
        f.render_widget(progress, footer_chunks[0]);

        // Help text with icons
        let help_text = vec![
            Line::from(vec![
                Span::styled(" q", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(":quit ", Style::default().fg(Color::DarkGray)),
                Span::styled("↑↓", Style::default().fg(Color::Cyan)),
                Span::styled(":scroll ", Style::default().fg(Color::DarkGray)),
                Span::styled("←→", Style::default().fg(Color::Green)),
                Span::styled(":chapter ", Style::default().fg(Color::DarkGray)),
                Span::styled("⎵", Style::default().fg(Color::Yellow)),
                Span::styled(":page ", Style::default().fg(Color::DarkGray)),
                Span::styled("/", Style::default().fg(Color::Magenta)),
                Span::styled(":search ", Style::default().fg(Color::DarkGray)),
                Span::styled("-", Style::default().fg(Color::Blue)),
                Span::styled(":contents", Style::default().fg(Color::DarkGray)),
            ]),
        ];
        let footer = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray))
            )
            .alignment(Alignment::Center);
        f.render_widget(footer, footer_chunks[1]);

        Self::render_floating_pane(f, floating_pane, epub);
    }

    fn style_line(line: &str) -> Line<'static> {
        let trimmed = line.trim_start();

        // Detect markdown-style headers
        if trimmed.starts_with("# ") {
            let text = trimmed[2..].to_string();
            return Line::from(vec![Span::styled(
                text,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]);
        } else if trimmed.starts_with("## ") {
            let text = trimmed[3..].to_string();
            return Line::from(vec![Span::styled(
                text,
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )]);
        } else if trimmed.starts_with("### ") {
            let text = trimmed[4..].to_string();
            return Line::from(vec![Span::styled(
                text,
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            )]);
        } else if trimmed.starts_with("#### ") || trimmed.starts_with("##### ") || trimmed.starts_with("###### ") {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed[hash_count + 1..].to_string();
            return Line::from(vec![Span::styled(
                text,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )]);
        }

        // Parse inline formatting (**bold**, *italic*)
        Self::parse_inline_formatting(line)
    }

    fn parse_inline_formatting(text: &str) -> Line<'static> {
        let mut spans = Vec::new();
        let mut current_text = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '*' {
                if chars.peek() == Some(&'*') {
                    // Handle **bold**
                    chars.next();
                    if !current_text.is_empty() {
                        spans.push(Span::styled(
                            current_text.clone(),
                            Style::default().fg(Color::White),
                        ));
                        current_text.clear();
                    }
                    let mut bold_text = String::new();
                    let mut found_close = false;
                    while let Some(ch2) = chars.next() {
                        if ch2 == '*' && chars.peek() == Some(&'*') {
                            chars.next();
                            found_close = true;
                            break;
                        }
                        bold_text.push(ch2);
                    }
                    if found_close && !bold_text.is_empty() {
                        spans.push(Span::styled(
                            bold_text,
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                } else {
                    // Handle *italic*
                    if !current_text.is_empty() {
                        spans.push(Span::styled(
                            current_text.clone(),
                            Style::default().fg(Color::White),
                        ));
                        current_text.clear();
                    }
                    let mut italic_text = String::new();
                    let mut found_close = false;
                    while let Some(ch2) = chars.next() {
                        if ch2 == '*' {
                            found_close = true;
                            break;
                        }
                        italic_text.push(ch2);
                    }
                    if found_close && !italic_text.is_empty() {
                        spans.push(Span::styled(
                            italic_text,
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::ITALIC),
                        ));
                    }
                }
            } else {
                current_text.push(ch);
            }
        }

        if !current_text.is_empty() {
            spans.push(Span::styled(
                current_text,
                Style::default().fg(Color::White),
            ));
        }

        if spans.is_empty() {
            Line::from(vec![Span::styled(
                text.to_string(),
                Style::default().fg(Color::White),
            )])
        } else {
            Line::from(spans)
        }
    }

    fn highlight_line(line: &str, search_term: &str) -> Line<'static> {
        let search_lower = search_term.to_lowercase();

        // First check if this is a header
        let trimmed = line.trim_start();
        let (is_header, header_level, text_after_hash) = if trimmed.starts_with("# ") {
            (true, 1, trimmed[2..].to_string())
        } else if trimmed.starts_with("## ") {
            (true, 2, trimmed[3..].to_string())
        } else if trimmed.starts_with("### ") {
            (true, 3, trimmed[4..].to_string())
        } else {
            (false, 0, line.to_string())
        };

        let text_to_search = if is_header { &text_after_hash } else { line };
        let text_lower = text_to_search.to_lowercase();

        if let Some(pos) = text_lower.find(&search_lower) {
            let mut spans = Vec::new();

            let base_style = if is_header {
                match header_level {
                    1 => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    2 => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                    3 => Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD),
                    _ => Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                }
            } else {
                Style::default().fg(Color::White)
            };

            if pos > 0 {
                spans.push(Span::styled(
                    text_to_search[..pos].to_string(),
                    base_style,
                ));
            }

            let end_pos = pos + search_term.len();
            spans.push(Span::styled(
                text_to_search[pos..end_pos.min(text_to_search.len())].to_string(),
                Style::default().bg(Color::Yellow).fg(Color::Black),
            ));

            if end_pos < text_to_search.len() {
                spans.push(Span::styled(
                    text_to_search[end_pos..].to_string(),
                    base_style,
                ));
            }

            Line::from(spans)
        } else {
            Self::style_line(line)
        }
    }

    fn get_page_size(&self) -> usize {
        self.terminal_height.saturating_sub(UI_RESERVED_HEIGHT)
    }

    fn get_max_scroll_for_chapter(&self, chapter_index: usize) -> usize {
        if let Ok(chapter) = self.epub.get_chapter(chapter_index) {
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
        if self.nav_state.current_chapter < self.epub.chapter_count().saturating_sub(1) {
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
        for chapter_index in 0..self.epub.chapter_count() {
            if let Ok(chapter) = self.epub.get_chapter(chapter_index) {
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
        if location.chapter == 0 || location.chapter > self.epub.chapter_count() {
            return;
        }

        self.nav_state.current_chapter = location.chapter - 1;

        if self.epub.get_chapter(self.nav_state.current_chapter).is_ok() {
            let target_line = location.line.saturating_sub(1);
            self.nav_state.scroll_offset = target_line.saturating_sub(SEARCH_RESULT_TOP_OFFSET);
            self.clamp_scroll_to_limits(self.nav_state.current_chapter);

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
        let colon_pos = text.find(": ")?;

        let chapter_str = text[0..colon_pos].trim();
        let chapter = chapter_str.parse().ok()?;

        Some(ChapterLocation { chapter })
    }

    fn jump_to_chapter_location(&mut self, location: ChapterLocation) {
        if location.chapter == 0 || location.chapter > self.epub.chapter_count() {
            return;
        }

        self.nav_state.current_chapter = location.chapter - 1;
        self.nav_state.reset_scroll();
    }

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
                    KeyCode::Esc => true,
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
                    KeyCode::Esc => true,
                    KeyCode::Up => {
                        selected_index = selected_index.saturating_sub(1);
                        self.floating_pane = FloatingPane::Contents { selected_index };
                        true
                    }
                    KeyCode::Down => {
                        if selected_index < self.epub.chapter_count().saturating_sub(1) {
                            selected_index += 1;
                        }
                        self.floating_pane = FloatingPane::Contents { selected_index };
                        true
                    }
                    KeyCode::Enter => {
                        let title = self
                            .epub
                            .get_chapter(selected_index)
                            .map(|ch| ch.title)
                            .unwrap_or_else(|_| String::from(""));
                        let selected_text = format!("{}: {}", selected_index + 1, title);
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

        // Render shadow effect
        let shadow_area = Rect {
            x: x + 1,
            y: y + 1,
            width: popup_width,
            height: popup_height,
        };
        f.render_widget(
            Block::default().style(Style::default().bg(Color::Black)),
            shadow_area,
        );

        f.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search input
                Constraint::Min(0),    // Results
                Constraint::Length(1), // Help text
            ])
            .split(popup_area);

        // Search input with blinking cursor effect
        let cursor = if std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1000
            < 500
        {
            "█"
        } else {
            " "
        };

        let input = Paragraph::new(format!("🔍 Search: {}{}", query, cursor))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title("Search Content")
                    .style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(input, chunks[0]);

        let items: Vec<ListItem> = results
            .iter()
            .map(|result| ListItem::new(result.as_str()))
            .collect();

        let results_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(format!(
                        "Results ({}/{})",
                        if results.is_empty() { 0 } else { selected_index + 1 },
                        results.len()
                    )),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        let mut list_state = ListState::default();
        list_state.select(if results.is_empty() { None } else { Some(selected_index) });

        f.render_stateful_widget(results_list, chunks[1], &mut list_state);

        // Help text
        let help = Paragraph::new(Line::from(vec![
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" select  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" close"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(help, chunks[2]);
    }

    fn render_contents_pane(f: &mut Frame, epub: &EpubReader, selected_index: usize) {
        let area = f.area();

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

        // Render shadow effect
        let shadow_area = Rect {
            x: x + 1,
            y: y + 1,
            width: popup_width,
            height: popup_height,
        };
        f.render_widget(
            Block::default().style(Style::default().bg(Color::Black)),
            shadow_area,
        );

        f.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(popup_area);

        let items: Vec<ListItem> = (0..epub.chapter_count())
            .filter_map(|i| {
                epub.get_chapter(i)
                    .ok()
                    .map(|chapter| ListItem::new(format!("{}: {}", i + 1, chapter.title)))
            })
            .collect();

        let contents_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Blue))
                    .title(format!("📑 Table of Contents ({} chapters)", epub.chapter_count()))
                    .style(Style::default().fg(Color::Blue)),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        let mut list_state = ListState::default();
        list_state.select(Some(selected_index));

        f.render_stateful_widget(contents_list, chunks[0], &mut list_state);

        // Help text
        let help = Paragraph::new(Line::from(vec![
            Span::styled("↑↓", Style::default().fg(Color::Blue)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Blue)),
            Span::raw(" select  "),
            Span::styled("Esc", Style::default().fg(Color::Blue)),
            Span::raw(" close"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(help, chunks[1]);
    }
}
