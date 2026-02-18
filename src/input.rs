use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::{Direction, Unit};

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Write,
    Navigate,
    Search,
}

/// Actions that can be performed
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    Quit,
    Save,
    // Text input
    InsertChar(char),
    InsertNewline,
    DeleteChar,
    DeleteCharForward,
    // Movement
    MoveCursor(Direction, Unit),
    // Line operations
    DeleteLine,
    CopyLine,
    Paste,
    // Undo/redo
    Undo,
    Redo,
    // Mode changes
    EnterNavigateMode,
    EnterWriteMode,
    EnterWriteModeWithChar(char), // Enter write mode and insert char (per spec 3.1)
    // UI
    ToggleStatus,
    ToggleSpellCheck,
    ShowHelp,
    ShowStats,
    ShowVersions,
    ShowProjectDocs,
    HideOverlay,
    // Search
    StartSearch,
    SubmitSearch,
    CancelSearch,
    SearchNext,
    SearchPrev,
    SearchInput(char),
    SearchBackspace,
}

/// State for multi-key sequences
#[derive(Debug, Default)]
pub struct InputState {
    pub pending_g: bool,
    pub pending_d: bool,
    pub pending_y: bool,
}

impl InputState {
    pub fn clear(&mut self) {
        self.pending_g = false;
        self.pending_d = false;
        self.pending_y = false;
    }
}

/// Handle a key event and return the corresponding action
pub fn handle_key(key: KeyEvent, mode: Mode, state: &mut InputState) -> Action {
    // Universal bindings (work in all modes except search)
    if mode != Mode::Search {
        if let Some(action) = handle_universal(key) {
            state.clear();
            return action;
        }
    }

    match mode {
        Mode::Write => handle_write_mode(key, state),
        Mode::Navigate => handle_navigate_mode(key, state),
        Mode::Search => handle_search_mode(key, state),
    }
}

fn handle_universal(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => Some(Action::Save),
        (KeyCode::Char('q'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Char('g'), KeyModifiers::CONTROL) => Some(Action::ToggleStatus),
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => Some(Action::Undo),
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => Some(Action::Redo),
        (KeyCode::Char(';'), KeyModifiers::CONTROL) => Some(Action::ToggleSpellCheck),
        _ => None,
    }
}

fn handle_write_mode(key: KeyEvent, state: &mut InputState) -> Action {
    state.clear();

    match key.code {
        KeyCode::Esc => Action::EnterNavigateMode,
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Action::InsertChar(c)
        }
        KeyCode::Enter => Action::InsertNewline,
        KeyCode::Backspace => Action::DeleteChar,
        KeyCode::Delete => Action::DeleteCharForward,
        // Arrow keys
        KeyCode::Left => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Action::MoveCursor(Direction::Left, Unit::Word)
            } else {
                Action::MoveCursor(Direction::Left, Unit::Char)
            }
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Action::MoveCursor(Direction::Right, Unit::Word)
            } else {
                Action::MoveCursor(Direction::Right, Unit::Char)
            }
        }
        KeyCode::Up => Action::MoveCursor(Direction::Up, Unit::Line),
        KeyCode::Down => Action::MoveCursor(Direction::Down, Unit::Line),
        KeyCode::Home => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Action::MoveCursor(Direction::Up, Unit::Document)
            } else {
                Action::MoveCursor(Direction::Left, Unit::Line)
            }
        }
        KeyCode::End => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Action::MoveCursor(Direction::Down, Unit::Document)
            } else {
                Action::MoveCursor(Direction::Right, Unit::Line)
            }
        }
        KeyCode::PageUp => Action::MoveCursor(Direction::Up, Unit::Page(20)),
        KeyCode::PageDown => Action::MoveCursor(Direction::Down, Unit::Page(20)),
        _ => Action::None,
    }
}

fn handle_navigate_mode(key: KeyEvent, state: &mut InputState) -> Action {
    // Handle pending sequences first
    if state.pending_g {
        state.pending_g = false;
        if key.code == KeyCode::Char('g') {
            return Action::MoveCursor(Direction::Up, Unit::Document);
        }
        // Invalid sequence, fall through
    }

    if state.pending_d {
        state.pending_d = false;
        if key.code == KeyCode::Char('d') {
            return Action::DeleteLine;
        }
        // Invalid sequence, fall through
    }

    if state.pending_y {
        state.pending_y = false;
        if key.code == KeyCode::Char('y') {
            return Action::CopyLine;
        }
        // Invalid sequence, fall through
    }

    match key.code {
        // Mode changes
        KeyCode::Char('i') => Action::EnterWriteMode,
        KeyCode::Esc => Action::HideOverlay,

        // Movement - vim style
        KeyCode::Char('h') => Action::MoveCursor(Direction::Left, Unit::Char),
        KeyCode::Char('j') => Action::MoveCursor(Direction::Down, Unit::Line),
        KeyCode::Char('k') => Action::MoveCursor(Direction::Up, Unit::Line),
        KeyCode::Char('l') => Action::MoveCursor(Direction::Right, Unit::Char),

        // Word movement
        KeyCode::Char('w') => Action::MoveCursor(Direction::Right, Unit::Word),
        KeyCode::Char('b') => Action::MoveCursor(Direction::Left, Unit::Word),

        // Paragraph movement (spec 4.1)
        KeyCode::Char('{') => Action::MoveCursor(Direction::Up, Unit::Paragraph),
        KeyCode::Char('}') => Action::MoveCursor(Direction::Down, Unit::Paragraph),

        // Line movement
        KeyCode::Char('0') => Action::MoveCursor(Direction::Left, Unit::Line),
        KeyCode::Char('$') => Action::MoveCursor(Direction::Right, Unit::Line),

        // Document movement
        KeyCode::Char('g') => {
            state.pending_g = true;
            Action::None
        }
        KeyCode::Char('G') => Action::MoveCursor(Direction::Down, Unit::Document),

        // Line operations
        KeyCode::Char('d') => {
            state.pending_d = true;
            Action::None
        }
        KeyCode::Char('y') => {
            state.pending_y = true;
            Action::None
        }
        KeyCode::Char('p') => Action::Paste,

        // Undo/redo
        KeyCode::Char('u') => Action::Undo,
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Redo,

        // Search
        KeyCode::Char('/') => Action::StartSearch,
        KeyCode::Char('n') => Action::SearchNext,
        KeyCode::Char('N') => Action::SearchPrev,

        // Help, Stats, Versions, and Projects
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Char('s') => Action::ShowStats,
        KeyCode::Char('v') => Action::ShowVersions,
        KeyCode::Char('P') => Action::ShowProjectDocs,

        // Arrow keys (also work in navigate mode)
        KeyCode::Left => Action::MoveCursor(Direction::Left, Unit::Char),
        KeyCode::Right => Action::MoveCursor(Direction::Right, Unit::Char),
        KeyCode::Up => Action::MoveCursor(Direction::Up, Unit::Line),
        KeyCode::Down => Action::MoveCursor(Direction::Down, Unit::Line),
        KeyCode::Home => Action::MoveCursor(Direction::Left, Unit::Line),
        KeyCode::End => Action::MoveCursor(Direction::Right, Unit::Line),
        KeyCode::PageUp => Action::MoveCursor(Direction::Up, Unit::Page(20)),
        KeyCode::PageDown => Action::MoveCursor(Direction::Down, Unit::Page(20)),

        // Per spec 3.1: Any printable character returns to Write mode AND inserts
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Action::EnterWriteModeWithChar(c)
        }

        _ => Action::None,
    }
}

fn handle_search_mode(key: KeyEvent, state: &mut InputState) -> Action {
    state.clear();

    match key.code {
        KeyCode::Esc => Action::CancelSearch,
        KeyCode::Enter => Action::SubmitSearch,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Action::SearchInput(c)
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn key_char(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn test_ctrl_s_saves() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key_ctrl('s'), Mode::Write, &mut state),
            Action::Save
        );
        assert_eq!(
            handle_key(key_ctrl('s'), Mode::Navigate, &mut state),
            Action::Save
        );
    }

    #[test]
    fn test_ctrl_q_quits() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key_ctrl('q'), Mode::Write, &mut state),
            Action::Quit
        );
    }

    #[test]
    fn test_escape_enters_navigate() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key(KeyCode::Esc), Mode::Write, &mut state),
            Action::EnterNavigateMode
        );
    }

    #[test]
    fn test_i_enters_write() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key_char('i'), Mode::Navigate, &mut state),
            Action::EnterWriteMode
        );
    }

    #[test]
    fn test_char_inserts_in_write_mode() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key_char('a'), Mode::Write, &mut state),
            Action::InsertChar('a')
        );
    }

    #[test]
    fn test_arrow_keys_move() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key(KeyCode::Left), Mode::Write, &mut state),
            Action::MoveCursor(Direction::Left, Unit::Char)
        );
        assert_eq!(
            handle_key(key(KeyCode::Up), Mode::Write, &mut state),
            Action::MoveCursor(Direction::Up, Unit::Line)
        );
    }

    #[test]
    fn test_vim_movement_in_navigate() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key_char('h'), Mode::Navigate, &mut state),
            Action::MoveCursor(Direction::Left, Unit::Char)
        );
        assert_eq!(
            handle_key(key_char('j'), Mode::Navigate, &mut state),
            Action::MoveCursor(Direction::Down, Unit::Line)
        );
        assert_eq!(
            handle_key(key_char('k'), Mode::Navigate, &mut state),
            Action::MoveCursor(Direction::Up, Unit::Line)
        );
        assert_eq!(
            handle_key(key_char('l'), Mode::Navigate, &mut state),
            Action::MoveCursor(Direction::Right, Unit::Char)
        );
    }

    #[test]
    fn test_gg_moves_to_start() {
        let mut state = InputState::default();

        // First g sets pending
        assert_eq!(
            handle_key(key_char('g'), Mode::Navigate, &mut state),
            Action::None
        );
        assert!(state.pending_g);

        // Second g completes the sequence
        assert_eq!(
            handle_key(key_char('g'), Mode::Navigate, &mut state),
            Action::MoveCursor(Direction::Up, Unit::Document)
        );
    }

    #[test]
    fn test_dd_deletes_line() {
        let mut state = InputState::default();

        assert_eq!(
            handle_key(key_char('d'), Mode::Navigate, &mut state),
            Action::None
        );
        assert!(state.pending_d);

        assert_eq!(
            handle_key(key_char('d'), Mode::Navigate, &mut state),
            Action::DeleteLine
        );
    }

    #[test]
    fn test_yy_copies_line() {
        let mut state = InputState::default();

        assert_eq!(
            handle_key(key_char('y'), Mode::Navigate, &mut state),
            Action::None
        );
        assert!(state.pending_y);

        assert_eq!(
            handle_key(key_char('y'), Mode::Navigate, &mut state),
            Action::CopyLine
        );
    }

    #[test]
    fn test_search_mode() {
        let mut state = InputState::default();

        // / starts search
        assert_eq!(
            handle_key(key_char('/'), Mode::Navigate, &mut state),
            Action::StartSearch
        );

        // In search mode, chars are search input
        assert_eq!(
            handle_key(key_char('a'), Mode::Search, &mut state),
            Action::SearchInput('a')
        );

        // Enter submits
        assert_eq!(
            handle_key(key(KeyCode::Enter), Mode::Search, &mut state),
            Action::SubmitSearch
        );

        // Escape cancels
        assert_eq!(
            handle_key(key(KeyCode::Esc), Mode::Search, &mut state),
            Action::CancelSearch
        );
    }

    #[test]
    fn test_ctrl_z_undoes() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key_ctrl('z'), Mode::Write, &mut state),
            Action::Undo
        );
        assert_eq!(
            handle_key(key_ctrl('z'), Mode::Navigate, &mut state),
            Action::Undo
        );
    }

    #[test]
    fn test_help_in_navigate() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
                Mode::Navigate,
                &mut state
            ),
            Action::ShowHelp
        );
    }

    #[test]
    fn test_ctrl_semicolon_toggles_spell() {
        let mut state = InputState::default();
        assert_eq!(
            handle_key(key_ctrl(';'), Mode::Write, &mut state),
            Action::ToggleSpellCheck
        );
        assert_eq!(
            handle_key(key_ctrl(';'), Mode::Navigate, &mut state),
            Action::ToggleSpellCheck
        );
    }
}
