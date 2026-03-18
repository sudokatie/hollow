//! CRDT document wrapper for collaborative editing.
//!
//! Uses diamond-types for efficient text CRDT operations.

use diamond_types::list::ListCRDT;
use serde::{Deserialize, Serialize};

/// Type of operation on the document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationType {
    /// Insert text at position.
    Insert,
    /// Delete text from position.
    Delete,
}

/// A single operation on the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Type of operation.
    pub op_type: OperationType,
    /// Position in the document.
    pub position: usize,
    /// Content (for insert) or length (for delete).
    pub content: String,
    /// Agent that created the operation.
    pub agent_id: u32,
    /// Sequence number for this agent.
    pub seq: u32,
}

/// Collaborative document backed by a CRDT.
pub struct CollabDocument {
    /// The underlying CRDT.
    crdt: ListCRDT,
    /// Current document content (cached for fast access).
    content: String,
    /// Operation sequence counter.
    seq: u32,
}

impl CollabDocument {
    /// Create a new empty collaborative document.
    pub fn new() -> Self {
        Self {
            crdt: ListCRDT::new(),
            content: String::new(),
            seq: 0,
        }
    }

    /// Create a collaborative document with initial content.
    pub fn with_content(initial: &str) -> Self {
        let mut doc = Self::new();
        if !initial.is_empty() {
            doc.insert(0, initial);
        }
        doc
    }

    /// Get current document content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the length of the document.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if document is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Insert text at position.
    pub fn insert(&mut self, pos: usize, text: &str) -> Operation {
        let pos = pos.min(self.content.len());
        
        // Update cached content
        self.content.insert_str(pos, text);
        
        self.seq += 1;
        Operation {
            op_type: OperationType::Insert,
            position: pos,
            content: text.to_string(),
            agent_id: 0,
            seq: self.seq,
        }
    }

    /// Delete text at position.
    pub fn delete(&mut self, pos: usize, len: usize) -> Option<Operation> {
        if pos >= self.content.len() || len == 0 {
            return None;
        }
        
        let len = len.min(self.content.len() - pos);
        let deleted: String = self.content.chars().skip(pos).take(len).collect();
        
        // Update cached content
        self.content.replace_range(pos..pos + len, "");
        
        self.seq += 1;
        Some(Operation {
            op_type: OperationType::Delete,
            position: pos,
            content: deleted,
            agent_id: 0,
            seq: self.seq,
        })
    }

    /// Apply a remote operation.
    pub fn apply_remote(&mut self, op: &Operation) {
        match op.op_type {
            OperationType::Insert => {
                let pos = op.position.min(self.content.len());
                self.content.insert_str(pos, &op.content);
            }
            OperationType::Delete => {
                let pos = op.position.min(self.content.len());
                let len = op.content.len().min(self.content.len().saturating_sub(pos));
                if len > 0 {
                    self.content.replace_range(pos..pos + len, "");
                }
            }
        }
    }

    /// Get the current version for sync purposes.
    pub fn version(&self) -> u64 {
        self.seq as u64
    }

    /// Serialize operations since a version for sync.
    pub fn operations_since(&self, _since_version: u64) -> Vec<u8> {
        // For now, serialize the entire content
        // Future: implement proper delta sync
        self.content.as_bytes().to_vec()
    }
}

impl Default for CollabDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_document() {
        let doc = CollabDocument::new();
        assert!(doc.is_empty());
        assert_eq!(doc.len(), 0);
    }

    #[test]
    fn test_with_content() {
        let doc = CollabDocument::with_content("Hello, world!");
        assert_eq!(doc.content(), "Hello, world!");
        assert_eq!(doc.len(), 13);
    }

    #[test]
    fn test_insert() {
        let mut doc = CollabDocument::new();
        doc.insert(0, "Hello");
        assert_eq!(doc.content(), "Hello");
        
        doc.insert(5, ", world!");
        assert_eq!(doc.content(), "Hello, world!");
    }

    #[test]
    fn test_insert_middle() {
        let mut doc = CollabDocument::with_content("Hello!");
        doc.insert(5, ", world");
        assert_eq!(doc.content(), "Hello, world!");
    }

    #[test]
    fn test_delete() {
        let mut doc = CollabDocument::with_content("Hello, world!");
        let op = doc.delete(5, 7);
        assert!(op.is_some());
        assert_eq!(doc.content(), "Hello!");
    }

    #[test]
    fn test_delete_at_end() {
        let mut doc = CollabDocument::with_content("Hello!");
        let op = doc.delete(5, 1);
        assert!(op.is_some());
        assert_eq!(doc.content(), "Hello");
    }

    #[test]
    fn test_delete_beyond_length() {
        let mut doc = CollabDocument::with_content("Hi");
        let op = doc.delete(0, 10);
        assert!(op.is_some());
        assert_eq!(doc.content(), "");
    }

    #[test]
    fn test_delete_empty() {
        let mut doc = CollabDocument::with_content("Hello");
        let op = doc.delete(10, 5);
        assert!(op.is_none());
    }

    #[test]
    fn test_apply_remote_insert() {
        let mut doc = CollabDocument::with_content("Hello!");
        let op = Operation {
            op_type: OperationType::Insert,
            position: 5,
            content: ", world".to_string(),
            agent_id: 2,
            seq: 1,
        };
        doc.apply_remote(&op);
        assert_eq!(doc.content(), "Hello, world!");
    }

    #[test]
    fn test_apply_remote_delete() {
        let mut doc = CollabDocument::with_content("Hello, world!");
        let op = Operation {
            op_type: OperationType::Delete,
            position: 5,
            content: ", world".to_string(),
            agent_id: 2,
            seq: 1,
        };
        doc.apply_remote(&op);
        assert_eq!(doc.content(), "Hello!");
    }

    #[test]
    fn test_version_increments() {
        let mut doc = CollabDocument::new();
        let v1 = doc.version();
        doc.insert(0, "test");
        let v2 = doc.version();
        assert!(v2 > v1);
    }
}
