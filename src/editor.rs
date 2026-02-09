use ropey::Rope;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

/// Direction for cursor movement
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Unit of movement
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    Char,
    Word,
    Line,
    Paragraph,
    Page(usize), // page height in lines
    Document,
}

/// Represents an edit operation for undo/redo
#[derive(Debug, Clone)]
enum UndoItem {
    Insert { pos: usize, text: String },
    Delete { pos: usize, text: String },
    Group(Vec<UndoItem>),
}

/// The main text editor
pub struct Editor {
    rope: Rope,
    cursor_line: usize,
    cursor_col: usize,
    modified: bool,
    clipboard: Option<String>,
    undo_stack: Vec<UndoItem>,
    redo_stack: Vec<UndoItem>,
    sticky_col: Option<usize>,
    last_edit_time: Option<Instant>,
    backup_created: bool,
    original_content: Option<String>,
}

impl Editor {
    /// Create a new empty editor
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            cursor_line: 0,
            cursor_col: 0,
            modified: false,
            clipboard: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            sticky_col: None,
            last_edit_time: None,
            backup_created: false,
            original_content: None,
        }
    }

    /// Load file contents into the editor
    pub fn load(&mut self, path: &Path) -> io::Result<()> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            // Normalize line endings to LF
            let normalized = content.replace("\r\n", "\n").replace("\r", "\n");
            self.rope = Rope::from_str(&normalized);
            // Store original content for backup on first edit
            self.original_content = Some(normalized);
        } else {
            // New file - start empty
            self.rope = Rope::new();
            self.original_content = None;
        }
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.modified = false;
        self.backup_created = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }

    /// Create backup file on first edit (per spec 5.4)
    pub fn create_backup_if_needed(&mut self, path: &Path) -> io::Result<()> {
        if self.backup_created {
            return Ok(());
        }
        if let Some(ref original) = self.original_content {
            let backup_path = path.with_extension("hollow-backup");
            fs::write(&backup_path, original)?;
        }
        self.backup_created = true;
        Ok(())
    }

    /// Check if this is the first edit (backup needed)
    pub fn needs_backup(&self) -> bool {
        !self.backup_created && self.original_content.is_some()
    }

    /// Save editor contents to file
    pub fn save(&mut self, path: &Path) -> io::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Atomic write: write to temp file, then rename
        let temp_path = path.with_extension("hollow-tmp");
        {
            let mut file = fs::File::create(&temp_path)?;
            for chunk in self.rope.chunks() {
                file.write_all(chunk.as_bytes())?;
            }
            // Ensure trailing newline
            if !self.rope.len_chars() == 0 {
                let last_char = self.rope.char(self.rope.len_chars().saturating_sub(1));
                if last_char != '\n' {
                    file.write_all(b"\n")?;
                }
            }
            file.sync_all()?;
        }
        fs::rename(&temp_path, path)?;

        self.modified = false;
        self.mark_undo_boundary(); // Force new undo group after save per spec 4.2
        Ok(())
    }

    /// Set the editor content (used for restoring versions)
    pub fn set_content(&mut self, content: &str) {
        let normalized = content.replace("\r\n", "\n").replace("\r", "\n");
        self.rope = Rope::from_str(&normalized);
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.modified = true;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        let byte_pos = self.cursor_byte_offset();
        let char_pos = self.rope.byte_to_char(byte_pos);

        // Record undo
        self.push_undo(UndoItem::Delete {
            pos: char_pos,
            text: c.to_string(),
        });

        self.rope.insert_char(char_pos, c);
        self.cursor_col += c.len_utf8();
        self.modified = true;
        self.sticky_col = None;
    }

    /// Insert a newline at the cursor position
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    /// Delete character before cursor (backspace)
    pub fn delete_char(&mut self) {
        if self.cursor_col == 0 && self.cursor_line == 0 {
            // At start of document, nothing to delete
            return;
        }

        if self.cursor_col == 0 {
            // At start of line, join with previous line
            self.cursor_line -= 1;
            self.cursor_col = self.line_len(self.cursor_line);
        } else {
            // Move cursor back
            let byte_pos = self.cursor_byte_offset();
            let char_pos = self.rope.byte_to_char(byte_pos);
            if char_pos > 0 {
                let prev_char = self.rope.char(char_pos - 1);
                self.cursor_col -= prev_char.len_utf8();
            }
        }

        // Delete the character
        let byte_pos = self.cursor_byte_offset();
        let char_pos = self.rope.byte_to_char(byte_pos);

        if char_pos < self.rope.len_chars() {
            let deleted_char = self.rope.char(char_pos);

            // Record undo
            self.push_undo(UndoItem::Insert {
                pos: char_pos,
                text: deleted_char.to_string(),
            });

            self.rope.remove(char_pos..char_pos + 1);
            self.modified = true;
        }
        self.sticky_col = None;
    }

    /// Delete character at cursor (delete key)
    pub fn delete_char_forward(&mut self) {
        let byte_pos = self.cursor_byte_offset();
        let char_pos = self.rope.byte_to_char(byte_pos);

        if char_pos < self.rope.len_chars() {
            let deleted_char = self.rope.char(char_pos);

            // Record undo
            self.push_undo(UndoItem::Insert {
                pos: char_pos,
                text: deleted_char.to_string(),
            });

            self.rope.remove(char_pos..char_pos + 1);
            self.modified = true;
        }
    }

    /// Delete the current line
    pub fn delete_line(&mut self) {
        if self.rope.len_lines() == 0 {
            return;
        }

        let line_start = self.rope.line_to_char(self.cursor_line);
        let line_end = if self.cursor_line + 1 < self.rope.len_lines() {
            self.rope.line_to_char(self.cursor_line + 1)
        } else {
            self.rope.len_chars()
        };

        if line_start < line_end {
            let deleted_text: String = self.rope.slice(line_start..line_end).chars().collect();

            // Record undo
            self.push_undo(UndoItem::Insert {
                pos: line_start,
                text: deleted_text,
            });

            self.rope.remove(line_start..line_end);
            self.modified = true;

            // Adjust cursor
            if self.cursor_line >= self.rope.len_lines() && self.cursor_line > 0 {
                self.cursor_line = self.rope.len_lines().saturating_sub(1);
            }
            self.cursor_col = 0;
        }
    }

    /// Copy the current line to clipboard
    pub fn copy_line(&mut self) {
        if self.cursor_line < self.rope.len_lines() {
            let line = self.rope.line(self.cursor_line);
            self.clipboard = Some(line.to_string());
        }
    }

    /// Paste clipboard contents at cursor
    pub fn paste(&mut self) {
        if let Some(ref text) = self.clipboard.clone() {
            let byte_pos = self.cursor_byte_offset();
            let char_pos = self.rope.byte_to_char(byte_pos);

            // Record undo
            self.push_undo(UndoItem::Delete {
                pos: char_pos,
                text: text.clone(),
            });

            self.rope.insert(char_pos, text);
            self.modified = true;

            // Move cursor to end of pasted text
            let text_lines: Vec<&str> = text.split('\n').collect();
            if text_lines.len() > 1 {
                self.cursor_line += text_lines.len() - 1;
                self.cursor_col = text_lines.last().map(|s| s.len()).unwrap_or(0);
            } else {
                self.cursor_col += text.len();
            }
        }
    }

    /// Undo the last operation
    pub fn undo(&mut self) {
        if let Some(item) = self.undo_stack.pop() {
            let redo_item = self.apply_undo_item(&item);
            self.redo_stack.push(redo_item);
            self.modified = true;
        }
    }

    /// Redo the last undone operation
    pub fn redo(&mut self) {
        if let Some(item) = self.redo_stack.pop() {
            let undo_item = self.apply_undo_item(&item);
            self.undo_stack.push(undo_item);
            self.modified = true;
        }
    }

    /// Apply an undo item and return its inverse
    fn apply_undo_item(&mut self, item: &UndoItem) -> UndoItem {
        match item {
            UndoItem::Insert { pos, text } => {
                self.rope.insert(*pos, text);
                UndoItem::Delete {
                    pos: *pos,
                    text: text.clone(),
                }
            }
            UndoItem::Delete { pos, text } => {
                self.rope.remove(*pos..*pos + text.chars().count());
                UndoItem::Insert {
                    pos: *pos,
                    text: text.clone(),
                }
            }
            UndoItem::Group(items) => {
                let mut inverse_items = Vec::new();
                for item in items.iter().rev() {
                    inverse_items.push(self.apply_undo_item(item));
                }
                inverse_items.reverse();
                UndoItem::Group(inverse_items)
            }
        }
    }

    /// Push an undo item, clearing the redo stack
    /// Groups rapid edits (within 2 seconds) into a single undo unit per spec 4.2
    fn push_undo(&mut self, item: UndoItem) {
        let now = Instant::now();
        let should_group = self.last_edit_time
            .map(|t| now.duration_since(t).as_secs() < 2)
            .unwrap_or(false);

        if should_group && !self.undo_stack.is_empty() {
            // Group with previous item
            let prev = self.undo_stack.pop().unwrap();
            let grouped = match prev {
                UndoItem::Group(mut items) => {
                    items.push(item);
                    UndoItem::Group(items)
                }
                other => UndoItem::Group(vec![other, item]),
            };
            self.undo_stack.push(grouped);
        } else {
            self.undo_stack.push(item);
        }

        self.last_edit_time = Some(now);
        self.redo_stack.clear();
    }

    /// Force a new undo group (called on save)
    pub fn mark_undo_boundary(&mut self) {
        self.last_edit_time = None;
    }

    /// Move cursor in the given direction by the given unit
    pub fn move_cursor(&mut self, direction: Direction, unit: Unit) {
        match (direction, unit) {
            (Direction::Left, Unit::Char) => self.move_left(),
            (Direction::Right, Unit::Char) => self.move_right(),
            (Direction::Up, Unit::Char) | (Direction::Up, Unit::Line) => self.move_up(),
            (Direction::Down, Unit::Char) | (Direction::Down, Unit::Line) => self.move_down(),
            (Direction::Left, Unit::Word) => self.move_word_backward(),
            (Direction::Right, Unit::Word) => self.move_word_forward(),
            (Direction::Left, Unit::Line) => self.move_line_start(),
            (Direction::Right, Unit::Line) => self.move_line_end(),
            (Direction::Up, Unit::Paragraph) => self.move_paragraph_up(),
            (Direction::Down, Unit::Paragraph) => self.move_paragraph_down(),
            (Direction::Up, Unit::Document) => self.move_document_start(),
            (Direction::Down, Unit::Document) => self.move_document_end(),
            (Direction::Up, Unit::Page(height)) => self.move_page_up(height),
            (Direction::Down, Unit::Page(height)) => self.move_page_down(height),
            _ => {}
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            // Move back one character
            let line_start = self.rope.line_to_byte(self.cursor_line);
            let current_byte = line_start + self.cursor_col;
            let char_pos = self.rope.byte_to_char(current_byte);
            if char_pos > 0 {
                let prev_char = self.rope.char(char_pos - 1);
                if prev_char == '\n' {
                    // Don't cross line boundary here
                    self.cursor_col = 0;
                } else {
                    self.cursor_col -= prev_char.len_utf8();
                }
            }
        } else if self.cursor_line > 0 {
            // Wrap to end of previous line
            self.cursor_line -= 1;
            self.cursor_col = self.line_len(self.cursor_line);
        }
        self.sticky_col = None;
    }

    fn move_right(&mut self) {
        let line_len = self.line_len(self.cursor_line);
        if self.cursor_col < line_len {
            // Move forward one character
            let line_start = self.rope.line_to_byte(self.cursor_line);
            let current_byte = line_start + self.cursor_col;
            let char_pos = self.rope.byte_to_char(current_byte);
            if char_pos < self.rope.len_chars() {
                let current_char = self.rope.char(char_pos);
                self.cursor_col += current_char.len_utf8();
            }
        } else if self.cursor_line + 1 < self.rope.len_lines() {
            // Wrap to start of next line
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
        self.sticky_col = None;
    }

    fn move_up(&mut self) {
        if self.cursor_line > 0 {
            let target_col = self.sticky_col.unwrap_or(self.cursor_col);
            self.cursor_line -= 1;
            let new_line_len = self.line_len(self.cursor_line);
            self.cursor_col = target_col.min(new_line_len);
            self.sticky_col = Some(target_col);
        }
    }

    fn move_down(&mut self) {
        if self.cursor_line + 1 < self.rope.len_lines() {
            let target_col = self.sticky_col.unwrap_or(self.cursor_col);
            self.cursor_line += 1;
            let new_line_len = self.line_len(self.cursor_line);
            self.cursor_col = target_col.min(new_line_len);
            self.sticky_col = Some(target_col);
        }
    }

    fn move_word_forward(&mut self) {
        let line_start = self.rope.line_to_char(self.cursor_line);
        let mut char_pos = line_start + self.cursor_col_chars();

        // Skip current word
        while char_pos < self.rope.len_chars() {
            let c = self.rope.char(char_pos);
            if c == '\n' || c.is_whitespace() {
                break;
            }
            char_pos += 1;
        }

        // Skip whitespace
        while char_pos < self.rope.len_chars() {
            let c = self.rope.char(char_pos);
            if c == '\n' {
                char_pos += 1;
                break;
            }
            if !c.is_whitespace() {
                break;
            }
            char_pos += 1;
        }

        // Update cursor position
        self.set_cursor_from_char_pos(char_pos);
        self.sticky_col = None;
    }

    fn move_word_backward(&mut self) {
        let line_start = self.rope.line_to_char(self.cursor_line);
        let mut char_pos = line_start + self.cursor_col_chars();

        if char_pos == 0 {
            return;
        }
        char_pos -= 1;

        // Skip whitespace
        while char_pos > 0 {
            let c = self.rope.char(char_pos);
            if !c.is_whitespace() {
                break;
            }
            char_pos -= 1;
        }

        // Skip to start of word
        while char_pos > 0 {
            let prev_c = self.rope.char(char_pos - 1);
            if prev_c.is_whitespace() || prev_c == '\n' {
                break;
            }
            char_pos -= 1;
        }

        self.set_cursor_from_char_pos(char_pos);
        self.sticky_col = None;
    }

    fn move_line_start(&mut self) {
        self.cursor_col = 0;
        self.sticky_col = None;
    }

    fn move_line_end(&mut self) {
        self.cursor_col = self.line_len(self.cursor_line);
        self.sticky_col = None;
    }

    /// Move to previous paragraph (blank line or start of document)
    fn move_paragraph_up(&mut self) {
        // Skip current line if not blank
        if self.cursor_line > 0 && !self.is_blank_line(self.cursor_line) {
            self.cursor_line -= 1;
        }

        // Skip blank lines
        while self.cursor_line > 0 && self.is_blank_line(self.cursor_line) {
            self.cursor_line -= 1;
        }

        // Find start of paragraph (first blank line or start)
        while self.cursor_line > 0 && !self.is_blank_line(self.cursor_line - 1) {
            self.cursor_line -= 1;
        }

        self.cursor_col = 0;
        self.sticky_col = None;
    }

    /// Move to next paragraph (blank line or end of document)
    fn move_paragraph_down(&mut self) {
        let max_line = self.rope.len_lines().saturating_sub(1);

        // Skip current paragraph content
        while self.cursor_line < max_line && !self.is_blank_line(self.cursor_line) {
            self.cursor_line += 1;
        }

        // Skip blank lines
        while self.cursor_line < max_line && self.is_blank_line(self.cursor_line) {
            self.cursor_line += 1;
        }

        self.cursor_col = 0;
        self.sticky_col = None;
    }

    /// Check if a line is blank (empty or whitespace only)
    fn is_blank_line(&self, line: usize) -> bool {
        if line >= self.rope.len_lines() {
            return true;
        }
        let line_text = self.rope.line(line);
        line_text.chars().all(|c| c.is_whitespace())
    }

    fn move_document_start(&mut self) {
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.sticky_col = None;
    }

    fn move_document_end(&mut self) {
        self.cursor_line = self.rope.len_lines().saturating_sub(1);
        self.cursor_col = self.line_len(self.cursor_line);
        self.sticky_col = None;
    }

    fn move_page_up(&mut self, height: usize) {
        let target_col = self.sticky_col.unwrap_or(self.cursor_col);
        self.cursor_line = self.cursor_line.saturating_sub(height);
        let new_line_len = self.line_len(self.cursor_line);
        self.cursor_col = target_col.min(new_line_len);
        self.sticky_col = Some(target_col);
    }

    fn move_page_down(&mut self, height: usize) {
        let target_col = self.sticky_col.unwrap_or(self.cursor_col);
        self.cursor_line = (self.cursor_line + height).min(self.rope.len_lines().saturating_sub(1));
        let new_line_len = self.line_len(self.cursor_line);
        self.cursor_col = target_col.min(new_line_len);
        self.sticky_col = Some(target_col);
    }

    /// Get cursor position in chars within the current line
    fn cursor_col_chars(&self) -> usize {
        if self.cursor_line >= self.rope.len_lines() {
            return 0;
        }
        let line_start_byte = self.rope.line_to_byte(self.cursor_line);
        let cursor_byte = line_start_byte + self.cursor_col;
        let line_start_char = self.rope.byte_to_char(line_start_byte);
        let cursor_char = self
            .rope
            .byte_to_char(cursor_byte.min(self.rope.len_bytes()));
        cursor_char - line_start_char
    }

    /// Set cursor position from a char position
    fn set_cursor_from_char_pos(&mut self, char_pos: usize) {
        let clamped = char_pos.min(self.rope.len_chars());
        self.cursor_line = self.rope.char_to_line(clamped);
        let line_start_char = self.rope.line_to_char(self.cursor_line);
        let line_start_byte = self.rope.char_to_byte(line_start_char);
        let cursor_byte = self.rope.char_to_byte(clamped);
        self.cursor_col = cursor_byte - line_start_byte;
    }

    /// Get byte offset of cursor position
    pub fn cursor_byte_offset(&self) -> usize {
        if self.cursor_line >= self.rope.len_lines() {
            return self.rope.len_bytes();
        }
        let line_start = self.rope.line_to_byte(self.cursor_line);
        let line_len = self.line_len(self.cursor_line);
        line_start + self.cursor_col.min(line_len)
    }

    /// Get length of a line in bytes (excluding newline)
    fn line_len(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_slice = self.rope.line(line);
        let len = line_slice.len_bytes();
        // Exclude trailing newline
        if len > 0 && line_slice.char(line_slice.len_chars().saturating_sub(1)) == '\n' {
            len - 1
        } else {
            len
        }
    }

    /// Get cursor position as (line, column)
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    /// Get reference to the rope content
    pub fn content(&self) -> &Rope {
        &self.rope
    }

    /// Check if the document has been modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Count words in the document
    pub fn word_count(&self) -> usize {
        self.rope
            .chars()
            .collect::<String>()
            .split_whitespace()
            .count()
    }

    /// Get number of lines
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get a specific line as string
    pub fn line(&self, idx: usize) -> Option<String> {
        if idx < self.rope.len_lines() {
            Some(self.rope.line(idx).to_string())
        } else {
            None
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor_is_empty() {
        let editor = Editor::new();
        assert_eq!(editor.word_count(), 0);
        assert!(!editor.is_modified());
        assert_eq!(editor.cursor_position(), (0, 0));
    }

    #[test]
    fn test_insert_char() {
        let mut editor = Editor::new();
        editor.insert_char('H');
        editor.insert_char('i');
        assert_eq!(editor.content().to_string(), "Hi");
        assert!(editor.is_modified());
        assert_eq!(editor.cursor_position(), (0, 2));
    }

    #[test]
    fn test_insert_newline() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_newline();
        editor.insert_char('b');
        assert_eq!(editor.content().to_string(), "a\nb");
        assert_eq!(editor.cursor_position(), (1, 1));
        assert_eq!(editor.line_count(), 2);
    }

    #[test]
    fn test_delete_char_backspace() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        editor.delete_char();
        assert_eq!(editor.content().to_string(), "ab");
        assert_eq!(editor.cursor_position(), (0, 2));
    }

    #[test]
    fn test_delete_char_at_start_does_nothing() {
        let mut editor = Editor::new();
        editor.delete_char();
        assert_eq!(editor.content().to_string(), "");
        assert_eq!(editor.cursor_position(), (0, 0));
    }

    #[test]
    fn test_delete_char_joins_lines() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_newline();
        editor.insert_char('b');
        // Cursor is at (1, 1), move to start of line 1
        editor.move_cursor(Direction::Left, Unit::Line);
        // Now delete to join lines
        editor.delete_char();
        assert_eq!(editor.content().to_string(), "ab");
    }

    #[test]
    fn test_delete_char_forward() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        editor.move_cursor(Direction::Left, Unit::Line);
        editor.delete_char_forward();
        assert_eq!(editor.content().to_string(), "bc");
    }

    #[test]
    fn test_word_count() {
        let mut editor = Editor::new();
        for c in "Hello world test".chars() {
            editor.insert_char(c);
        }
        assert_eq!(editor.word_count(), 3);
    }

    #[test]
    fn test_move_left_right() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        assert_eq!(editor.cursor_position(), (0, 3));

        editor.move_cursor(Direction::Left, Unit::Char);
        assert_eq!(editor.cursor_position(), (0, 2));

        editor.move_cursor(Direction::Right, Unit::Char);
        assert_eq!(editor.cursor_position(), (0, 3));
    }

    #[test]
    fn test_move_up_down() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_newline();
        editor.insert_char('c');
        editor.insert_char('d');
        editor.insert_char('e');
        // Cursor at (1, 3)

        editor.move_cursor(Direction::Up, Unit::Line);
        assert_eq!(editor.cursor_position(), (0, 2)); // Clamped to line length

        editor.move_cursor(Direction::Down, Unit::Line);
        assert_eq!(editor.cursor_position(), (1, 3)); // Sticky col returns to original
    }

    #[test]
    fn test_move_line_start_end() {
        let mut editor = Editor::new();
        for c in "Hello".chars() {
            editor.insert_char(c);
        }

        editor.move_cursor(Direction::Left, Unit::Line);
        assert_eq!(editor.cursor_position(), (0, 0));

        editor.move_cursor(Direction::Right, Unit::Line);
        assert_eq!(editor.cursor_position(), (0, 5));
    }

    #[test]
    fn test_move_document_start_end() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_newline();
        editor.insert_char('b');
        editor.insert_newline();
        editor.insert_char('c');

        editor.move_cursor(Direction::Up, Unit::Document);
        assert_eq!(editor.cursor_position(), (0, 0));

        editor.move_cursor(Direction::Down, Unit::Document);
        assert_eq!(editor.cursor_position(), (2, 1));
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        // Force undo boundary so each char is separate undo unit
        editor.mark_undo_boundary();
        editor.insert_char('b');
        assert_eq!(editor.content().to_string(), "ab");

        editor.undo();
        assert_eq!(editor.content().to_string(), "a");

        editor.undo();
        assert_eq!(editor.content().to_string(), "");

        editor.redo();
        assert_eq!(editor.content().to_string(), "a");
    }

    #[test]
    fn test_delete_line() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_newline();
        editor.insert_char('b');
        editor.insert_newline();
        editor.insert_char('c');

        editor.move_cursor(Direction::Up, Unit::Line);
        editor.delete_line();

        assert_eq!(editor.line_count(), 2);
    }

    #[test]
    fn test_copy_paste() {
        let mut editor = Editor::new();
        for c in "Hello\n".chars() {
            editor.insert_char(c);
        }
        editor.move_cursor(Direction::Up, Unit::Line);
        editor.copy_line();

        editor.move_cursor(Direction::Down, Unit::Document);
        editor.paste();

        assert!(editor.content().to_string().contains("Hello"));
    }

    #[test]
    fn test_paragraph_movement() {
        let mut editor = Editor::new();
        // Create content with paragraphs separated by blank lines
        for c in "Line one.\n\nLine two.\n\nLine three.".chars() {
            editor.insert_char(c);
        }
        // Content: "Line one.\n\nLine two.\n\nLine three."
        // Line 0: "Line one."
        // Line 1: "" (blank)
        // Line 2: "Line two."
        // Line 3: "" (blank)
        // Line 4: "Line three."

        // Move to start
        editor.move_cursor(Direction::Up, Unit::Document);
        assert_eq!(editor.cursor_position().0, 0);

        // Move down by paragraph - should skip line 0, skip blank line 1, land on line 2
        editor.move_cursor(Direction::Down, Unit::Paragraph);
        // Should land on first non-blank line after blank
        assert!(editor.cursor_position().0 >= 2);

        // Move up by paragraph - should go back
        let before = editor.cursor_position().0;
        editor.move_cursor(Direction::Up, Unit::Paragraph);
        assert!(editor.cursor_position().0 < before);
    }
}
