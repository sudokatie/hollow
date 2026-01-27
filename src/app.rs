use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::config::Config;
use crate::editor::Editor;
use crate::input::{self, Action, InputState, Mode};
use crate::search::Search;
use crate::session::Session;
use crate::ui::{self, RenderState};

/// Overlay state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overlay {
    None,
    Help,
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
}

impl App {
    /// Create a new application instance
    pub fn new(file_path: PathBuf, config: Config) -> io::Result<Self> {
        let mut editor = Editor::new();
        editor.load(&file_path)?;

        let initial_word_count = editor.word_count();
        let session = Session::new(initial_word_count);

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
            config,
        })
    }

    /// Run the main application loop
    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            // Render
            terminal.draw(|f| {
                let content = self.editor.content().to_string();
                let (cursor_line, cursor_col) = self.editor.cursor_position();
                let matches = self.search.all_matches(self.editor.content());

                let state = RenderState {
                    content: &content,
                    cursor_line,
                    cursor_col,
                    mode: self.mode,
                    word_count: self.editor.word_count(),
                    elapsed: &self.session.elapsed_formatted(),
                    modified: self.editor.is_modified(),
                    show_status: self.show_status,
                    show_help: self.overlay == Overlay::Help,
                    show_quit_confirm: self.overlay == Overlay::QuitConfirm,
                    search_active: self.mode == Mode::Search,
                    search_query: &self.search_input,
                    search_matches: &matches,
                    text_width: self.config.editor.text_width,
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

        // Normal key handling
        let action = input::handle_key(key, self.mode, &mut self.input_state);
        self.handle_action(action);
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.try_quit(),
            Action::Save => {
                let _ = self.editor.save(&self.file_path);
                self.last_save = Instant::now();
            }

            // Text input
            Action::InsertChar(c) => self.editor.insert_char(c),
            Action::InsertNewline => self.editor.insert_newline(),
            Action::DeleteChar => self.editor.delete_char(),
            Action::DeleteCharForward => self.editor.delete_char_forward(),

            // Movement
            Action::MoveCursor(dir, unit) => self.editor.move_cursor(dir, unit),

            // Line operations
            Action::DeleteLine => self.editor.delete_line(),
            Action::CopyLine => self.editor.copy_line(),
            Action::Paste => self.editor.paste(),

            // Undo/redo
            Action::Undo => self.editor.undo(),
            Action::Redo => self.editor.redo(),

            // Mode changes
            Action::EnterNavigateMode => self.mode = Mode::Navigate,
            Action::EnterWriteMode => self.mode = Mode::Write,

            // UI
            Action::ToggleStatus => {
                self.show_status = !self.show_status;
                if self.show_status {
                    self.status_timer = Some(Instant::now());
                }
            }
            Action::ShowHelp => self.overlay = Overlay::Help,
            Action::HideOverlay => self.overlay = Overlay::None,

            // Search
            Action::StartSearch => {
                self.mode = Mode::Search;
                self.search_input.clear();
            }
            Action::SubmitSearch => {
                self.search.set_query(&self.search_input);
                self.mode = Mode::Navigate;
                // Find first match
                let (_, cursor_col) = self.editor.cursor_position();
                if let Some((start, _)) = self.search.find_next(self.editor.content(), cursor_col) {
                    self.editor
                        .move_cursor(crate::editor::Direction::Up, crate::editor::Unit::Document);
                    // TODO: Move to match position
                }
            }
            Action::CancelSearch => {
                self.mode = Mode::Navigate;
                self.search_input.clear();
            }
            Action::SearchNext => {
                if self.search.is_active() {
                    // TODO: Implement search navigation
                }
            }
            Action::SearchPrev => {
                if self.search.is_active() {
                    // TODO: Implement search navigation
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
        }

        Ok(())
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
}
