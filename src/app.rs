use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal::size;

const MIN_COLS: u16 = 40;
const MIN_ROWS: u16 = 10;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::config::Config;
use crate::editor::Editor;
use crate::input::{self, Action, InputState, Mode};
use crate::search::Search;
use crate::session::Session;
use crate::stats::StatsTracker;
use crate::ui::{self, RenderState};

/// Overlay state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overlay {
    None,
    Help,
    Stats,
    QuitConfirm,
}

/// Main application state
pub struct App {
    pub editor: Editor,
    pub config: Config,
    pub session: Session,
    pub search: Search,
    pub mode: Mode,
    pub input_state: InputState,
    pub file_path: PathBuf,
    pub show_status: bool,
    pub status_timer: Option<Instant>,
    pub overlay: Overlay,
    pub search_input: String,
    pub should_quit: bool,
    pub last_save: Instant,
    pub saved_indicator: Option<Instant>, // Shows "Saved" briefly per spec 5.3
    pub terminal_too_small: bool,
    pub stats: Option<StatsTracker>,
    pub streak: usize,
    pub writing_stats: Option<crate::stats::WritingStats>,
}

impl App {
    /// Create a new application instance
    pub fn new(file_path: PathBuf, config: Config) -> io::Result<Self> {
        let mut editor = Editor::new();
        editor.load(&file_path)?;

        let initial_word_count = editor.word_count();
        let session = Session::new(initial_word_count);
        
        // Initialize stats tracker if daily goal is set
        let (stats, streak) = if config.goals.daily_goal > 0 {
            match StatsTracker::new(config.goals.daily_goal) {
                Ok(tracker) => {
                    let streak = tracker.get_streak().unwrap_or(0);
                    (Some(tracker), streak)
                }
                Err(_) => (None, 0),
            }
        } else {
            (None, 0)
        };

        Ok(Self {
            editor,
            session,
            search: Search::new(),
            mode: Mode::Write,
            input_state: InputState::default(),
            file_path,
            show_status: config.display.show_status,
            status_timer: None,
            overlay: Overlay::None,
            search_input: String::new(),
            should_quit: false,
            last_save: Instant::now(),
            saved_indicator: None,
            terminal_too_small: false,
            writing_stats: None,
            stats,
            streak,
            config,
        })
    }

    /// Run the main application loop
    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            // Check terminal size (spec 10.2)
            if let Ok((cols, rows)) = size() {
                self.terminal_too_small = cols < MIN_COLS || rows < MIN_ROWS;
            }

            // Render
            terminal.draw(|f| {
                // Show size warning if terminal too small
                if self.terminal_too_small {
                    let area = f.area();
                    let msg = format!(
                        "Terminal too small\nMinimum: {}x{}",
                        MIN_COLS, MIN_ROWS
                    );
                    let paragraph = ratatui::widgets::Paragraph::new(msg)
                        .alignment(ratatui::layout::Alignment::Center);
                    f.render_widget(paragraph, area);
                    return;
                }

                let content = self.editor.content().to_string();
                let (cursor_line, cursor_col) = self.editor.cursor_position();
                let matches = self.search.all_matches(self.editor.content());

                let word_count = self.editor.word_count();
                let (goal_progress, goal_met) = if let Some(ref stats) = self.stats {
                    (stats.get_progress(word_count), stats.is_goal_met(word_count))
                } else {
                    (0.0, false)
                };
                
                let state = RenderState {
                    content: &content,
                    cursor_line,
                    cursor_col,
                    mode: self.mode,
                    word_count,
                    elapsed: &self.session.elapsed_formatted(),
                    modified: self.editor.is_modified(),
                    show_status: self.show_status,
                    show_help: self.overlay == Overlay::Help,
                    show_quit_confirm: self.overlay == Overlay::QuitConfirm,
                    show_stats: self.overlay == Overlay::Stats,
                    search_active: self.mode == Mode::Search,
                    search_query: &self.search_input,
                    search_matches: &matches,
                    text_width: self.config.editor.text_width,
                    show_saved_indicator: self.saved_indicator.is_some(),
                    daily_goal: self.config.goals.daily_goal,
                    goal_progress,
                    streak: self.streak,
                    goal_met,
                    show_goal: self.config.goals.show_progress || self.config.goals.show_streak,
                    writing_stats: self.writing_stats.as_ref(),
                };

                ui::render(f, &state);
            })?;

            // Poll for events
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
            }

            // Check auto-save
            self.check_auto_save()?;

            // Check status timeout
            self.check_status_timeout();

            // Exit if requested
            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Handle quit confirmation overlay specially
        if self.overlay == Overlay::QuitConfirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let _ = self.editor.save(&self.file_path);
                    self.should_quit = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.should_quit = true;
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                    self.overlay = Overlay::None;
                }
                _ => {}
            }
            return;
        }

        // Handle help overlay
        if self.overlay == Overlay::Help {
            self.overlay = Overlay::None;
            return;
        }
        
        // Handle stats overlay
        if self.overlay == Overlay::Stats {
            self.overlay = Overlay::None;
            return;
        }

        // Normal key handling
        let action = input::handle_key(key, self.mode, &mut self.input_state);
        self.handle_action(action);
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.try_quit(),
            Action::Save => {
                if self.editor.save(&self.file_path).is_ok() {
                    self.last_save = Instant::now();
                    self.saved_indicator = Some(Instant::now());
                    self.record_stats();
                }
            }

            // Text input (create backup on first edit per spec 5.4)
            Action::InsertChar(c) => {
                let _ = self.editor.create_backup_if_needed(&self.file_path);
                self.editor.insert_char(c);
            }
            Action::InsertNewline => {
                let _ = self.editor.create_backup_if_needed(&self.file_path);
                self.editor.insert_newline();
            }
            Action::DeleteChar => {
                let _ = self.editor.create_backup_if_needed(&self.file_path);
                self.editor.delete_char();
            }
            Action::DeleteCharForward => {
                let _ = self.editor.create_backup_if_needed(&self.file_path);
                self.editor.delete_char_forward();
            }

            // Movement
            Action::MoveCursor(dir, unit) => self.editor.move_cursor(dir, unit),

            // Line operations
            Action::DeleteLine => {
                let _ = self.editor.create_backup_if_needed(&self.file_path);
                self.editor.delete_line();
            }
            Action::CopyLine => self.editor.copy_line(),
            Action::Paste => {
                let _ = self.editor.create_backup_if_needed(&self.file_path);
                self.editor.paste();
            }

            // Undo/redo
            Action::Undo => self.editor.undo(),
            Action::Redo => self.editor.redo(),

            // Mode changes
            Action::EnterNavigateMode => self.mode = Mode::Navigate,
            Action::EnterWriteMode => self.mode = Mode::Write,
            Action::EnterWriteModeWithChar(c) => {
                self.mode = Mode::Write;
                self.editor.insert_char(c);
            }

            // UI
            Action::ToggleStatus => {
                self.show_status = !self.show_status;
                if self.show_status {
                    self.status_timer = Some(Instant::now());
                }
            }
            Action::ShowHelp => self.overlay = Overlay::Help,
            Action::ShowStats => {
                // Refresh stats before showing
                if let Some(ref stats) = self.stats {
                    self.writing_stats = stats.get_writing_stats().ok();
                }
                self.overlay = Overlay::Stats;
            }
            Action::HideOverlay => self.overlay = Overlay::None,

            // Search
            Action::StartSearch => {
                self.mode = Mode::Search;
                self.search_input.clear();
            }
            Action::SubmitSearch => {
                self.search.set_query(&self.search_input);
                self.mode = Mode::Navigate;
                // Find first match from cursor position
                self.jump_to_next_match();
            }
            Action::CancelSearch => {
                self.mode = Mode::Navigate;
                self.search_input.clear();
                self.search.clear();
            }
            Action::SearchNext => {
                if self.search.is_active() {
                    self.jump_to_next_match();
                }
            }
            Action::SearchPrev => {
                if self.search.is_active() {
                    self.jump_to_prev_match();
                }
            }
            Action::SearchInput(c) => self.search_input.push(c),
            Action::SearchBackspace => {
                self.search_input.pop();
            }
        }

        // Update session word count
        self.session.update_word_count(self.editor.word_count());
    }

    fn try_quit(&mut self) {
        if self.editor.is_modified() {
            self.overlay = Overlay::QuitConfirm;
        } else {
            self.should_quit = true;
        }
    }

    fn check_auto_save(&mut self) -> io::Result<()> {
        if self.config.editor.auto_save_seconds == 0 {
            return Ok(());
        }

        let elapsed = self.last_save.elapsed().as_secs();
        if elapsed >= self.config.editor.auto_save_seconds && self.editor.is_modified() {
            self.editor.save(&self.file_path)?;
            self.last_save = Instant::now();
            self.saved_indicator = Some(Instant::now()); // Show "Saved" indicator per spec 5.3
            
            // Record stats on save
            self.record_stats();
        }

        // Clear saved indicator after 2 seconds
        if let Some(saved_time) = self.saved_indicator {
            if saved_time.elapsed().as_secs() >= 2 {
                self.saved_indicator = None;
            }
        }

        Ok(())
    }
    
    /// Record writing stats to database
    fn record_stats(&mut self) {
        if let Some(ref stats) = self.stats {
            let word_count = self.editor.word_count();
            let _ = stats.record_words(word_count);
            
            // Update streak if goal was just met
            if stats.is_goal_met(word_count) {
                self.streak = stats.get_streak().unwrap_or(self.streak);
            }
        }
    }

    fn check_status_timeout(&mut self) {
        if self.config.display.status_timeout == 0 {
            return;
        }

        if let Some(timer) = self.status_timer {
            if timer.elapsed().as_secs() >= self.config.display.status_timeout {
                self.show_status = false;
                self.status_timer = None;
            }
        }
    }

    /// Jump to next search match
    fn jump_to_next_match(&mut self) {
        let cursor_char = self.cursor_to_char_pos();
        if let Some((start, _)) = self.search.find_next(self.editor.content(), cursor_char + 1) {
            self.set_cursor_from_char_pos(start);
        }
    }

    /// Jump to previous search match
    fn jump_to_prev_match(&mut self) {
        let cursor_char = self.cursor_to_char_pos();
        if let Some((start, _)) = self.search.find_prev(self.editor.content(), cursor_char) {
            self.set_cursor_from_char_pos(start);
        }
    }

    /// Convert current cursor position to char offset in rope
    fn cursor_to_char_pos(&self) -> usize {
        let content = self.editor.content();
        let (line, col) = self.editor.cursor_position();
        if line >= content.len_lines() {
            return content.len_chars();
        }
        let line_start = content.line_to_char(line);
        let line_len = content.line(line).len_chars();
        line_start + col.min(line_len)
    }

    /// Set cursor position from char offset in rope
    fn set_cursor_from_char_pos(&mut self, char_pos: usize) {
        let content = self.editor.content();
        let clamped = char_pos.min(content.len_chars());
        let target_line = content.char_to_line(clamped);
        let line_start = content.line_to_char(target_line);
        let target_col = clamped - line_start;

        // Move to document start first, then navigate to target
        self.editor.move_cursor(crate::editor::Direction::Up, crate::editor::Unit::Document);
        for _ in 0..target_line {
            self.editor.move_cursor(crate::editor::Direction::Down, crate::editor::Unit::Line);
        }
        self.editor.move_cursor(crate::editor::Direction::Left, crate::editor::Unit::Line);
        for _ in 0..target_col {
            self.editor.move_cursor(crate::editor::Direction::Right, crate::editor::Unit::Char);
        }
    }
}
