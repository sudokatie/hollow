//! Collaboration session management.
//!
//! Handles peer connections and session state.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// State of the collaboration session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Not in a collaborative session.
    Disconnected,
    /// Hosting a session, waiting for peers.
    Hosting,
    /// Connected to a host.
    Connected,
    /// Attempting to connect.
    Connecting,
}

/// Information about a connected peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Unique peer identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Cursor position in the document.
    pub cursor_pos: usize,
    /// Color index for UI display.
    pub color_index: u8,
    /// Whether this peer is currently active.
    pub active: bool,
}

/// Configuration for a collaboration session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Port to listen on when hosting.
    pub port: u16,
    /// Display name for this peer.
    pub name: String,
    /// Session ID (generated when hosting, provided when joining).
    pub session_id: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            port: 7878,
            name: whoami::username(),
            session_id: None,
        }
    }
}

/// Manages a collaboration session.
pub struct CollabSession {
    /// Current session state.
    state: SessionState,
    /// Session configuration.
    config: SessionConfig,
    /// Connected peers.
    peers: Vec<PeerInfo>,
    /// Our peer info.
    local_peer: PeerInfo,
}

impl CollabSession {
    /// Create a new disconnected session.
    pub fn new(name: &str) -> Self {
        let local_id = Uuid::new_v4().to_string();
        Self {
            state: SessionState::Disconnected,
            config: SessionConfig {
                name: name.to_string(),
                ..Default::default()
            },
            peers: Vec::new(),
            local_peer: PeerInfo {
                id: local_id,
                name: name.to_string(),
                cursor_pos: 0,
                color_index: 0,
                active: true,
            },
        }
    }

    /// Get current session state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Check if currently in a collaborative session.
    pub fn is_active(&self) -> bool {
        matches!(self.state, SessionState::Hosting | SessionState::Connected)
    }

    /// Get session ID if hosting.
    pub fn session_id(&self) -> Option<&str> {
        self.config.session_id.as_deref()
    }

    /// Get list of connected peers.
    pub fn peers(&self) -> &[PeerInfo] {
        &self.peers
    }

    /// Get peer count.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Get local peer info.
    pub fn local_peer(&self) -> &PeerInfo {
        &self.local_peer
    }

    /// Update local cursor position.
    pub fn update_cursor(&mut self, pos: usize) {
        self.local_peer.cursor_pos = pos;
    }

    /// Start hosting a new session.
    pub fn host(&mut self) -> String {
        let session_id = generate_session_id();
        self.config.session_id = Some(session_id.clone());
        self.state = SessionState::Hosting;
        self.peers.clear();
        session_id
    }

    /// Join an existing session.
    pub fn join(&mut self, session_id: &str) {
        self.config.session_id = Some(session_id.to_string());
        self.state = SessionState::Connecting;
    }

    /// Handle successful connection.
    pub fn on_connected(&mut self) {
        self.state = SessionState::Connected;
    }

    /// Handle peer joined.
    pub fn on_peer_joined(&mut self, peer: PeerInfo) {
        // Assign color index
        let color_index = self.peers.len() as u8 + 1;
        let mut peer = peer;
        peer.color_index = color_index;
        self.peers.push(peer);
    }

    /// Handle peer left.
    pub fn on_peer_left(&mut self, peer_id: &str) {
        self.peers.retain(|p| p.id != peer_id);
    }

    /// Handle peer cursor update.
    pub fn on_peer_cursor(&mut self, peer_id: &str, pos: usize) {
        if let Some(peer) = self.peers.iter_mut().find(|p| p.id == peer_id) {
            peer.cursor_pos = pos;
        }
    }

    /// Disconnect from current session.
    pub fn disconnect(&mut self) {
        self.state = SessionState::Disconnected;
        self.config.session_id = None;
        self.peers.clear();
    }
}

/// Generate a short, human-readable session ID.
fn generate_session_id() -> String {
    // Generate a 6-character alphanumeric ID
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    let chars: String = bytes[0..3]
        .iter()
        .map(|b| {
            let idx = (*b as usize) % 36;
            if idx < 10 {
                (b'0' + idx as u8) as char
            } else {
                (b'a' + (idx - 10) as u8) as char
            }
        })
        .collect();
    chars.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session() {
        let session = CollabSession::new("Alice");
        assert_eq!(session.state(), SessionState::Disconnected);
        assert!(!session.is_active());
        assert_eq!(session.peer_count(), 0);
    }

    #[test]
    fn test_host_session() {
        let mut session = CollabSession::new("Alice");
        let id = session.host();
        
        assert_eq!(session.state(), SessionState::Hosting);
        assert!(session.is_active());
        assert_eq!(session.session_id(), Some(id.as_str()));
    }

    #[test]
    fn test_session_id_format() {
        let id = generate_session_id();
        assert_eq!(id.len(), 3);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_join_session() {
        let mut session = CollabSession::new("Bob");
        session.join("ABC");
        
        assert_eq!(session.state(), SessionState::Connecting);
        assert!(!session.is_active());
    }

    #[test]
    fn test_peer_management() {
        let mut session = CollabSession::new("Alice");
        session.host();
        
        let peer = PeerInfo {
            id: "peer-1".to_string(),
            name: "Bob".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        session.on_peer_joined(peer);
        
        assert_eq!(session.peer_count(), 1);
        assert_eq!(session.peers()[0].name, "Bob");
        assert_eq!(session.peers()[0].color_index, 1);
    }

    #[test]
    fn test_peer_left() {
        let mut session = CollabSession::new("Alice");
        session.host();
        
        session.on_peer_joined(PeerInfo {
            id: "peer-1".to_string(),
            name: "Bob".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        });
        
        assert_eq!(session.peer_count(), 1);
        session.on_peer_left("peer-1");
        assert_eq!(session.peer_count(), 0);
    }

    #[test]
    fn test_cursor_update() {
        let mut session = CollabSession::new("Alice");
        session.update_cursor(42);
        assert_eq!(session.local_peer().cursor_pos, 42);
    }

    #[test]
    fn test_peer_cursor_update() {
        let mut session = CollabSession::new("Alice");
        session.host();
        
        session.on_peer_joined(PeerInfo {
            id: "peer-1".to_string(),
            name: "Bob".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        });
        
        session.on_peer_cursor("peer-1", 100);
        assert_eq!(session.peers()[0].cursor_pos, 100);
    }

    #[test]
    fn test_disconnect() {
        let mut session = CollabSession::new("Alice");
        session.host();
        session.on_peer_joined(PeerInfo {
            id: "peer-1".to_string(),
            name: "Bob".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        });
        
        session.disconnect();
        
        assert_eq!(session.state(), SessionState::Disconnected);
        assert!(!session.is_active());
        assert_eq!(session.peer_count(), 0);
        assert!(session.session_id().is_none());
    }
}
