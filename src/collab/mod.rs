//! Collaborative editing module.
//!
//! Provides real-time collaboration via CRDTs (Conflict-free Replicated Data Types).
//! No central server required - peers connect directly and sync operations.

mod crdt;
mod network;
mod presence;
mod session;
mod sync;

pub use crdt::{CollabDocument, Operation, OperationType};
pub use network::{NetworkClient, NetworkEvent, NetworkServer, PeerMessage, DEFAULT_PORT};
pub use presence::{
    CursorColor, CursorPosition, PeerCursor, PresenceManager, Selection, PEER_COLORS,
};
pub use session::{CollabSession, PeerInfo, SessionConfig, SessionState};
pub use sync::{CollabMode, CollabSync, OpBatcher};
