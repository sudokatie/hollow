use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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

    // Render main text content
    render_content(frame, text_area, state);

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
    let (cursor_x, cursor_y) = calculate_cursor_position(state, text_area);
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_content(frame: &mut Frame, area: Rect, state: &RenderState) {
    let lines: Vec<&str> = state.content.lines().collect();
    let visible_lines = area.height as usize;

    // Calculate scroll offset to keep cursor visible
    let scroll = if state.cursor_line >= visible_lines {
        state.cursor_line - visible_lines + 1
    } else {
        0
    };

    // Build styled lines
    let display_lines: Vec<Line> = lines
        .iter()
        .skip(scroll)
        .take(visible_lines)
        .enumerate()
        .map(|(idx, line)| {
            let actual_line = scroll + idx;
            let style = if actual_line == state.cursor_line {
                Style::default()
            } else {
                Style::default().fg(Color::White)
            };
            Line::styled(*line, style)
        })
        .collect();

    let paragraph = Paragraph::new(display_lines).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_status(frame: &mut Frame, area: Rect, state: &RenderState) {
    let mode_str = match state.mode {
        Mode::Write => "WRITE",
        Mode::Navigate => "NAV",
        Mode::Search => "SEARCH",
    };

    let modified_str = if state.modified { " [+]" } else { "" };

    let status = format!(
        "Words: {}  |  {}  |  {}{}",
        state.word_count, state.elapsed, mode_str, modified_str
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
    i               Return to writing

  GENERAL
    Ctrl+S          Save
    Ctrl+Q          Quit
    Ctrl+G          Toggle status
    ?               Show this help

  Press any key to close
"#;

    let width = 50.min(area.width - 4);
    let height = 32.min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let overlay_area = Rect {
        x,
        y,
        width,
        height,
    };

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

    let overlay_area = Rect {
        x,
        y,
        width,
        height,
    };

    frame.render_widget(Clear, overlay_area);

    let confirm = Paragraph::new("\n  Save changes before quitting?\n\n  (y)es  (n)o  (c)ancel")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Unsaved Changes "),
        )
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

fn calculate_cursor_position(state: &RenderState, area: Rect) -> (u16, u16) {
    let lines: Vec<&str> = state.content.lines().collect();
    let visible_lines = area.height as usize;

    let scroll = if state.cursor_line >= visible_lines {
        state.cursor_line - visible_lines + 1
    } else {
        0
    };

    let visual_line = state.cursor_line.saturating_sub(scroll);
    let visual_col = state
        .cursor_col
        .min(lines.get(state.cursor_line).map(|l| l.len()).unwrap_or(0));

    (area.x + visual_col as u16, area.y + visual_line as u16)
}
