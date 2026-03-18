//! Collaboration UI components.
//!
//! Renders collaboration status, peer list, and session dialogs.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::collab::{CollabMode, CursorColor, PeerCursor, PEER_COLORS};

/// State for collaboration UI rendering.
#[derive(Debug, Default)]
pub struct CollabRenderState {
    /// Whether collaboration is active.
    pub active: bool,
    /// Current mode (host/client/none).
    pub mode: CollabMode,
    /// Session ID (when hosting).
    pub session_id: Option<String>,
    /// Connected peer count.
    pub peer_count: usize,
    /// Peer cursors for rendering.
    pub peers: Vec<PeerDisplay>,
    /// Show peer list overlay.
    pub show_peer_list: bool,
    /// Show host dialog.
    pub show_host_dialog: bool,
    /// Show join dialog.
    pub show_join_dialog: bool,
    /// Join dialog input.
    pub join_input: String,
    /// Connection status message.
    pub status_message: Option<String>,
}

/// Display info for a peer.
#[derive(Debug, Clone)]
pub struct PeerDisplay {
    /// Peer name.
    pub name: String,
    /// Cursor line.
    pub cursor_line: usize,
    /// Cursor column.
    pub cursor_column: usize,
    /// Color for this peer.
    pub color: CursorColor,
}

impl From<&PeerCursor> for PeerDisplay {
    fn from(cursor: &PeerCursor) -> Self {
        Self {
            name: cursor.name.clone(),
            cursor_line: cursor.position.line,
            cursor_column: cursor.position.column,
            color: cursor.color,
        }
    }
}

/// Render collaboration status in the status bar.
pub fn render_collab_status(state: &CollabRenderState) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    if !state.active {
        return spans;
    }

    let mode_str = match state.mode {
        CollabMode::Host => "HOST",
        CollabMode::Client => "COLLAB",
        CollabMode::None => return spans,
    };

    spans.push(Span::styled(
        format!(" {} ", mode_str),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    if state.peer_count > 0 {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("{} peer{}", state.peer_count, if state.peer_count == 1 { "" } else { "s" }),
            Style::default().fg(Color::Cyan),
        ));
    }

    if let Some(ref id) = state.session_id {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[{}]", id),
            Style::default().fg(Color::DarkGray),
        ));
    }

    spans
}

/// Render peer list overlay.
pub fn render_peer_list(frame: &mut Frame, state: &CollabRenderState) {
    if !state.show_peer_list {
        return;
    }

    let area = centered_rect(40, 60, frame.area());
    frame.render_widget(Clear, area);

    let title = match state.mode {
        CollabMode::Host => format!(" Hosting: {} ", state.session_id.as_deref().unwrap_or("?")),
        CollabMode::Client => " Connected Peers ".to_string(),
        CollabMode::None => " Collaboration ".to_string(),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.peers.is_empty() {
        let msg = Paragraph::new("No peers connected")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
        return;
    }

    let items: Vec<ListItem> = state
        .peers
        .iter()
        .map(|peer| {
            let color = Color::Rgb(peer.color.r, peer.color.g, peer.color.b);
            let line = Line::from(vec![
                Span::styled("● ", Style::default().fg(color)),
                Span::raw(&peer.name),
                Span::styled(
                    format!("  L{}:C{}", peer.cursor_line + 1, peer.cursor_column + 1),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

/// Render host session dialog.
pub fn render_host_dialog(frame: &mut Frame, session_id: &str) {
    let area = centered_rect(50, 30, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Share Session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(1),
    ])
    .split(inner);

    let intro = Paragraph::new("Share this code with collaborators:")
        .alignment(Alignment::Center);
    frame.render_widget(intro, chunks[0]);

    let code = Paragraph::new(session_id)
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(code, chunks[1]);

    let hint = Paragraph::new("They can join with: hollow --join <code>")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[2]);

    let footer = Paragraph::new("Press Esc to close")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);
}

/// Render join session dialog.
pub fn render_join_dialog(frame: &mut Frame, input: &str, status: Option<&str>) {
    let area = centered_rect(50, 30, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Join Session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(1),
    ])
    .split(inner);

    let intro = Paragraph::new("Enter session code:")
        .alignment(Alignment::Center);
    frame.render_widget(intro, chunks[0]);

    let input_display = if input.is_empty() {
        Span::styled("___", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(input, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    };
    let input_para = Paragraph::new(Line::from(vec![Span::raw("> "), input_display]))
        .alignment(Alignment::Center);
    frame.render_widget(input_para, chunks[1]);

    if let Some(status) = status {
        let status_para = Paragraph::new(status)
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        frame.render_widget(status_para, chunks[2]);
    }

    let footer = Paragraph::new("Enter to join • Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);
}

/// Render remote cursors in the editor.
pub fn render_remote_cursors(
    frame: &mut Frame,
    editor_area: Rect,
    peers: &[PeerDisplay],
    scroll_offset: usize,
    visible_lines: usize,
) {
    for peer in peers {
        // Check if cursor is visible
        if peer.cursor_line < scroll_offset {
            continue;
        }
        let screen_line = peer.cursor_line - scroll_offset;
        if screen_line >= visible_lines {
            continue;
        }

        // Calculate position
        let x = editor_area.x + peer.cursor_column as u16;
        let y = editor_area.y + screen_line as u16;

        // Check bounds
        if x >= editor_area.x + editor_area.width || y >= editor_area.y + editor_area.height {
            continue;
        }

        // Render cursor marker
        let color = Color::Rgb(peer.color.r, peer.color.g, peer.color.b);
        let cursor_area = Rect::new(x, y, 1, 1);
        let cursor = Paragraph::new("│")
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD));
        frame.render_widget(cursor, cursor_area);

        // Render name tag above cursor (if room)
        if y > editor_area.y {
            let name = if peer.name.len() > 8 {
                format!("{}…", &peer.name[..7])
            } else {
                peer.name.clone()
            };
            let tag_width = name.len() as u16 + 2;
            let tag_x = x.saturating_sub(1);
            if tag_x + tag_width <= editor_area.x + editor_area.width {
                let tag_area = Rect::new(tag_x, y - 1, tag_width, 1);
                let tag = Paragraph::new(format!(" {} ", name))
                    .style(Style::default().fg(Color::Black).bg(color));
                frame.render_widget(tag, tag_area);
            }
        }
    }
}

/// Get color for a new peer.
pub fn get_next_peer_color(peer_index: usize) -> CursorColor {
    PEER_COLORS[peer_index % PEER_COLORS.len()]
}

/// Center a rect within another rect.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collab_render_state_default() {
        let state = CollabRenderState::default();
        assert!(!state.active);
        assert_eq!(state.peer_count, 0);
    }

    #[test]
    fn test_render_collab_status_inactive() {
        let state = CollabRenderState::default();
        let spans = render_collab_status(&state);
        assert!(spans.is_empty());
    }

    #[test]
    fn test_render_collab_status_host() {
        let state = CollabRenderState {
            active: true,
            mode: CollabMode::Host,
            session_id: Some("ABC".to_string()),
            peer_count: 2,
            ..Default::default()
        };
        let spans = render_collab_status(&state);
        assert!(!spans.is_empty());
        
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("HOST"));
        assert!(text.contains("2 peers"));
        assert!(text.contains("[ABC]"));
    }

    #[test]
    fn test_render_collab_status_client() {
        let state = CollabRenderState {
            active: true,
            mode: CollabMode::Client,
            peer_count: 1,
            ..Default::default()
        };
        let spans = render_collab_status(&state);
        
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("COLLAB"));
        assert!(text.contains("1 peer"));
    }

    #[test]
    fn test_peer_display_from_cursor() {
        let cursor = PeerCursor::new("peer-1".to_string(), "Alice".to_string(), 0);
        let display = PeerDisplay::from(&cursor);
        
        assert_eq!(display.name, "Alice");
        assert_eq!(display.cursor_line, 0);
        assert_eq!(display.cursor_column, 0);
    }

    #[test]
    fn test_get_next_peer_color() {
        let c0 = get_next_peer_color(0);
        let c1 = get_next_peer_color(1);
        let c8 = get_next_peer_color(8); // Wraps around
        
        // Different colors for different indices
        assert_ne!((c0.r, c0.g, c0.b), (c1.r, c1.g, c1.b));
        // Index 8 wraps to same as index 0
        assert_eq!((c0.r, c0.g, c0.b), (c8.r, c8.g, c8.b));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect(50, 50, area);
        
        // Should be roughly centered
        assert!(centered.x > 0);
        assert!(centered.y > 0);
        assert!(centered.x + centered.width < area.width);
        assert!(centered.y + centered.height < area.height);
    }
}
