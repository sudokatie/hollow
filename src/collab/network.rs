//! Peer-to-peer networking for collaboration.
//!
//! TCP-based networking layer for real-time document collaboration.
//! Supports both hosting (server) and joining (client) modes.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, RwLock};

use super::session::PeerInfo;
use super::crdt::Operation;

/// Default port for collaboration server.
pub const DEFAULT_PORT: u16 = 7878;

/// Messages sent between peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerMessage {
    /// Request to join a session.
    Join {
        session_id: String,
        peer_info: PeerInfo,
    },
    /// Acknowledge successful join.
    JoinAck {
        host_info: PeerInfo,
        peers: Vec<PeerInfo>,
        document: String,
    },
    /// Reject join request.
    JoinReject { reason: String },
    /// A peer joined the session.
    PeerJoined { peer: PeerInfo },
    /// A peer left the session.
    PeerLeft { peer_id: String },
    /// Document operation from a peer.
    Operation { op: Operation },
    /// Cursor position update.
    Cursor { peer_id: String, position: usize },
    /// Heartbeat to keep connection alive.
    Ping,
    /// Heartbeat response.
    Pong,
    /// Graceful disconnect.
    Goodbye,
}

impl PeerMessage {
    /// Serialize to JSON line (newline-delimited).
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let mut json = serde_json::to_string(self)?;
        json.push('\n');
        Ok(json)
    }

    /// Deserialize from JSON line.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json.trim())
    }
}

/// Events emitted by the network layer.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// A peer connected and joined.
    PeerConnected(PeerInfo),
    /// A peer disconnected.
    PeerDisconnected(String),
    /// Received an operation from a peer.
    OperationReceived(Operation),
    /// Received cursor update from a peer.
    CursorUpdate { peer_id: String, position: usize },
    /// Connection established (for client).
    Connected(PeerInfo),
    /// Connection failed.
    ConnectionFailed(String),
    /// Host sent document state.
    DocumentReceived(String),
}

/// State of a connected peer.
#[derive(Debug)]
struct PeerConnection {
    info: PeerInfo,
    tx: mpsc::Sender<PeerMessage>,
}

/// Network server for hosting a collaboration session.
pub struct NetworkServer {
    /// Session ID being hosted.
    session_id: String,
    /// Local peer info.
    local_peer: PeerInfo,
    /// Current document content (for new joiners).
    document: Arc<RwLock<String>>,
    /// Connected peers.
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    /// Broadcast channel for outgoing messages.
    broadcast_tx: broadcast::Sender<PeerMessage>,
    /// Event channel.
    event_tx: mpsc::Sender<NetworkEvent>,
    /// Shutdown signal.
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl NetworkServer {
    /// Create a new network server.
    pub fn new(
        session_id: String,
        local_peer: PeerInfo,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(64);
        Self {
            session_id,
            local_peer,
            document: Arc::new(RwLock::new(String::new())),
            peers: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            event_tx,
            shutdown_tx: None,
        }
    }

    /// Update the document content.
    pub async fn set_document(&self, content: String) {
        let mut doc = self.document.write().await;
        *doc = content;
    }

    /// Get connected peer count.
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    /// Start listening for connections.
    pub async fn start(&mut self, port: u16) -> io::Result<SocketAddr> {
        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        let addr = listener.local_addr()?;

        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let session_id = self.session_id.clone();
        let local_peer = self.local_peer.clone();
        let document = self.document.clone();
        let peers = self.peers.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_tx.subscribe();
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, peer_addr)) => {
                                Self::handle_connection(
                                    stream,
                                    peer_addr,
                                    session_id.clone(),
                                    local_peer.clone(),
                                    document.clone(),
                                    peers.clone(),
                                    broadcast_tx.clone(),
                                    event_tx.clone(),
                                );
                            }
                            Err(e) => {
                                eprintln!("Accept error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(addr)
    }

    /// Stop the server.
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Broadcast a message to all peers.
    pub fn broadcast(&self, msg: PeerMessage) {
        let _ = self.broadcast_tx.send(msg);
    }

    /// Send operation to all peers.
    pub fn send_operation(&self, op: Operation) {
        self.broadcast(PeerMessage::Operation { op });
    }

    /// Send cursor update to all peers.
    pub fn send_cursor(&self, position: usize) {
        self.broadcast(PeerMessage::Cursor {
            peer_id: self.local_peer.id.clone(),
            position,
        });
    }

    fn handle_connection(
        stream: TcpStream,
        _peer_addr: SocketAddr,
        session_id: String,
        local_peer: PeerInfo,
        document: Arc<RwLock<String>>,
        peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
        broadcast_tx: broadcast::Sender<PeerMessage>,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) {
        tokio::spawn(async move {
            if let Err(e) = Self::run_connection(
                stream,
                session_id,
                local_peer,
                document,
                peers,
                broadcast_tx,
                event_tx,
            )
            .await
            {
                eprintln!("Connection error: {}", e);
            }
        });
    }

    async fn run_connection(
        stream: TcpStream,
        session_id: String,
        local_peer: PeerInfo,
        document: Arc<RwLock<String>>,
        peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
        broadcast_tx: broadcast::Sender<PeerMessage>,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) -> io::Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        // Wait for join request
        reader.read_line(&mut line).await?;
        let msg = PeerMessage::from_json(&line)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let peer_info = match msg {
            PeerMessage::Join {
                session_id: join_session_id,
                peer_info,
            } => {
                if join_session_id != session_id {
                    let reject = PeerMessage::JoinReject {
                        reason: "Invalid session ID".to_string(),
                    };
                    writer.write_all(reject.to_json_line().unwrap().as_bytes()).await?;
                    return Ok(());
                }
                peer_info
            }
            _ => {
                let reject = PeerMessage::JoinReject {
                    reason: "Expected Join message".to_string(),
                };
                writer.write_all(reject.to_json_line().unwrap().as_bytes()).await?;
                return Ok(());
            }
        };

        // Send ack with current state
        let current_peers: Vec<PeerInfo> = peers.read().await.values().map(|p| p.info.clone()).collect();
        let doc_content = document.read().await.clone();
        let ack = PeerMessage::JoinAck {
            host_info: local_peer,
            peers: current_peers.clone(),
            document: doc_content,
        };
        writer.write_all(ack.to_json_line().unwrap().as_bytes()).await?;

        // Create channel for sending to this peer
        let (tx, mut rx) = mpsc::channel::<PeerMessage>(32);

        // Store peer connection
        let peer_id = peer_info.id.clone();
        {
            let mut peers_write = peers.write().await;
            peers_write.insert(
                peer_id.clone(),
                PeerConnection {
                    info: peer_info.clone(),
                    tx: tx.clone(),
                },
            );
        }

        // Notify about new peer
        let _ = event_tx.send(NetworkEvent::PeerConnected(peer_info.clone())).await;

        // Broadcast to other peers
        let _ = broadcast_tx.send(PeerMessage::PeerJoined { peer: peer_info.clone() });

        // Subscribe to broadcasts
        let mut broadcast_rx = broadcast_tx.subscribe();

        // Connection loop
        loop {
            line.clear();
            tokio::select! {
                result = reader.read_line(&mut line) => {
                    match result {
                        Ok(0) => break, // Connection closed
                        Ok(_) => {
                            if let Ok(msg) = PeerMessage::from_json(&line) {
                                match msg {
                                    PeerMessage::Operation { op } => {
                                        // Forward to app and broadcast to others
                                        let _ = event_tx.send(NetworkEvent::OperationReceived(op.clone())).await;
                                        let _ = broadcast_tx.send(PeerMessage::Operation { op });
                                    }
                                    PeerMessage::Cursor { peer_id: pid, position } => {
                                        let _ = event_tx.send(NetworkEvent::CursorUpdate { peer_id: pid.clone(), position }).await;
                                        let _ = broadcast_tx.send(PeerMessage::Cursor { peer_id: pid, position });
                                    }
                                    PeerMessage::Ping => {
                                        let pong = PeerMessage::Pong;
                                        let _ = writer.write_all(pong.to_json_line().unwrap().as_bytes()).await;
                                    }
                                    PeerMessage::Goodbye => break,
                                    _ => {}
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                msg = rx.recv() => {
                    if let Some(msg) = msg {
                        let _ = writer.write_all(msg.to_json_line().unwrap().as_bytes()).await;
                    }
                }
                msg = broadcast_rx.recv() => {
                    if let Ok(msg) = msg {
                        // Don't echo back to sender
                        let _ = writer.write_all(msg.to_json_line().unwrap().as_bytes()).await;
                    }
                }
            }
        }

        // Clean up
        {
            let mut peers_write = peers.write().await;
            peers_write.remove(&peer_id);
        }
        let _ = event_tx.send(NetworkEvent::PeerDisconnected(peer_id.clone())).await;
        let _ = broadcast_tx.send(PeerMessage::PeerLeft { peer_id });

        Ok(())
    }
}

/// Network client for joining a collaboration session.
pub struct NetworkClient {
    /// Local peer info.
    local_peer: PeerInfo,
    /// Message sender to server.
    tx: Option<mpsc::Sender<PeerMessage>>,
    /// Event sender.
    event_tx: mpsc::Sender<NetworkEvent>,
    /// Shutdown signal.
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl NetworkClient {
    /// Create a new network client.
    pub fn new(local_peer: PeerInfo, event_tx: mpsc::Sender<NetworkEvent>) -> Self {
        Self {
            local_peer,
            tx: None,
            event_tx,
            shutdown_tx: None,
        }
    }

    /// Connect to a session host.
    pub async fn connect(&mut self, addr: &str, session_id: &str) -> io::Result<()> {
        let stream = TcpStream::connect(addr).await?;
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Send join request
        let join = PeerMessage::Join {
            session_id: session_id.to_string(),
            peer_info: self.local_peer.clone(),
        };
        writer.write_all(join.to_json_line().unwrap().as_bytes()).await?;

        // Wait for ack
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let msg = PeerMessage::from_json(&line)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        match msg {
            PeerMessage::JoinAck {
                host_info,
                peers,
                document,
            } => {
                // Notify app
                let _ = self.event_tx.send(NetworkEvent::Connected(host_info)).await;
                let _ = self.event_tx.send(NetworkEvent::DocumentReceived(document)).await;
                for peer in peers {
                    let _ = self.event_tx.send(NetworkEvent::PeerConnected(peer)).await;
                }
            }
            PeerMessage::JoinReject { reason } => {
                let _ = self.event_tx.send(NetworkEvent::ConnectionFailed(reason.clone())).await;
                return Err(io::Error::new(io::ErrorKind::ConnectionRefused, reason));
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unexpected response",
                ));
            }
        }

        // Set up channels
        let (tx, mut rx) = mpsc::channel::<PeerMessage>(32);
        self.tx = Some(tx);

        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let event_tx = self.event_tx.clone();

        // Run connection loop
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_tx.subscribe();
            loop {
                line.clear();
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        match result {
                            Ok(0) => break,
                            Ok(_) => {
                                if let Ok(msg) = PeerMessage::from_json(&line) {
                                    match msg {
                                        PeerMessage::Operation { op } => {
                                            let _ = event_tx.send(NetworkEvent::OperationReceived(op)).await;
                                        }
                                        PeerMessage::Cursor { peer_id, position } => {
                                            let _ = event_tx.send(NetworkEvent::CursorUpdate { peer_id, position }).await;
                                        }
                                        PeerMessage::PeerJoined { peer } => {
                                            let _ = event_tx.send(NetworkEvent::PeerConnected(peer)).await;
                                        }
                                        PeerMessage::PeerLeft { peer_id } => {
                                            let _ = event_tx.send(NetworkEvent::PeerDisconnected(peer_id)).await;
                                        }
                                        PeerMessage::Pong => {}
                                        _ => {}
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    msg = rx.recv() => {
                        if let Some(msg) = msg {
                            if writer.write_all(msg.to_json_line().unwrap().as_bytes()).await.is_err() {
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        let goodbye = PeerMessage::Goodbye;
                        let _ = writer.write_all(goodbye.to_json_line().unwrap().as_bytes()).await;
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Disconnect from the session.
    pub fn disconnect(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.tx = None;
    }

    /// Send operation to host.
    pub async fn send_operation(&self, op: Operation) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(PeerMessage::Operation { op }).await;
        }
    }

    /// Send cursor update to host.
    pub async fn send_cursor(&self, position: usize) {
        if let Some(tx) = &self.tx {
            let _ = tx
                .send(PeerMessage::Cursor {
                    peer_id: self.local_peer.id.clone(),
                    position,
                })
                .await;
        }
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.tx.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_message_serialize() {
        let msg = PeerMessage::Ping;
        let json = msg.to_json_line().unwrap();
        assert!(json.ends_with('\n'));
        assert!(json.contains("Ping"));
    }

    #[test]
    fn test_peer_message_deserialize() {
        let json = r#"{"Ping":null}"#;
        let msg = PeerMessage::from_json(json).unwrap();
        assert!(matches!(msg, PeerMessage::Ping));
    }

    #[test]
    fn test_join_message() {
        let peer = PeerInfo {
            id: "test-id".to_string(),
            name: "Alice".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        let msg = PeerMessage::Join {
            session_id: "ABC".to_string(),
            peer_info: peer,
        };
        let json = msg.to_json_line().unwrap();
        let parsed = PeerMessage::from_json(&json).unwrap();
        match parsed {
            PeerMessage::Join { session_id, peer_info } => {
                assert_eq!(session_id, "ABC");
                assert_eq!(peer_info.name, "Alice");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_operation_message() {
        use super::super::crdt::OperationType;
        
        let op = Operation {
            op_type: OperationType::Insert,
            position: 10,
            content: "hello".to_string(),
            agent_id: 1,
            seq: 0,
        };
        let msg = PeerMessage::Operation { op };
        let json = msg.to_json_line().unwrap();
        let parsed = PeerMessage::from_json(&json).unwrap();
        match parsed {
            PeerMessage::Operation { op } => {
                assert_eq!(op.position, 10);
                assert_eq!(op.content, "hello");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_cursor_message() {
        let msg = PeerMessage::Cursor {
            peer_id: "peer-1".to_string(),
            position: 42,
        };
        let json = msg.to_json_line().unwrap();
        let parsed = PeerMessage::from_json(&json).unwrap();
        match parsed {
            PeerMessage::Cursor { peer_id, position } => {
                assert_eq!(peer_id, "peer-1");
                assert_eq!(position, 42);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_join_ack_message() {
        let host = PeerInfo {
            id: "host".to_string(),
            name: "Host".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        let msg = PeerMessage::JoinAck {
            host_info: host,
            peers: vec![],
            document: "Hello world".to_string(),
        };
        let json = msg.to_json_line().unwrap();
        let parsed = PeerMessage::from_json(&json).unwrap();
        match parsed {
            PeerMessage::JoinAck { host_info, document, .. } => {
                assert_eq!(host_info.name, "Host");
                assert_eq!(document, "Hello world");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_join_reject_message() {
        let msg = PeerMessage::JoinReject {
            reason: "Session full".to_string(),
        };
        let json = msg.to_json_line().unwrap();
        let parsed = PeerMessage::from_json(&json).unwrap();
        match parsed {
            PeerMessage::JoinReject { reason } => {
                assert_eq!(reason, "Session full");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_peer_joined_message() {
        let peer = PeerInfo {
            id: "new-peer".to_string(),
            name: "NewGuy".to_string(),
            cursor_pos: 0,
            color_index: 2,
            active: true,
        };
        let msg = PeerMessage::PeerJoined { peer };
        let json = msg.to_json_line().unwrap();
        let parsed = PeerMessage::from_json(&json).unwrap();
        match parsed {
            PeerMessage::PeerJoined { peer } => {
                assert_eq!(peer.name, "NewGuy");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_peer_left_message() {
        let msg = PeerMessage::PeerLeft {
            peer_id: "leaving".to_string(),
        };
        let json = msg.to_json_line().unwrap();
        let parsed = PeerMessage::from_json(&json).unwrap();
        match parsed {
            PeerMessage::PeerLeft { peer_id } => {
                assert_eq!(peer_id, "leaving");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[tokio::test]
    async fn test_server_creation() {
        let (event_tx, _) = mpsc::channel(32);
        let peer = PeerInfo {
            id: "host".to_string(),
            name: "Host".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        let server = NetworkServer::new("ABC".to_string(), peer, event_tx);
        assert_eq!(server.session_id, "ABC");
    }

    #[tokio::test]
    async fn test_server_document() {
        let (event_tx, _) = mpsc::channel(32);
        let peer = PeerInfo {
            id: "host".to_string(),
            name: "Host".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        let server = NetworkServer::new("ABC".to_string(), peer, event_tx);
        server.set_document("Hello world".to_string()).await;
        let doc = server.document.read().await;
        assert_eq!(*doc, "Hello world");
    }

    #[tokio::test]
    async fn test_client_creation() {
        let (event_tx, _) = mpsc::channel(32);
        let peer = PeerInfo {
            id: "client".to_string(),
            name: "Client".to_string(),
            cursor_pos: 0,
            color_index: 0,
            active: true,
        };
        let client = NetworkClient::new(peer, event_tx);
        assert!(!client.is_connected());
    }
}
