use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::input::Mode;

/// Render state passed to UI
pub struct RenderState<'a> {
    pub content: &'a str,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub mode: Mode,
    pub word_count: usize,
    pub elapsed: &'a str,
    pub modified: bool,
    pub show_status: bool,
    pub show_help: bool,
    pub show_quit_confirm: bool,
    pub search_active: bool,
    pub search_query: &'a str,
    pub search_matches: &'a [(usize, usize)],
    pub text_width: usize,
    pub show_saved_indicator: bool,
}

const WRAP_INDENT: &str = "  "; // 2 spaces for wrapped line continuation per spec 4.3

/// Wrap a single line at word boundaries with indent for continuation
fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }

    let effective_width = width.saturating_sub(WRAP_INDENT.len());
    if effective_width < 10 {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();
    let mut is_first = true;

    for word in line.split_inclusive(' ') {
        let prefix = if is_first { "" } else { WRAP_INDENT };
        let max_width = if is_first { width } else { effective_width };

        if current_line.is_empty() {
            current_line = format!("{}{}", prefix, word);
        } else if current_line.len() + word.len() <= max_width {
            current_line.push_str(word);
        } else {
            // Line is full, start a new one
            result.push(current_line);
            is_first = false;
            current_line = format!("{}{}", WRAP_INDENT, word);
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

/// Build visual lines from content with word wrapping
/// Returns (visual_lines, line_map) where line_map[visual_idx] = (logical_line, is_continuation)
fn build_visual_lines(content: &str, width: usize) -> (Vec<String>, Vec<(usize, bool)>) {
    let mut visual_lines = Vec::new();
    let mut line_map = Vec::new();

    for (logical_idx, line) in content.lines().enumerate() {
        let wrapped = wrap_line(line, width);
        for (i, wrapped_line) in wrapped.into_iter().enumerate() {
            visual_lines.push(wrapped_line);
            line_map.push((logical_idx, i > 0));
        }
    }

    // Handle empty content
    if visual_lines.is_empty() {
        visual_lines.push(String::new());
        line_map.push((0, false));
    }

    (visual_lines, line_map)
}

/// Find visual line and column for a logical cursor position
fn logical_to_visual(
    content: &str,
    logical_line: usize,
    logical_col: usize,
    width: usize,
) -> (usize, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut visual_line = 0;

    // Count visual lines before cursor's logical line
    for (idx, line) in lines.iter().enumerate() {
        if idx == logical_line {
            break;
        }
        visual_line += wrap_line(line, width).len();
    }

    // Now find position within the wrapped lines of the cursor's logical line
    if logical_line < lines.len() {
        let cursor_line_text = lines[logical_line];
        let wrapped = wrap_line(cursor_line_text, width);

        let mut remaining_col = logical_col;
        for (i, wrapped_line) in wrapped.iter().enumerate() {
            let line_len = if i == 0 {
                wrapped_line.len()
            } else {
                wrapped_line.len().saturating_sub(WRAP_INDENT.len())
            };

            if remaining_col <= line_len || i == wrapped.len() - 1 {
                let visual_col = if i == 0 {
                    remaining_col
                } else {
                    remaining_col + WRAP_INDENT.len()
                };
                return (visual_line + i, visual_col);
            }
            remaining_col -= line_len;
        }
    }

    (visual_line, logical_col)
}

/// Main render function
pub fn render(frame: &mut Frame, state: &RenderState) {
    let area = frame.area();

    // Calculate margins for centering text
    let text_width = state.text_width.min(area.width as usize - 4);
    let margin = (area.width as usize).saturating_sub(text_width) / 2;

    // Create text area (full screen minus status line if visible)
    let main_height = if state.show_status {
        area.height.saturating_sub(1)
    } else {
        area.height
    };

    let text_area = Rect {
        x: margin as u16,
        y: 0,
        width: text_width as u16,
        height: main_height,
    };

    // Render main text content with word wrapping
    let (cursor_x, cursor_y) = render_content(frame, text_area, state);

    // Render status line if visible
    if state.show_status {
        let status_area = Rect {
            x: 0,
            y: area.height - 1,
            width: area.width,
            height: 1,
        };
        render_status(frame, status_area, state);
    }

    // Render overlays
    if state.show_help {
        render_help_overlay(frame, area);
    } else if state.show_quit_confirm {
        render_quit_confirm(frame, area);
    } else if state.search_active {
        render_search_prompt(frame, area, state.search_query);
    }

    // Position cursor
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_content(frame: &mut Frame, area: Rect, state: &RenderState) -> (u16, u16) {
    let width = area.width as usize;
    let visible_lines = area.height as usize;

    // Build visual lines with word wrapping
    let (visual_lines, line_map) = build_visual_lines(state.content, width);

    // Find cursor visual position
    let (cursor_visual_line, cursor_visual_col) = logical_to_visual(
        state.content,
        state.cursor_line,
        state.cursor_col,
        width,
    );

    // Calculate scroll to keep cursor visible
    let scroll = if cursor_visual_line >= visible_lines {
        cursor_visual_line - visible_lines + 1
    } else {
        0
    };

    // Build styled lines with search highlighting
    let display_lines: Vec<Line> = visual_lines
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_lines)
        .map(|(idx, line)| {
            let (logical_line, is_continuation) = line_map.get(idx).copied().unwrap_or((0, false));

            // Highlight search matches
            let styled_line = if !state.search_matches.is_empty() && !state.search_query.is_empty() {
                highlight_matches(line, state.search_query, logical_line, is_continuation, state.content)
            } else {
                Line::from(line.as_str())
            };

            styled_line
        })
        .collect();

    let paragraph = Paragraph::new(display_lines);
    frame.render_widget(paragraph, area);

    // Calculate cursor screen position
    let cursor_screen_y = (cursor_visual_line - scroll) as u16;
    let cursor_screen_x = area.x + cursor_visual_col.min(width) as u16;

    (cursor_screen_x, area.y + cursor_screen_y)
}

/// Highlight search matches in a line
fn highlight_matches(
    line: &str,
    query: &str,
    _logical_line: usize,
    _is_continuation: bool,
    _content: &str,
) -> Line<'static> {
    if query.is_empty() {
        return Line::from(line.to_string());
    }

    let query_lower = query.to_lowercase();
    let line_lower = line.to_lowercase();

    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in line_lower.match_indices(&query_lower) {
        // Add text before match
        if start > last_end {
            spans.push(Span::raw(line[last_end..start].to_string()));
        }
        // Add highlighted match
        spans.push(Span::styled(
            line[start..start + query.len()].to_string(),
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ));
        last_end = start + query.len();
    }

    // Add remaining text
    if last_end < line.len() {
        spans.push(Span::raw(line[last_end..].to_string()));
    }

    if spans.is_empty() {
        Line::from(line.to_string())
    } else {
        Line::from(spans)
    }
}

fn render_status(frame: &mut Frame, area: Rect, state: &RenderState) {
    let mode_str = match state.mode {
        Mode::Write => "WRITE",
        Mode::Navigate => "NAV",
        Mode::Search => "SEARCH",
    };

    let modified_str = if state.modified { " [+]" } else { "" };
    let saved_str = if state.show_saved_indicator { "  Saved" } else { "" };

    let status = format!(
        "Words: {}  |  {}  |  {}{}{}",
        state.word_count, state.elapsed, mode_str, modified_str, saved_str
    );

    let status_line = Paragraph::new(status)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    frame.render_widget(status_line, area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let help_text = r#"
  HOLLOW - Key Bindings

  NAVIGATION
    Arrow keys      Move cursor
    Ctrl+Left/Right Move by word
    Home/End        Line start/end
    Ctrl+Home/End   Document start/end
    Page Up/Down    Move by page

  NAVIGATE MODE (Escape to enter)
    h/j/k/l         Move left/down/up/right
    w/b             Move by word
    0/$             Line start/end
    gg/G            Document start/end
    /               Search
    n/N             Next/prev match

  EDITING (Navigate mode)
    dd              Delete line
    yy              Copy line
    p               Paste
    u               Undo
    Ctrl+r          Redo
    i or any char   Return to writing

  GENERAL
    Ctrl+S          Save
    Ctrl+Q          Quit
    Ctrl+G          Toggle status
    ?               Show this help

  Press any key to close
"#;

    let width = 50.min(area.width - 4);
    let height = 34.min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let overlay_area = Rect { x, y, width, height };

    frame.render_widget(Clear, overlay_area);

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .style(Style::default().fg(Color::White));

    frame.render_widget(help, overlay_area);
}

fn render_quit_confirm(frame: &mut Frame, area: Rect) {
    let width = 40.min(area.width - 4);
    let height = 5;
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let overlay_area = Rect { x, y, width, height };

    frame.render_widget(Clear, overlay_area);

    let confirm = Paragraph::new("\n  Save changes before quitting?\n\n  (y)es  (n)o  (c)ancel")
        .block(Block::default().borders(Borders::ALL).title(" Unsaved Changes "))
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(confirm, overlay_area);
}

fn render_search_prompt(frame: &mut Frame, area: Rect, query: &str) {
    let search_area = Rect {
        x: 0,
        y: area.height - 1,
        width: area.width,
        height: 1,
    };

    let prompt = format!("/{}", query);
    let search_line = Paragraph::new(prompt).style(Style::default().fg(Color::Cyan));

    frame.render_widget(search_line, search_area);
}
