//! Collaborative editing module.
//!
//! Provides real-time collaboration via CRDTs (Conflict-free Replicated Data Types).
//! No central server required - peers connect directly and sync operations.

mod crdt;
mod network;
mod session;

pub use crdt::{CollabDocument, Operation, OperationType};
pub use network::{NetworkClient, NetworkEvent, NetworkServer, PeerMessage, DEFAULT_PORT};
pub use session::{CollabSession, PeerInfo, SessionConfig, SessionState};
