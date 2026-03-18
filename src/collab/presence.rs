//! Cursor presence tracking for collaborative editing.
//!
//! Tracks local and remote cursor positions with throttled updates
//! and visual styling for peer differentiation.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Represents a cursor position in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorPosition {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub column: usize,
    /// Absolute character offset.
    pub offset: usize,
}

impl CursorPosition {
    /// Create a new cursor position.
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self { line, column, offset }
    }

    /// Create from just an offset (line/column calculated separately).
    pub fn from_offset(offset: usize) -> Self {
        Self {
            line: 0,
            column: 0,
            offset,
        }
    }

    /// Calculate line and column from text content.
    pub fn from_offset_in_text(offset: usize, text: &str) -> Self {
        let offset = offset.min(text.len());
        let mut line = 0;
        let mut column = 0;
        
        for (i, ch) in text.chars().enumerate() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        
        Self { line, column, offset }
    }
}

impl Default for CursorPosition {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

/// Selection range (if user has selected text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Start of selection.
    pub start: CursorPosition,
    /// End of selection.
    pub end: CursorPosition,
}

impl Selection {
    /// Create a new selection.
    pub fn new(start: CursorPosition, end: CursorPosition) -> Self {
        Self { start, end }
    }

    /// Check if selection is empty (cursor only).
    pub fn is_empty(&self) -> bool {
        self.start.offset == self.end.offset
    }

    /// Get selection length in characters.
    pub fn len(&self) -> usize {
        if self.start.offset <= self.end.offset {
            self.end.offset - self.start.offset
        } else {
            self.start.offset - self.end.offset
        }
    }

    /// Get normalized selection (start <= end).
    pub fn normalized(&self) -> Self {
        if self.start.offset <= self.end.offset {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }
}

/// Colors for peer cursors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorColor {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
}

impl CursorColor {
    /// Create a new color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to ANSI 256-color index (approximate).
    pub fn to_ansi256(&self) -> u8 {
        // Use 6x6x6 color cube (indices 16-231)
        let r = (self.r as u16 * 5 / 255) as u8;
        let g = (self.g as u16 * 5 / 255) as u8;
        let b = (self.b as u16 * 5 / 255) as u8;
        16 + 36 * r + 6 * g + b
    }

    /// Get a contrasting text color (black or white).
    pub fn contrasting_text(&self) -> CursorColor {
        // Use relative luminance formula
        let luminance = 0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32;
        if luminance > 128.0 {
            Self::new(0, 0, 0) // Black text on light background
        } else {
            Self::new(255, 255, 255) // White text on dark background
        }
    }
}

/// Predefined colors for peer cursors.
pub const PEER_COLORS: [CursorColor; 8] = [
    CursorColor::new(255, 107, 107), // Red
    CursorColor::new(78, 205, 196),  // Teal
    CursorColor::new(255, 230, 109), // Yellow
    CursorColor::new(199, 125, 255), // Purple
    CursorColor::new(107, 185, 240), // Blue
    CursorColor::new(255, 159, 67),  // Orange
    CursorColor::new(46, 213, 115),  // Green
    CursorColor::new(255, 121, 198), // Pink
];

/// Get color for a peer by index.
pub fn get_peer_color(index: usize) -> CursorColor {
    PEER_COLORS[index % PEER_COLORS.len()]
}

/// State of a remote peer's cursor.
#[derive(Debug, Clone)]
pub struct PeerCursor {
    /// Peer identifier.
    pub peer_id: String,
    /// Peer display name.
    pub name: String,
    /// Current cursor position.
    pub position: CursorPosition,
    /// Current selection (if any).
    pub selection: Option<Selection>,
    /// Color for this peer.
    pub color: CursorColor,
    /// Last update time.
    pub last_update: Instant,
    /// Whether cursor is visible (recently active).
    pub visible: bool,
}

impl PeerCursor {
    /// Create a new peer cursor.
    pub fn new(peer_id: String, name: String, color_index: usize) -> Self {
        Self {
            peer_id,
            name,
            position: CursorPosition::default(),
            selection: None,
            color: get_peer_color(color_index),
            last_update: Instant::now(),
            visible: true,
        }
    }

    /// Update cursor position.
    pub fn update_position(&mut self, position: CursorPosition) {
        self.position = position;
        self.last_update = Instant::now();
        self.visible = true;
    }

    /// Update selection.
    pub fn update_selection(&mut self, selection: Option<Selection>) {
        self.selection = selection;
        self.last_update = Instant::now();
    }

    /// Check if cursor is stale (no recent updates).
    pub fn is_stale(&self, timeout: Duration) -> bool {
        self.last_update.elapsed() > timeout
    }

    /// Mark cursor as invisible if stale.
    pub fn check_visibility(&mut self, timeout: Duration) {
        if self.is_stale(timeout) {
            self.visible = false;
        }
    }
}

/// Manages presence state for all peers.
pub struct PresenceManager {
    /// Remote peer cursors.
    peers: HashMap<String, PeerCursor>,
    /// Local cursor position.
    local_position: CursorPosition,
    /// Local selection.
    local_selection: Option<Selection>,
    /// Last broadcast time.
    last_broadcast: Instant,
    /// Minimum interval between broadcasts.
    broadcast_interval: Duration,
    /// Timeout for marking cursors as stale.
    stale_timeout: Duration,
    /// Color index counter for new peers.
    next_color_index: usize,
}

impl PresenceManager {
    /// Create a new presence manager.
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            local_position: CursorPosition::default(),
            local_selection: None,
            last_broadcast: Instant::now(),
            broadcast_interval: Duration::from_millis(50),
            stale_timeout: Duration::from_secs(30),
            next_color_index: 0,
        }
    }

    /// Get local cursor position.
    pub fn local_position(&self) -> CursorPosition {
        self.local_position
    }

    /// Get local selection.
    pub fn local_selection(&self) -> Option<Selection> {
        self.local_selection
    }

    /// Update local cursor position.
    /// Returns true if broadcast is needed.
    pub fn update_local(&mut self, position: CursorPosition) -> bool {
        let changed = self.local_position != position;
        self.local_position = position;
        
        if changed && self.last_broadcast.elapsed() >= self.broadcast_interval {
            self.last_broadcast = Instant::now();
            true
        } else {
            false
        }
    }

    /// Update local selection.
    pub fn update_local_selection(&mut self, selection: Option<Selection>) {
        self.local_selection = selection;
    }

    /// Force a broadcast (regardless of throttle).
    pub fn force_broadcast(&mut self) {
        self.last_broadcast = Instant::now() - self.broadcast_interval;
    }

    /// Add a peer.
    pub fn add_peer(&mut self, peer_id: String, name: String) {
        let color_index = self.next_color_index;
        self.next_color_index += 1;
        self.peers.insert(
            peer_id.clone(),
            PeerCursor::new(peer_id, name, color_index),
        );
    }

    /// Remove a peer.
    pub fn remove_peer(&mut self, peer_id: &str) {
        self.peers.remove(peer_id);
    }

    /// Update a peer's cursor position.
    pub fn update_peer(&mut self, peer_id: &str, offset: usize, text: &str) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            let position = CursorPosition::from_offset_in_text(offset, text);
            peer.update_position(position);
        }
    }

    /// Update a peer's cursor with full position.
    pub fn update_peer_position(&mut self, peer_id: &str, position: CursorPosition) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.update_position(position);
        }
    }

    /// Get all visible peer cursors.
    pub fn visible_peers(&self) -> Vec<&PeerCursor> {
        self.peers.values().filter(|p| p.visible).collect()
    }

    /// Get all peer cursors.
    pub fn all_peers(&self) -> Vec<&PeerCursor> {
        self.peers.values().collect()
    }

    /// Get a specific peer cursor.
    pub fn get_peer(&self, peer_id: &str) -> Option<&PeerCursor> {
        self.peers.get(peer_id)
    }

    /// Get peer count.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Check and update visibility of all cursors.
    pub fn update_visibility(&mut self) {
        for peer in self.peers.values_mut() {
            peer.check_visibility(self.stale_timeout);
        }
    }

    /// Get cursors on a specific line.
    pub fn cursors_on_line(&self, line: usize) -> Vec<&PeerCursor> {
        self.peers
            .values()
            .filter(|p| p.visible && p.position.line == line)
            .collect()
    }

    /// Clear all peers.
    pub fn clear(&mut self) {
        self.peers.clear();
        self.next_color_index = 0;
    }
}

impl Default for PresenceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_position_default() {
        let pos = CursorPosition::default();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
        assert_eq!(pos.offset, 0);
    }

    #[test]
    fn test_cursor_position_from_offset() {
        let pos = CursorPosition::from_offset(42);
        assert_eq!(pos.offset, 42);
    }

    #[test]
    fn test_cursor_position_from_text() {
        let text = "Hello\nWorld\n!";
        
        // Start of first line
        let pos = CursorPosition::from_offset_in_text(0, text);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
        
        // Middle of first line
        let pos = CursorPosition::from_offset_in_text(3, text);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 3);
        
        // Start of second line
        let pos = CursorPosition::from_offset_in_text(6, text);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);
        
        // Middle of second line
        let pos = CursorPosition::from_offset_in_text(9, text);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 3);
    }

    #[test]
    fn test_selection_empty() {
        let pos = CursorPosition::new(0, 0, 0);
        let sel = Selection::new(pos, pos);
        assert!(sel.is_empty());
        assert_eq!(sel.len(), 0);
    }

    #[test]
    fn test_selection_length() {
        let start = CursorPosition::new(0, 0, 5);
        let end = CursorPosition::new(0, 10, 15);
        let sel = Selection::new(start, end);
        assert!(!sel.is_empty());
        assert_eq!(sel.len(), 10);
    }

    #[test]
    fn test_selection_normalized() {
        let start = CursorPosition::new(0, 10, 15);
        let end = CursorPosition::new(0, 0, 5);
        let sel = Selection::new(start, end);
        
        let norm = sel.normalized();
        assert_eq!(norm.start.offset, 5);
        assert_eq!(norm.end.offset, 15);
    }

    #[test]
    fn test_cursor_color_ansi256() {
        let red = CursorColor::new(255, 0, 0);
        let ansi = red.to_ansi256();
        assert!(ansi >= 16 && ansi <= 231);
    }

    #[test]
    fn test_cursor_color_contrasting() {
        let white = CursorColor::new(255, 255, 255);
        let contrast = white.contrasting_text();
        assert_eq!(contrast.r, 0); // Black text
        
        let black = CursorColor::new(0, 0, 0);
        let contrast = black.contrasting_text();
        assert_eq!(contrast.r, 255); // White text
    }

    #[test]
    fn test_peer_colors() {
        assert_eq!(PEER_COLORS.len(), 8);
        for i in 0..16 {
            let color = get_peer_color(i);
            assert!(color.r > 0 || color.g > 0 || color.b > 0);
        }
    }

    #[test]
    fn test_peer_cursor_new() {
        let cursor = PeerCursor::new("peer-1".to_string(), "Alice".to_string(), 0);
        assert_eq!(cursor.peer_id, "peer-1");
        assert_eq!(cursor.name, "Alice");
        assert!(cursor.visible);
    }

    #[test]
    fn test_peer_cursor_update() {
        let mut cursor = PeerCursor::new("peer-1".to_string(), "Alice".to_string(), 0);
        let pos = CursorPosition::new(5, 10, 50);
        cursor.update_position(pos);
        
        assert_eq!(cursor.position.line, 5);
        assert_eq!(cursor.position.column, 10);
        assert_eq!(cursor.position.offset, 50);
    }

    #[test]
    fn test_presence_manager_new() {
        let manager = PresenceManager::new();
        assert_eq!(manager.peer_count(), 0);
        assert_eq!(manager.local_position().offset, 0);
    }

    #[test]
    fn test_presence_manager_add_peer() {
        let mut manager = PresenceManager::new();
        manager.add_peer("peer-1".to_string(), "Alice".to_string());
        manager.add_peer("peer-2".to_string(), "Bob".to_string());
        
        assert_eq!(manager.peer_count(), 2);
        assert!(manager.get_peer("peer-1").is_some());
        assert!(manager.get_peer("peer-2").is_some());
    }

    #[test]
    fn test_presence_manager_remove_peer() {
        let mut manager = PresenceManager::new();
        manager.add_peer("peer-1".to_string(), "Alice".to_string());
        assert_eq!(manager.peer_count(), 1);
        
        manager.remove_peer("peer-1");
        assert_eq!(manager.peer_count(), 0);
    }

    #[test]
    fn test_presence_manager_update_local() {
        let mut manager = PresenceManager::new();
        
        // Force initial state to allow broadcast
        manager.force_broadcast();
        
        let pos = CursorPosition::new(1, 5, 10);
        
        // First update should broadcast (after force)
        assert!(manager.update_local(pos));
        
        // Immediate same position shouldn't broadcast (not changed)
        assert!(!manager.update_local(pos));
        
        // Different position but too soon shouldn't broadcast
        let pos2 = CursorPosition::new(2, 0, 20);
        assert!(!manager.update_local(pos2));
    }

    #[test]
    fn test_presence_manager_update_peer() {
        let mut manager = PresenceManager::new();
        manager.add_peer("peer-1".to_string(), "Alice".to_string());
        
        let text = "Hello\nWorld";
        manager.update_peer("peer-1", 8, text);
        
        let peer = manager.get_peer("peer-1").unwrap();
        assert_eq!(peer.position.line, 1);
        assert_eq!(peer.position.column, 2);
    }

    #[test]
    fn test_presence_manager_visible_peers() {
        let mut manager = PresenceManager::new();
        manager.add_peer("peer-1".to_string(), "Alice".to_string());
        manager.add_peer("peer-2".to_string(), "Bob".to_string());
        
        let visible = manager.visible_peers();
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn test_presence_manager_cursors_on_line() {
        let mut manager = PresenceManager::new();
        manager.add_peer("peer-1".to_string(), "Alice".to_string());
        manager.add_peer("peer-2".to_string(), "Bob".to_string());
        
        // Update peer-1 to line 1
        manager.update_peer_position("peer-1", CursorPosition::new(1, 0, 10));
        // peer-2 stays at line 0
        
        let line_0 = manager.cursors_on_line(0);
        assert_eq!(line_0.len(), 1);
        assert_eq!(line_0[0].name, "Bob");
        
        let line_1 = manager.cursors_on_line(1);
        assert_eq!(line_1.len(), 1);
        assert_eq!(line_1[0].name, "Alice");
    }

    #[test]
    fn test_presence_manager_clear() {
        let mut manager = PresenceManager::new();
        manager.add_peer("peer-1".to_string(), "Alice".to_string());
        manager.add_peer("peer-2".to_string(), "Bob".to_string());
        
        manager.clear();
        assert_eq!(manager.peer_count(), 0);
    }

    #[test]
    fn test_different_peer_colors() {
        let mut manager = PresenceManager::new();
        manager.add_peer("peer-1".to_string(), "Alice".to_string());
        manager.add_peer("peer-2".to_string(), "Bob".to_string());
        
        let p1 = manager.get_peer("peer-1").unwrap();
        let p2 = manager.get_peer("peer-2").unwrap();
        
        // Different peers should have different colors
        assert_ne!(
            (p1.color.r, p1.color.g, p1.color.b),
            (p2.color.r, p2.color.g, p2.color.b)
        );
    }
}
