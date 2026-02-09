//! Version history tracking for documents
//!
//! Stores document versions in SQLite database at ~/.config/hollow/versions.db
//! Content is compressed with DEFLATE to minimize storage.

use chrono::{DateTime, Local};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use rusqlite::{Connection, Result as SqlResult};
use std::io::{Read, Write};
use std::path::PathBuf;

/// A single document version
#[derive(Debug, Clone)]
pub struct Version {
    pub id: i64,
    pub file_path: String,
    pub created_at: DateTime<Local>,
    pub content: String,
    pub word_count: usize,
}

impl Version {
    /// Get a preview snippet of the content (first 50 chars)
    pub fn preview(&self) -> String {
        let preview: String = self
            .content
            .chars()
            .take(50)
            .map(|c| if c == '\n' { ' ' } else { c })
            .collect();
        if self.content.len() > 50 {
            format!("{}...", preview.trim())
        } else {
            preview.trim().to_string()
        }
    }

    /// Format the creation time for display
    pub fn formatted_time(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// Version store with SQLite persistence
pub struct VersionStore {
    conn: Connection,
    max_versions: usize,
}

impl VersionStore {
    /// Get the database path
    fn db_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hollow")
            .join("versions.db")
    }

    /// Create a new version store
    pub fn new(max_versions: usize) -> SqlResult<Self> {
        let db_path = Self::db_path();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(&db_path)?;

        // Create versions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                content_compressed BLOB NOT NULL,
                word_count INTEGER NOT NULL
            )",
            [],
        )?;

        // Create index on file_path and created_at
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_versions_file_time 
             ON versions (file_path, created_at DESC)",
            [],
        )?;

        Ok(Self { conn, max_versions })
    }

    /// Compress content using DEFLATE
    fn compress(content: &str) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(content.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    /// Decompress content from DEFLATE
    fn decompress(data: &[u8]) -> String {
        let mut decoder = DeflateDecoder::new(data);
        let mut result = String::new();
        decoder.read_to_string(&mut result).unwrap_or_default();
        result
    }

    /// Count words in content
    fn count_words(content: &str) -> usize {
        content.split_whitespace().count()
    }

    /// Save a new version
    pub fn save_version(&self, file_path: &str, content: &str) -> SqlResult<i64> {
        let compressed = Self::compress(content);
        let word_count = Self::count_words(content);
        let timestamp = Local::now().timestamp_millis();

        self.conn.execute(
            "INSERT INTO versions (file_path, created_at, content_compressed, word_count)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![file_path, timestamp, compressed, word_count as i64],
        )?;

        let id = self.conn.last_insert_rowid();

        // Prune old versions
        self.prune_old_versions(file_path)?;

        Ok(id)
    }

    /// Check if content differs from last saved version
    pub fn content_differs(&self, file_path: &str, content: &str) -> SqlResult<bool> {
        let last_content: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT content_compressed FROM versions 
                 WHERE file_path = ?1 
                 ORDER BY created_at DESC LIMIT 1",
                [file_path],
                |row| row.get(0),
            )
            .ok();

        match last_content {
            Some(compressed) => {
                let last = Self::decompress(&compressed);
                Ok(last != content)
            }
            None => Ok(true), // No previous version, so it differs
        }
    }

    /// Get all versions for a file (newest first)
    pub fn get_versions(&self, file_path: &str) -> SqlResult<Vec<Version>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, created_at, content_compressed, word_count
             FROM versions
             WHERE file_path = ?1
             ORDER BY created_at DESC",
        )?;

        let versions = stmt
            .query_map([file_path], |row| {
                let id: i64 = row.get(0)?;
                let file_path: String = row.get(1)?;
                let timestamp: i64 = row.get(2)?;
                let compressed: Vec<u8> = row.get(3)?;
                let word_count: i64 = row.get(4)?;

                let content = Self::decompress(&compressed);
                let created_at = chrono::DateTime::from_timestamp_millis(timestamp)
                    .map(|dt| dt.with_timezone(&Local))
                    .unwrap_or_else(Local::now);

                Ok(Version {
                    id,
                    file_path,
                    created_at,
                    content,
                    word_count: word_count as usize,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(versions)
    }

    /// Get a specific version by ID
    pub fn get_version(&self, id: i64) -> SqlResult<Option<Version>> {
        let result = self.conn.query_row(
            "SELECT id, file_path, created_at, content_compressed, word_count
             FROM versions WHERE id = ?1",
            [id],
            |row| {
                let id: i64 = row.get(0)?;
                let file_path: String = row.get(1)?;
                let timestamp: i64 = row.get(2)?;
                let compressed: Vec<u8> = row.get(3)?;
                let word_count: i64 = row.get(4)?;

                let content = Self::decompress(&compressed);
                let created_at = chrono::DateTime::from_timestamp_millis(timestamp)
                    .map(|dt| dt.with_timezone(&Local))
                    .unwrap_or_else(Local::now);

                Ok(Version {
                    id,
                    file_path,
                    created_at,
                    content,
                    word_count: word_count as usize,
                })
            },
        );

        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get version count for a file
    pub fn version_count(&self, file_path: &str) -> SqlResult<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM versions WHERE file_path = ?1",
            [file_path],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Prune old versions beyond the limit
    fn prune_old_versions(&self, file_path: &str) -> SqlResult<()> {
        let count = self.version_count(file_path)?;
        if count > self.max_versions {
            let to_delete = count - self.max_versions;
            self.conn.execute(
                "DELETE FROM versions WHERE id IN (
                    SELECT id FROM versions 
                    WHERE file_path = ?1 
                    ORDER BY created_at ASC 
                    LIMIT ?2
                )",
                rusqlite::params![file_path, to_delete as i64],
            )?;
        }
        Ok(())
    }

    /// Generate a unified diff between two strings
    pub fn diff(old: &str, new: &str) -> String {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let mut result = String::new();
        let mut old_idx = 0;
        let mut new_idx = 0;

        // Simple line-by-line diff (not optimal but works)
        while old_idx < old_lines.len() || new_idx < new_lines.len() {
            if old_idx >= old_lines.len() {
                // Remaining new lines are additions
                result.push_str(&format!("+ {}\n", new_lines[new_idx]));
                new_idx += 1;
            } else if new_idx >= new_lines.len() {
                // Remaining old lines are deletions
                result.push_str(&format!("- {}\n", old_lines[old_idx]));
                old_idx += 1;
            } else if old_lines[old_idx] == new_lines[new_idx] {
                // Lines match
                result.push_str(&format!("  {}\n", old_lines[old_idx]));
                old_idx += 1;
                new_idx += 1;
            } else {
                // Lines differ - show as delete + add
                result.push_str(&format!("- {}\n", old_lines[old_idx]));
                result.push_str(&format!("+ {}\n", new_lines[new_idx]));
                old_idx += 1;
                new_idx += 1;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_store() -> (VersionStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("versions.db");
        let conn = Connection::open(&db_path).unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                content_compressed BLOB NOT NULL,
                word_count INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();

        let store = VersionStore {
            conn,
            max_versions: 10,
        };
        (store, temp_dir)
    }

    #[test]
    fn test_compress_decompress() {
        let original = "Hello, World! This is a test of compression.";
        let compressed = VersionStore::compress(original);
        let decompressed = VersionStore::decompress(&compressed);
        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_compress_large_text() {
        let original = "Lorem ipsum dolor sit amet. ".repeat(1000);
        let compressed = VersionStore::compress(&original);
        let decompressed = VersionStore::decompress(&compressed);
        assert_eq!(original, decompressed);
        // Compression should reduce size significantly for repeated text
        assert!(compressed.len() < original.len() / 10);
    }

    #[test]
    fn test_save_and_get_version() {
        let (store, _temp) = setup_test_store();
        let content = "This is test content.";
        let file_path = "/test/file.md";

        let id = store.save_version(file_path, content).unwrap();
        assert!(id > 0);

        let version = store.get_version(id).unwrap().unwrap();
        assert_eq!(version.content, content);
        assert_eq!(version.file_path, file_path);
        assert_eq!(version.word_count, 4);
    }

    #[test]
    fn test_get_versions_ordered() {
        let (store, _temp) = setup_test_store();
        let file_path = "/test/file.md";

        store.save_version(file_path, "First version").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store.save_version(file_path, "Second version").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store.save_version(file_path, "Third version").unwrap();

        let versions = store.get_versions(file_path).unwrap();
        assert_eq!(versions.len(), 3);
        // Newest first
        assert_eq!(versions[0].content, "Third version");
        assert_eq!(versions[1].content, "Second version");
        assert_eq!(versions[2].content, "First version");
    }

    #[test]
    fn test_content_differs() {
        let (store, _temp) = setup_test_store();
        let file_path = "/test/file.md";

        // No previous version, should return true
        assert!(store.content_differs(file_path, "New content").unwrap());

        // Save a version
        store.save_version(file_path, "Original content").unwrap();

        // Same content should return false
        assert!(!store.content_differs(file_path, "Original content").unwrap());

        // Different content should return true
        assert!(store.content_differs(file_path, "Modified content").unwrap());
    }

    #[test]
    fn test_prune_old_versions() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("versions.db");
        let conn = Connection::open(&db_path).unwrap();

        conn.execute(
            "CREATE TABLE versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                content_compressed BLOB NOT NULL,
                word_count INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();

        let store = VersionStore {
            conn,
            max_versions: 3,
        };

        let file_path = "/test/file.md";

        // Save 5 versions
        for i in 1..=5 {
            store
                .save_version(file_path, &format!("Version {}", i))
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Should only have 3 versions (the newest)
        let count = store.version_count(file_path).unwrap();
        assert_eq!(count, 3);

        let versions = store.get_versions(file_path).unwrap();
        assert_eq!(versions[0].content, "Version 5");
        assert_eq!(versions[1].content, "Version 4");
        assert_eq!(versions[2].content, "Version 3");
    }

    #[test]
    fn test_version_preview() {
        let version = Version {
            id: 1,
            file_path: "/test/file.md".to_string(),
            created_at: Local::now(),
            content: "This is a short preview text.".to_string(),
            word_count: 6,
        };
        assert_eq!(version.preview(), "This is a short preview text.");

        let long_version = Version {
            id: 2,
            file_path: "/test/file.md".to_string(),
            created_at: Local::now(),
            content: "This is a much longer piece of content that exceeds fifty characters and needs truncation.".to_string(),
            word_count: 15,
        };
        let preview = long_version.preview();
        assert!(preview.ends_with("..."));
        assert!(preview.len() <= 55); // 50 chars + "..."
    }

    #[test]
    fn test_diff_additions() {
        let old = "line 1\nline 2";
        let new = "line 1\nline 2\nline 3";
        let diff = VersionStore::diff(old, new);
        assert!(diff.contains("+ line 3"));
    }

    #[test]
    fn test_diff_deletions() {
        let old = "line 1\nline 2\nline 3";
        let new = "line 1\nline 2";
        let diff = VersionStore::diff(old, new);
        assert!(diff.contains("- line 3"));
    }

    #[test]
    fn test_diff_modifications() {
        let old = "line 1\nold line\nline 3";
        let new = "line 1\nnew line\nline 3";
        let diff = VersionStore::diff(old, new);
        assert!(diff.contains("- old line"));
        assert!(diff.contains("+ new line"));
    }

    #[test]
    fn test_word_count() {
        assert_eq!(VersionStore::count_words(""), 0);
        assert_eq!(VersionStore::count_words("hello"), 1);
        assert_eq!(VersionStore::count_words("hello world"), 2);
        assert_eq!(VersionStore::count_words("  multiple   spaces  "), 2);
        assert_eq!(VersionStore::count_words("line1\nline2\nline3"), 3);
    }
}
