//! Operation synchronization for collaborative editing.
//!
//! Orchestrates CRDT operations with network transport.

use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};

use super::crdt::{CollabDocument, Operation, OperationType};
use super::network::{NetworkClient, NetworkEvent, NetworkServer, DEFAULT_PORT};
use super::session::{CollabSession, PeerInfo};

/// Mode of collaboration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollabMode {
    /// Not collaborating.
    None,
    /// Hosting a session.
    Host,
    /// Connected to a host.
    Client,
}

/// Handles synchronization between local edits and remote peers.
pub struct CollabSync {
    /// Collaboration mode.
    mode: CollabMode,
    /// Session management.
    session: CollabSession,
    /// CRDT document.
    document: Arc<RwLock<CollabDocument>>,
    /// Network server (when hosting).
    server: Option<NetworkServer>,
    /// Network client (when joined).
    client: Option<NetworkClient>,
    /// Event receiver from network layer.
    event_rx: Option<mpsc::Receiver<NetworkEvent>>,
    /// Event sender (used internally).
    event_tx: mpsc::Sender<NetworkEvent>,
    /// Pending operations to batch.
    pending_ops: Vec<Operation>,
    /// Last cursor broadcast position.
    last_cursor: usize,
}

impl CollabSync {
    /// Create a new collaboration sync handler.
    pub fn new(name: &str) -> Self {
        let (event_tx, event_rx) = mpsc::channel(64);
        Self {
            mode: CollabMode::None,
            session: CollabSession::new(name),
            document: Arc::new(RwLock::new(CollabDocument::new())),
            server: None,
            client: None,
            event_rx: Some(event_rx),
            event_tx,
            pending_ops: Vec::new(),
            last_cursor: 0,
        }
    }

    /// Get current collaboration mode.
    pub fn mode(&self) -> CollabMode {
        self.mode
    }

    /// Check if actively collaborating.
    pub fn is_active(&self) -> bool {
        self.mode != CollabMode::None
    }

    /// Get session reference.
    pub fn session(&self) -> &CollabSession {
        &self.session
    }

    /// Get mutable session reference.
    pub fn session_mut(&mut self) -> &mut CollabSession {
        &mut self.session
    }

    /// Get document reference.
    pub fn document(&self) -> &Arc<RwLock<CollabDocument>> {
        &self.document
    }

    /// Host a new collaboration session.
    pub async fn host(&mut self, content: &str) -> std::io::Result<String> {
        self.host_on_port(content, DEFAULT_PORT).await
    }

    /// Host a new collaboration session on a specific port.
    pub async fn host_on_port(&mut self, content: &str, port: u16) -> std::io::Result<String> {
        let session_id = self.session.host();
        
        // Initialize document with current content
        {
            let mut doc = self.document.write().await;
            *doc = CollabDocument::with_content(content);
        }

        // Start network server
        let mut server = NetworkServer::new(
            session_id.clone(),
            self.session.local_peer().clone(),
            self.event_tx.clone(),
        );
        server.set_document(content.to_string()).await;
        server.start(port).await?;
        
        self.server = Some(server);
        self.mode = CollabMode::Host;

        Ok(session_id)
    }

    /// Initialize host mode without network (for testing).
    #[cfg(test)]
    pub async fn host_local(&mut self, content: &str) -> String {
        let session_id = self.session.host();
        
        {
            let mut doc = self.document.write().await;
            *doc = CollabDocument::with_content(content);
        }

        self.mode = CollabMode::Host;
        session_id
    }

    /// Join an existing collaboration session.
    pub async fn join(&mut self, addr: &str, session_id: &str) -> std::io::Result<()> {
        self.session.join(session_id);

        let mut client = NetworkClient::new(
            self.session.local_peer().clone(),
            self.event_tx.clone(),
        );
        client.connect(addr, session_id).await?;
        
        self.client = Some(client);
        self.session.on_connected();
        self.mode = CollabMode::Client;

        Ok(())
    }

    /// Disconnect from the current session.
    pub fn disconnect(&mut self) {
        if let Some(mut server) = self.server.take() {
            server.stop();
        }
        if let Some(mut client) = self.client.take() {
            client.disconnect();
        }
        self.session.disconnect();
        self.mode = CollabMode::None;
        self.pending_ops.clear();
    }

    /// Apply a local insert and broadcast to peers.
    pub async fn local_insert(&mut self, pos: usize, text: &str) {
        let op = {
            let mut doc = self.document.write().await;
            doc.insert(pos, text)
        };

        self.broadcast_operation(op).await;
    }

    /// Apply a local delete and broadcast to peers.
    pub async fn local_delete(&mut self, pos: usize, len: usize) {
        let op = {
            let mut doc = self.document.write().await;
            doc.delete(pos, len)
        };

        if let Some(op) = op {
            self.broadcast_operation(op).await;
        }
    }

    /// Apply a remote operation to the document.
    pub async fn apply_remote(&mut self, op: Operation) {
        let mut doc = self.document.write().await;
        doc.apply_remote(&op);
    }

    /// Update local cursor position.
    pub fn update_cursor(&mut self, pos: usize) {
        self.session.update_cursor(pos);
        
        // Only broadcast if cursor moved significantly
        if pos.abs_diff(self.last_cursor) >= 5 {
            self.broadcast_cursor(pos);
            self.last_cursor = pos;
        }
    }

    /// Broadcast cursor position to peers.
    pub fn broadcast_cursor(&self, pos: usize) {
        match self.mode {
            CollabMode::Host => {
                if let Some(server) = &self.server {
                    server.send_cursor(pos);
                }
            }
            CollabMode::Client => {
                // Client cursor updates are handled via send_cursor in the async context
                // This sync method only works for host mode
                let _ = pos; // Mark as intentionally unused for client mode
            }
            CollabMode::None => {}
        }
    }

    /// Broadcast an operation to all peers.
    async fn broadcast_operation(&self, op: Operation) {
        match self.mode {
            CollabMode::Host => {
                if let Some(server) = &self.server {
                    server.send_operation(op);
                }
            }
            CollabMode::Client => {
                if let Some(client) = &self.client {
                    client.send_operation(op).await;
                }
            }
            CollabMode::None => {}
        }
    }

    /// Take the event receiver (for integration with app event loop).
    pub fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<NetworkEvent>> {
        self.event_rx.take()
    }

    /// Process a network event.
    pub async fn handle_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::PeerConnected(peer) => {
                self.session.on_peer_joined(peer);
            }
            NetworkEvent::PeerDisconnected(peer_id) => {
                self.session.on_peer_left(&peer_id);
            }
            NetworkEvent::OperationReceived(op) => {
                self.apply_remote(op).await;
            }
            NetworkEvent::CursorUpdate { peer_id, position } => {
                self.session.on_peer_cursor(&peer_id, position);
            }
            NetworkEvent::Connected(host) => {
                self.session.on_connected();
                self.session.on_peer_joined(host);
            }
            NetworkEvent::DocumentReceived(content) => {
                let mut doc = self.document.write().await;
                *doc = CollabDocument::with_content(&content);
            }
            NetworkEvent::ConnectionFailed(reason) => {
                eprintln!("Connection failed: {}", reason);
                self.disconnect();
            }
        }
    }

    /// Get current document content.
    pub async fn content(&self) -> String {
        let doc = self.document.read().await;
        doc.content().to_string()
    }

    /// Get peer count.
    pub fn peer_count(&self) -> usize {
        self.session.peer_count()
    }

    /// Get connected peers.
    pub fn peers(&self) -> &[PeerInfo] {
        self.session.peers()
    }
}

/// Operation batching for efficiency.
pub struct OpBatcher {
    /// Pending operations.
    ops: Vec<Operation>,
    /// Time of last batch send (for timeout-based flushing).
    last_flush: std::time::Instant,
    /// Batch size threshold.
    batch_size: usize,
    /// Batch timeout in milliseconds.
    batch_timeout_ms: u64,
}

impl OpBatcher {
    /// Create a new operation batcher.
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            last_flush: std::time::Instant::now(),
            batch_size: 10,
            batch_timeout_ms: 50,
        }
    }

    /// Add an operation to the batch.
    pub fn add(&mut self, op: Operation) {
        self.ops.push(op);
    }

    /// Check if batch should be flushed.
    pub fn should_flush(&self) -> bool {
        self.ops.len() >= self.batch_size
            || (!self.ops.is_empty()
                && self.last_flush.elapsed().as_millis() >= self.batch_timeout_ms as u128)
    }

    /// Flush the batch, returning operations.
    pub fn flush(&mut self) -> Vec<Operation> {
        self.last_flush = std::time::Instant::now();
        std::mem::take(&mut self.ops)
    }

    /// Get pending operation count.
    pub fn pending(&self) -> usize {
        self.ops.len()
    }

    /// Compact consecutive operations when possible.
    pub fn compact(&mut self) {
        if self.ops.len() < 2 {
            return;
        }

        let ops = std::mem::take(&mut self.ops);
        let mut compacted = Vec::with_capacity(ops.len());
        let mut iter = ops.into_iter().peekable();

        while let Some(mut op) = iter.next() {
            // Try to merge consecutive inserts at adjacent positions
            while let Some(next) = iter.peek() {
                if op.op_type == OperationType::Insert
                    && next.op_type == OperationType::Insert
                    && next.position == op.position + op.content.len()
                    && op.agent_id == next.agent_id
                {
                    let next = iter.next().unwrap();
                    op.content.push_str(&next.content);
                } else {
                    break;
                }
            }
            compacted.push(op);
        }

        self.ops = compacted;
    }
}

impl Default for OpBatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collab_mode() {
        let sync = CollabSync::new("Alice");
        assert_eq!(sync.mode(), CollabMode::None);
        assert!(!sync.is_active());
    }

    #[tokio::test]
    async fn test_host_session() {
        let mut sync = CollabSync::new("Alice");
        let session_id = sync.host_local("Hello").await;
        
        assert_eq!(sync.mode(), CollabMode::Host);
        assert!(sync.is_active());
        assert!(!session_id.is_empty());
        
        let content = sync.content().await;
        assert_eq!(content, "Hello");
        
        sync.disconnect();
        assert_eq!(sync.mode(), CollabMode::None);
    }

    #[tokio::test]
    async fn test_local_insert() {
        let mut sync = CollabSync::new("Alice");
        sync.host_local("Hello").await;
        
        sync.local_insert(5, " world").await;
        
        let content = sync.content().await;
        assert_eq!(content, "Hello world");
    }

    #[tokio::test]
    async fn test_local_delete() {
        let mut sync = CollabSync::new("Alice");
        sync.host_local("Hello world").await;
        
        sync.local_delete(5, 6).await;
        
        let content = sync.content().await;
        assert_eq!(content, "Hello");
    }

    #[tokio::test]
    async fn test_apply_remote() {
        let mut sync = CollabSync::new("Alice");
        sync.host_local("Hello").await;
        
        let op = Operation {
            op_type: OperationType::Insert,
            position: 5,
            content: " world".to_string(),
            agent_id: 2,
            seq: 0,
        };
        sync.apply_remote(op).await;
        
        let content = sync.content().await;
        assert_eq!(content, "Hello world");
    }

    #[test]
    fn test_op_batcher_new() {
        let batcher = OpBatcher::new();
        assert_eq!(batcher.pending(), 0);
        assert!(!batcher.should_flush());
    }

    #[test]
    fn test_op_batcher_add() {
        let mut batcher = OpBatcher::new();
        let op = Operation {
            op_type: OperationType::Insert,
            position: 0,
            content: "a".to_string(),
            agent_id: 1,
            seq: 0,
        };
        batcher.add(op);
        assert_eq!(batcher.pending(), 1);
    }

    #[test]
    fn test_op_batcher_flush() {
        let mut batcher = OpBatcher::new();
        for i in 0..5 {
            batcher.add(Operation {
                op_type: OperationType::Insert,
                position: i,
                content: "a".to_string(),
                agent_id: 1,
                seq: i as u32,
            });
        }
        
        let ops = batcher.flush();
        assert_eq!(ops.len(), 5);
        assert_eq!(batcher.pending(), 0);
    }

    #[test]
    fn test_op_batcher_should_flush_by_size() {
        let mut batcher = OpBatcher::new();
        for i in 0..10 {
            batcher.add(Operation {
                op_type: OperationType::Insert,
                position: i,
                content: "a".to_string(),
                agent_id: 1,
                seq: i as u32,
            });
        }
        assert!(batcher.should_flush());
    }

    #[test]
    fn test_op_batcher_compact() {
        let mut batcher = OpBatcher::new();
        // Add consecutive inserts
        batcher.add(Operation {
            op_type: OperationType::Insert,
            position: 0,
            content: "H".to_string(),
            agent_id: 1,
            seq: 0,
        });
        batcher.add(Operation {
            op_type: OperationType::Insert,
            position: 1,
            content: "e".to_string(),
            agent_id: 1,
            seq: 1,
        });
        batcher.add(Operation {
            op_type: OperationType::Insert,
            position: 2,
            content: "llo".to_string(),
            agent_id: 1,
            seq: 2,
        });
        
        batcher.compact();
        
        assert_eq!(batcher.pending(), 1);
        let ops = batcher.flush();
        assert_eq!(ops[0].content, "Hello");
    }

    #[test]
    fn test_op_batcher_compact_different_agents() {
        let mut batcher = OpBatcher::new();
        // Different agents shouldn't compact
        batcher.add(Operation {
            op_type: OperationType::Insert,
            position: 0,
            content: "a".to_string(),
            agent_id: 1,
            seq: 0,
        });
        batcher.add(Operation {
            op_type: OperationType::Insert,
            position: 1,
            content: "b".to_string(),
            agent_id: 2, // Different agent
            seq: 0,
        });
        
        batcher.compact();
        
        assert_eq!(batcher.pending(), 2);
    }

    #[tokio::test]
    async fn test_handle_peer_event() {
        let mut sync = CollabSync::new("Alice");
        sync.host_local("Hello").await;
        
        let peer = PeerInfo {
            id: "peer-1".to_string(),
            name: "Bob".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        
        sync.handle_event(NetworkEvent::PeerConnected(peer)).await;
        
        assert_eq!(sync.peer_count(), 1);
        assert_eq!(sync.peers()[0].name, "Bob");
    }

    #[tokio::test]
    async fn test_handle_operation_event() {
        let mut sync = CollabSync::new("Alice");
        sync.host_local("Hello").await;
        
        let op = Operation {
            op_type: OperationType::Insert,
            position: 5,
            content: "!".to_string(),
            agent_id: 2,
            seq: 0,
        };
        
        sync.handle_event(NetworkEvent::OperationReceived(op)).await;
        
        let content = sync.content().await;
        assert_eq!(content, "Hello!");
    }

    #[tokio::test]
    async fn test_handle_cursor_event() {
        let mut sync = CollabSync::new("Alice");
        sync.host_local("Hello").await;
        
        let peer = PeerInfo {
            id: "peer-1".to_string(),
            name: "Bob".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        sync.handle_event(NetworkEvent::PeerConnected(peer)).await;
        
        sync.handle_event(NetworkEvent::CursorUpdate {
            peer_id: "peer-1".to_string(),
            position: 42,
        }).await;
        
        assert_eq!(sync.peers()[0].cursor_pos, 42);
    }

    #[tokio::test]
    async fn test_handle_disconnect_event() {
        let mut sync = CollabSync::new("Alice");
        sync.host_local("Hello").await;
        
        let peer = PeerInfo {
            id: "peer-1".to_string(),
            name: "Bob".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        sync.handle_event(NetworkEvent::PeerConnected(peer)).await;
        assert_eq!(sync.peer_count(), 1);
        
        sync.handle_event(NetworkEvent::PeerDisconnected("peer-1".to_string())).await;
        assert_eq!(sync.peer_count(), 0);
    }
}
