use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::input::Mode;
use crate::spell::Misspelling;
use crate::stats::WritingStats;
use crate::theme::Theme;
use crate::versions::Version;

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
    pub show_stats: bool,
    pub search_active: bool,
    pub search_query: &'a str,
    pub search_matches: &'a [(usize, usize)],
    pub text_width: usize,
    pub show_saved_indicator: bool,
    // Goal tracking
    pub daily_goal: usize,
    pub goal_progress: f64,
    pub streak: usize,
    pub goal_met: bool,
    pub show_goal: bool,
    // Statistics
    pub writing_stats: Option<&'a WritingStats>,
    // Version history
    pub show_versions: bool,
    pub versions: &'a [Version],
    pub version_index: usize,
    pub version_view: Option<&'a str>,    // Content of version being viewed
    pub version_diff: Option<&'a str>,    // Diff output
    pub version_time: Option<&'a str>,    // Time of version being viewed
    // Project documents
    pub show_project_docs: bool,
    pub project_name: Option<&'a str>,
    pub project_docs: &'a [String],
    pub project_doc_index: usize,
    pub current_doc: &'a str,
    // Theme
    pub theme: &'a Theme,
    // Spell checking
    pub spell_enabled: bool,
    pub misspellings: &'a [Misspelling],
    // Spell suggestions popup
    pub show_spell_suggestions: bool,
    pub spell_suggestion_word: &'a str,
    pub spell_suggestions: &'a [String],
    pub spell_suggestion_index: usize,
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
    } else if state.show_stats {
        render_stats_overlay(frame, area, state.writing_stats);
    } else if state.show_versions {
        render_versions_overlay(frame, area, state.versions, state.version_index);
    } else if let Some(content) = state.version_view {
        render_version_view(frame, area, content, state.version_time.unwrap_or(""));
    } else if let Some(diff) = state.version_diff {
        render_version_diff(frame, area, diff, state.version_time.unwrap_or(""));
    } else if state.show_project_docs {
        render_project_docs_overlay(
            frame, area,
            state.project_name.unwrap_or("Project"),
            state.project_docs,
            state.project_doc_index,
            state.current_doc,
        );
    } else if state.show_spell_suggestions {
        render_spell_suggestions_overlay(
            frame, area,
            state.spell_suggestion_word,
            state.spell_suggestions,
            state.spell_suggestion_index,
        );
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

    // Build styled lines with search and spell highlighting
    let display_lines: Vec<Line> = visual_lines
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_lines)
        .map(|(idx, line)| {
            let (logical_line, is_continuation) = line_map.get(idx).copied().unwrap_or((0, false));

            // Start with spell highlighting if enabled
            let base_line = if state.spell_enabled && !state.misspellings.is_empty() {
                highlight_misspellings(line, logical_line, is_continuation, state.misspellings)
            } else {
                Line::from(line.as_str())
            };

            // Then apply search highlighting on top
            let styled_line = if !state.search_matches.is_empty() && !state.search_query.is_empty() {
                highlight_matches_on_line(&base_line, state.search_query)
            } else {
                base_line
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

/// Highlight misspelled words in a line with red underline
fn highlight_misspellings(
    line: &str,
    logical_line: usize,
    is_continuation: bool,
    misspellings: &[Misspelling],
) -> Line<'static> {
    // Get misspellings for this logical line
    let line_misspellings: Vec<&Misspelling> = misspellings
        .iter()
        .filter(|m| m.line == logical_line)
        .collect();

    if line_misspellings.is_empty() {
        return Line::from(line.to_string());
    }

    // Account for continuation indent (used in position calculations)
    let _col_offset = if is_continuation { WRAP_INDENT.len() } else { 0 };

    let mut spans = Vec::new();
    let mut last_end = 0;
    let chars: Vec<char> = line.chars().collect();

    for m in line_misspellings {
        // Adjust column for visual line (accounting for wrap indent)
        let visual_col = if is_continuation {
            // For continuation lines, we need to figure out where this word appears
            // This is complex with wrapping, so for now highlight based on word match
            if let Some(pos) = line.to_lowercase().find(&m.word.to_lowercase()) {
                pos
            } else {
                continue;
            }
        } else {
            m.col
        };

        if visual_col >= chars.len() {
            continue;
        }

        // Add text before misspelling
        if visual_col > last_end {
            let before: String = chars[last_end..visual_col].iter().collect();
            spans.push(Span::raw(before));
        }

        // Add misspelled word with underline
        let word_end = (visual_col + m.word.len()).min(chars.len());
        let word: String = chars[visual_col..word_end].iter().collect();
        spans.push(Span::styled(
            word,
            Style::default().fg(Color::Red).add_modifier(Modifier::UNDERLINED),
        ));
        last_end = word_end;
    }

    // Add remaining text
    if last_end < chars.len() {
        let remaining: String = chars[last_end..].iter().collect();
        spans.push(Span::raw(remaining));
    }

    if spans.is_empty() {
        Line::from(line.to_string())
    } else {
        Line::from(spans)
    }
}

/// Apply search highlighting on top of an existing styled line
fn highlight_matches_on_line(line: &Line, query: &str) -> Line<'static> {
    if query.is_empty() {
        // Convert to owned by rebuilding
        let spans: Vec<Span<'static>> = line.spans.iter()
            .map(|s| Span::styled(s.content.to_string(), s.style))
            .collect();
        return Line::from(spans);
    }

    let query_lower = query.to_lowercase();
    let mut new_spans = Vec::new();

    for span in line.spans.iter() {
        let text = span.content.as_ref();
        let text_lower = text.to_lowercase();
        let base_style = span.style;

        let mut last_end = 0;
        for (start, _) in text_lower.match_indices(&query_lower) {
            // Add text before match with original style
            if start > last_end {
                new_spans.push(Span::styled(text[last_end..start].to_string(), base_style));
            }
            // Add highlighted match
            new_spans.push(Span::styled(
                text[start..start + query.len()].to_string(),
                Style::default().bg(Color::Yellow).fg(Color::Black),
            ));
            last_end = start + query.len();
        }

        // Add remaining text with original style
        if last_end < text.len() {
            new_spans.push(Span::styled(text[last_end..].to_string(), base_style));
        } else if last_end == 0 {
            // No matches in this span, keep original
            new_spans.push(Span::styled(text.to_string(), base_style));
        }
    }

    Line::from(new_spans)
}

fn render_status(frame: &mut Frame, area: Rect, state: &RenderState) {
    // Format per spec 2.4: "Words: NNN  |  Session: XXm  |  [Modified]"
    let modified_str = if state.modified { "  |  [Modified]" } else { "" };
    let saved_str = if state.show_saved_indicator { "  Saved" } else { "" };
    let spell_str = if state.spell_enabled { "  |  [Spell]" } else { "" };
    
    // Goal progress string
    let goal_str = if state.show_goal && state.daily_goal > 0 {
        if state.goal_met {
            // Celebration - subtle checkmark
            format!("  |  Goal: {} [done]", format_progress_bar(state.goal_progress))
        } else {
            format!("  |  Goal: {} ({}/{})", 
                format_progress_bar(state.goal_progress),
                state.word_count.min(state.daily_goal),
                state.daily_goal
            )
        }
    } else {
        String::new()
    };
    
    // Streak string
    let streak_str = if state.show_goal && state.streak > 0 {
        format!("  |  Streak: {} day{}", state.streak, if state.streak == 1 { "" } else { "s" })
    } else {
        String::new()
    };

    let status = format!(
        "Words: {}  |  Session: {}{}{}{}{}{}",
        state.word_count, state.elapsed, spell_str, goal_str, streak_str, modified_str, saved_str
    );

    let status_line = Paragraph::new(status)
        .style(Style::default()
            .fg(state.theme.status_text.to_color())
            .bg(state.theme.status_bg.to_color()))
        .alignment(Alignment::Center);

    frame.render_widget(status_line, area);
}

/// Format a progress bar like [====----] for goal progress
fn format_progress_bar(progress: f64) -> String {
    let total_chars = 8;
    let filled = ((progress.min(1.0) * total_chars as f64) as usize).min(total_chars);
    let empty = total_chars - filled;
    format!("[{}{}]", "=".repeat(filled), "-".repeat(empty))
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
    {/}             Move by paragraph
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
    s               Writing statistics
    v               Version history
    P               Project documents
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

fn render_stats_overlay(frame: &mut Frame, area: Rect, stats: Option<&WritingStats>) {
    let width = 50.min(area.width - 4);
    let height = 20.min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let overlay_area = Rect { x, y, width, height };
    frame.render_widget(Clear, overlay_area);

    let stats_text = if let Some(s) = stats {
        let productive_hour = s.most_productive_hour
            .map(|h| format!("{}:00", h))
            .unwrap_or_else(|| "N/A".to_string());
        
        // Build ASCII chart for last 7 days
        let max_words = s.words_last_7_days.iter().map(|(_, w)| *w).max().unwrap_or(1).max(1);
        let chart_height = 5;
        let mut chart_lines = vec![String::new(); chart_height];
        
        for (_, words) in &s.words_last_7_days {
            let bar_height = ((*words as f64 / max_words as f64) * chart_height as f64) as usize;
            for (row, line) in chart_lines.iter_mut().enumerate() {
                let ch = if chart_height - row <= bar_height { '#' } else { ' ' };
                line.push(ch);
                line.push(' ');
            }
        }
        
        let date_labels: String = s.words_last_7_days.iter()
            .map(|(d, _)| format!("{} ", d))
            .collect();

        format!(
            r#"
  WRITING STATISTICS

  Total Words:       {:>8}
  Total Sessions:    {:>8}
  Total Time:        {:>5} min
  
  Avg Words/Session: {:>8}
  Avg Session Time:  {:>5} min
  
  Current Streak:    {:>5} days
  Longest Streak:    {:>5} days
  Most Productive:   {:>8}

  Last 7 Days:
  {}
  {}
  {}
  {}
  {}
  {}

  Press any key to close
"#,
            s.total_words,
            s.total_sessions,
            s.total_minutes,
            s.avg_words_per_session,
            s.avg_session_minutes,
            s.current_streak,
            s.longest_streak,
            productive_hour,
            chart_lines.first().unwrap_or(&String::new()),
            chart_lines.get(1).unwrap_or(&String::new()),
            chart_lines.get(2).unwrap_or(&String::new()),
            chart_lines.get(3).unwrap_or(&String::new()),
            chart_lines.get(4).unwrap_or(&String::new()),
            date_labels,
        )
    } else {
        "  No statistics available yet.\n\n  Start writing to track your progress!\n\n  Press any key to close".to_string()
    };

    let stats_para = Paragraph::new(stats_text)
        .block(Block::default().borders(Borders::ALL).title(" Statistics "))
        .style(Style::default().fg(Color::White));

    frame.render_widget(stats_para, overlay_area);
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

fn render_versions_overlay(frame: &mut Frame, area: Rect, versions: &[Version], selected: usize) {
    let width = 60.min(area.width - 4);
    let height = 20.min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let overlay_area = Rect { x, y, width, height };
    frame.render_widget(Clear, overlay_area);

    let content_height = height.saturating_sub(4) as usize; // Account for border and help text

    if versions.is_empty() {
        let text = "\n  No versions saved yet.\n\n  Versions are created when you save.\n\n  Press Escape to close";
        let para = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Version History "))
            .style(Style::default().fg(Color::White));
        frame.render_widget(para, overlay_area);
        return;
    }

    // Build version list with selection highlight
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    // Calculate scroll offset to keep selection visible
    let scroll = if selected >= content_height.saturating_sub(2) {
        selected.saturating_sub(content_height.saturating_sub(3))
    } else {
        0
    };

    for (i, version) in versions.iter().enumerate().skip(scroll).take(content_height.saturating_sub(3)) {
        let prefix = if i == selected { "> " } else { "  " };
        let line_text = format!(
            "{}{}  {:>5} words  {}",
            prefix,
            version.formatted_time(),
            version.word_count,
            version.preview()
        );

        let style = if i == selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(Span::styled(line_text, style)));
    }

    // Add help text at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  j/k: navigate  Enter: view  d: diff  r: restore  q: close",
        Style::default().fg(Color::DarkGray),
    )));

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Version History "))
        .style(Style::default().fg(Color::White));

    frame.render_widget(para, overlay_area);
}

fn render_version_view(frame: &mut Frame, area: Rect, content: &str, time: &str) {
    let width = (area.width - 4).min(100);
    let height = area.height - 4;
    let x = (area.width - width) / 2;
    let y = 2;

    let overlay_area = Rect { x, y, width, height };
    frame.render_widget(Clear, overlay_area);

    let title = format!(" Version: {} (read-only) ", time);
    
    // Truncate content to visible area
    let visible_lines = height.saturating_sub(3) as usize;
    let display_content: String = content
        .lines()
        .take(visible_lines)
        .collect::<Vec<_>>()
        .join("\n");

    let para = Paragraph::new(display_content)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));

    frame.render_widget(para, overlay_area);

    // Show help at bottom
    let help_area = Rect {
        x: 0,
        y: area.height - 1,
        width: area.width,
        height: 1,
    };
    let help = Paragraph::new("  r: restore this version  q/Escape: back to list")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, help_area);
}

fn render_version_diff(frame: &mut Frame, area: Rect, diff: &str, time: &str) {
    let width = (area.width - 4).min(100);
    let height = area.height - 4;
    let x = (area.width - width) / 2;
    let y = 2;

    let overlay_area = Rect { x, y, width, height };
    frame.render_widget(Clear, overlay_area);

    let title = format!(" Diff: {} vs current ", time);

    // Style diff output with colors
    let visible_lines = height.saturating_sub(3) as usize;
    let lines: Vec<Line> = diff
        .lines()
        .take(visible_lines)
        .map(|line| {
            if line.starts_with('+') {
                Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Green)))
            } else if line.starts_with('-') {
                Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Red)))
            } else {
                Line::from(line.to_string())
            }
        })
        .collect();

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));

    frame.render_widget(para, overlay_area);

    // Show help at bottom
    let help_area = Rect {
        x: 0,
        y: area.height - 1,
        width: area.width,
        height: 1,
    };
    let help = Paragraph::new("  Press any key to return")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, help_area);
}

fn render_project_docs_overlay(
    frame: &mut Frame,
    area: Rect,
    project_name: &str,
    docs: &[String],
    selected: usize,
    current_doc: &str,
) {
    let width = 60.min(area.width - 4);
    let height = 20.min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let overlay_area = Rect { x, y, width, height };
    frame.render_widget(Clear, overlay_area);

    let content_height = height.saturating_sub(4) as usize;

    if docs.is_empty() {
        let text = "\n  No documents in project.\n\n  Use 'hollow project add' to add documents.\n\n  Press Escape to close";
        let title = format!(" {} ", project_name);
        let para = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(Color::White));
        frame.render_widget(para, overlay_area);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    let scroll = if selected >= content_height.saturating_sub(2) {
        selected.saturating_sub(content_height.saturating_sub(3))
    } else {
        0
    };

    for (i, doc) in docs.iter().enumerate().skip(scroll).take(content_height.saturating_sub(3)) {
        let is_current = doc == current_doc;
        let prefix = if i == selected { "> " } else { "  " };
        let suffix = if is_current { " [current]" } else { "" };
        let line_text = format!("{}{}{}", prefix, doc, suffix);

        let style = if i == selected {
            Style::default().fg(Color::Yellow)
        } else if is_current {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(Span::styled(line_text, style)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  j/k: navigate  Enter: open  q: close",
        Style::default().fg(Color::DarkGray),
    )));

    let title = format!(" {} ", project_name);
    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));

    frame.render_widget(para, overlay_area);
}

/// Render spell suggestions popup
fn render_spell_suggestions_overlay(
    frame: &mut Frame,
    area: Rect,
    word: &str,
    suggestions: &[String],
    selected: usize,
) {
    let width = 40.min(area.width - 4);
    let height = (suggestions.len() + 6).clamp(6, 15) as u16;
    let height = height.min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let overlay_area = Rect { x, y, width, height };
    frame.render_widget(Clear, overlay_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    if suggestions.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("  No suggestions for '{}'", word),
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Tab: add to dictionary  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let content_height = height.saturating_sub(5) as usize;
        let scroll = if selected >= content_height {
            selected.saturating_sub(content_height - 1)
        } else {
            0
        };

        for (i, suggestion) in suggestions.iter().enumerate().skip(scroll).take(content_height) {
            let prefix = if i == selected { "> " } else { "  " };
            let line_text = format!("{}{}", prefix, suggestion);

            let style = if i == selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(Span::styled(line_text, style)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  j/k: navigate  Enter: replace  Tab: add to dict  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let title = format!(" Suggestions for '{}' ", word);
    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));

    frame.render_widget(para, overlay_area);
}
