//! Git-based cloud sync for hollow documents.
//!
//! Provides sync functionality using git as the backend.
//! Users can sync their writing projects to any git remote.
//!
//! Note: Module is implemented but not yet wired into the main TUI. UI integration planned for v0.3.

#![allow(dead_code)]

use git2::{
    Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository, Signature, StatusOptions,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Sync configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Whether auto-sync is enabled.
    pub auto_sync: bool,
    /// Auto-sync interval in minutes.
    pub auto_sync_interval: u32,
    /// Remote name (default: "origin").
    pub remote_name: String,
    /// Branch name (default: "main").
    pub branch_name: String,
    /// Commit author name.
    pub author_name: String,
    /// Commit author email.
    pub author_email: String,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            auto_sync: false,
            auto_sync_interval: 5,
            remote_name: "origin".to_string(),
            branch_name: "main".to_string(),
            author_name: "Hollow Writer".to_string(),
            author_email: "hollow@local".to_string(),
        }
    }
}

/// Sync status for a project.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    /// Not a git repository.
    NotInitialized,
    /// Clean - no uncommitted changes.
    Clean,
    /// Has uncommitted changes.
    Modified { files: Vec<String> },
    /// Ahead of remote.
    Ahead { commits: usize },
    /// Behind remote.
    Behind { commits: usize },
    /// Diverged from remote.
    Diverged { ahead: usize, behind: usize },
    /// Has merge conflicts.
    Conflicts { files: Vec<String> },
    /// No remote configured.
    NoRemote,
}

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum ConflictResolution {
    /// Keep local version.
    KeepLocal,
    /// Keep remote version.
    KeepRemote,
    /// Keep both (creates conflict markers).
    KeepBoth,
}

/// Error type for sync operations.
#[derive(Debug)]
pub enum SyncError {
    /// Git error.
    Git(git2::Error),
    /// IO error.
    Io(std::io::Error),
    /// No remote configured.
    NoRemote,
    /// Conflicts exist.
    Conflicts(Vec<String>),
    /// Not a git repository.
    NotARepository,
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Git(e) => write!(f, "git error: {}", e),
            SyncError::Io(e) => write!(f, "io error: {}", e),
            SyncError::NoRemote => write!(f, "no remote configured"),
            SyncError::Conflicts(files) => write!(f, "conflicts in: {}", files.join(", ")),
            SyncError::NotARepository => write!(f, "not a git repository"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<git2::Error> for SyncError {
    fn from(e: git2::Error) -> Self {
        SyncError::Git(e)
    }
}

impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self {
        SyncError::Io(e)
    }
}

/// Git-based sync manager.
pub struct SyncManager {
    config: SyncConfig,
}

impl SyncManager {
    /// Create a new sync manager.
    pub fn new(config: SyncConfig) -> Self {
        Self { config }
    }

    /// Initialize a git repository in the given path.
    pub fn init(&self, path: &Path) -> Result<Repository, SyncError> {
        let repo = Repository::init(path)?;
        
        // Create initial .gitignore
        let gitignore_path = path.join(".gitignore");
        if !gitignore_path.exists() {
            std::fs::write(&gitignore_path, "*.swp\n*.swo\n.DS_Store\n")?;
        }
        
        Ok(repo)
    }

    /// Open an existing repository.
    pub fn open(&self, path: &Path) -> Result<Repository, SyncError> {
        Repository::open(path).map_err(|_| SyncError::NotARepository)
    }

    /// Get the sync status of a repository.
    pub fn status(&self, repo: &Repository) -> Result<SyncStatus, SyncError> {
        // Check for uncommitted changes
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts))?;

        let modified_files: Vec<String> = statuses
            .iter()
            .filter(|s| !s.status().is_ignored())
            .filter_map(|s| s.path().map(|p| p.to_string()))
            .collect();

        if !modified_files.is_empty() {
            // Check for conflicts
            let conflicts: Vec<String> = statuses
                .iter()
                .filter(|s| s.status().is_conflicted())
                .filter_map(|s| s.path().map(|p| p.to_string()))
                .collect();

            if !conflicts.is_empty() {
                return Ok(SyncStatus::Conflicts { files: conflicts });
            }

            return Ok(SyncStatus::Modified { files: modified_files });
        }

        // Check remote status
        let _remote = match repo.find_remote(&self.config.remote_name) {
            Ok(r) => r,
            Err(_) => return Ok(SyncStatus::NoRemote),
        };

        let remote_ref = format!("refs/remotes/{}/{}", self.config.remote_name, self.config.branch_name);
        let local_ref = format!("refs/heads/{}", self.config.branch_name);

        let remote_oid = match repo.refname_to_id(&remote_ref) {
            Ok(oid) => oid,
            Err(_) => return Ok(SyncStatus::NoRemote),
        };

        let local_oid = match repo.refname_to_id(&local_ref) {
            Ok(oid) => oid,
            Err(_) => return Ok(SyncStatus::Clean),
        };

        if local_oid == remote_oid {
            return Ok(SyncStatus::Clean);
        }

        // Count ahead/behind
        let (ahead, behind) = repo.graph_ahead_behind(local_oid, remote_oid)?;

        match (ahead, behind) {
            (0, 0) => Ok(SyncStatus::Clean),
            (a, 0) => Ok(SyncStatus::Ahead { commits: a }),
            (0, b) => Ok(SyncStatus::Behind { commits: b }),
            (a, b) => Ok(SyncStatus::Diverged { ahead: a, behind: b }),
        }
    }

    /// Stage and commit all changes.
    pub fn commit(&self, repo: &Repository, message: &str) -> Result<git2::Oid, SyncError> {
        let mut index = repo.index()?;
        
        // Add all changes
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
        
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;
        
        let sig = Signature::now(&self.config.author_name, &self.config.author_email)?;
        
        // Get parent commit if it exists
        let parent = match repo.head() {
            Ok(head) => {
                let oid = head.target().ok_or_else(|| {
                    SyncError::Git(git2::Error::from_str("HEAD has no target"))
                })?;
                Some(repo.find_commit(oid)?)
            }
            Err(_) => None,
        };
        
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        
        let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
        
        Ok(oid)
    }

    /// Fetch from remote.
    pub fn fetch(&self, repo: &Repository) -> Result<(), SyncError> {
        let mut remote = repo.find_remote(&self.config.remote_name)?;
        
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        });
        
        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        
        remote.fetch(
            &[&self.config.branch_name],
            Some(&mut fetch_opts),
            None,
        )?;
        
        Ok(())
    }

    /// Push to remote.
    pub fn push(&self, repo: &Repository) -> Result<(), SyncError> {
        let mut remote = repo.find_remote(&self.config.remote_name)?;
        
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        });
        
        let mut push_opts = PushOptions::new();
        push_opts.remote_callbacks(callbacks);
        
        let refspec = format!("refs/heads/{}:refs/heads/{}", 
            self.config.branch_name, 
            self.config.branch_name
        );
        
        remote.push(&[&refspec], Some(&mut push_opts))?;
        
        Ok(())
    }

    /// Pull from remote (fetch + merge).
    pub fn pull(&self, repo: &Repository) -> Result<(), SyncError> {
        // Fetch first
        self.fetch(repo)?;
        
        // Then merge
        let remote_ref = format!("refs/remotes/{}/{}", 
            self.config.remote_name, 
            self.config.branch_name
        );
        
        let fetch_head = repo.find_reference(&remote_ref)?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
        
        let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;
        
        if analysis.is_up_to_date() {
            return Ok(());
        }
        
        if analysis.is_fast_forward() {
            // Fast-forward merge
            let refname = format!("refs/heads/{}", self.config.branch_name);
            let mut reference = repo.find_reference(&refname)?;
            reference.set_target(fetch_commit.id(), "fast-forward")?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        } else if analysis.is_normal() {
            // Normal merge - might have conflicts
            let local_head = repo.head()?.peel_to_commit()?;
            let remote_commit = repo.find_commit(fetch_commit.id())?;
            
            let mut index = repo.merge_commits(&local_head, &remote_commit, None)?;
            
            if index.has_conflicts() {
                let conflicts: Vec<String> = index
                    .conflicts()?
                    .filter_map(|c| c.ok())
                    .filter_map(|c| {
                        c.our.map(|e| {
                            String::from_utf8_lossy(&e.path).to_string()
                        })
                    })
                    .collect();
                
                return Err(SyncError::Conflicts(conflicts));
            }
            
            // Commit the merge
            let tree_oid = index.write_tree_to(repo)?;
            let tree = repo.find_tree(tree_oid)?;
            let sig = Signature::now(&self.config.author_name, &self.config.author_email)?;
            
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Merge {} into {}", self.config.remote_name, self.config.branch_name),
                &tree,
                &[&local_head, &remote_commit],
            )?;
        }
        
        Ok(())
    }

    /// Sync (commit, pull, push).
    pub fn sync(&self, repo: &Repository, message: &str) -> Result<(), SyncError> {
        // Check status first
        let status = self.status(repo)?;
        
        // Commit any local changes
        if matches!(status, SyncStatus::Modified { .. }) {
            self.commit(repo, message)?;
        }
        
        // Pull (fetch + merge)
        self.pull(repo)?;
        
        // Push
        self.push(repo)?;
        
        Ok(())
    }

    /// Resolve a conflict file with the given strategy.
    pub fn resolve_conflict(
        &self,
        repo: &Repository,
        path: &Path,
        resolution: ConflictResolution,
    ) -> Result<(), SyncError> {
        let mut index = repo.index()?;
        
        // Collect conflict info first to avoid borrow issues
        let conflict_info: Option<(Option<git2::Oid>, Option<git2::Oid>)> = {
            let conflicts = index.conflicts()?;
            let mut found = None;
            
            for conflict in conflicts.flatten() {
                let conflict_path = conflict.our
                    .as_ref()
                    .or(conflict.their.as_ref())
                    .map(|e| PathBuf::from(String::from_utf8_lossy(&e.path).to_string()));
                
                if conflict_path.as_ref() == Some(&path.to_path_buf()) {
                    found = Some((
                        conflict.our.map(|e| e.id),
                        conflict.their.map(|e| e.id),
                    ));
                    break;
                }
            }
            found
        };
        
        if let Some((our_oid, their_oid)) = conflict_info {
            match resolution {
                ConflictResolution::KeepLocal => {
                    if let Some(oid) = our_oid {
                        let blob = repo.find_blob(oid)?;
                        std::fs::write(path, blob.content())?;
                    }
                }
                ConflictResolution::KeepRemote => {
                    if let Some(oid) = their_oid {
                        let blob = repo.find_blob(oid)?;
                        std::fs::write(path, blob.content())?;
                    }
                }
                ConflictResolution::KeepBoth => {
                    // Keep file as-is with conflict markers
                }
            }
            
            // Mark as resolved
            index.add_path(path)?;
            index.write()?;
        }
        
        Ok(())
    }

    /// Add a remote to the repository.
    pub fn add_remote(&self, repo: &Repository, url: &str) -> Result<(), SyncError> {
        repo.remote(&self.config.remote_name, url)?;
        Ok(())
    }

    /// Check if auto-sync is enabled.
    pub fn is_auto_sync_enabled(&self) -> bool {
        self.config.auto_sync
    }

    /// Get auto-sync interval in minutes.
    pub fn auto_sync_interval(&self) -> u32 {
        self.config.auto_sync_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sync_config_default() {
        let config = SyncConfig::default();
        assert!(!config.auto_sync);
        assert_eq!(config.auto_sync_interval, 5);
        assert_eq!(config.remote_name, "origin");
        assert_eq!(config.branch_name, "main");
    }

    #[test]
    fn test_init_repository() {
        let tmp = TempDir::new().unwrap();
        let manager = SyncManager::new(SyncConfig::default());
        
        let _repo = manager.init(tmp.path()).unwrap();
        assert!(tmp.path().join(".git").exists());
        assert!(tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn test_status_not_initialized() {
        let tmp = TempDir::new().unwrap();
        let manager = SyncManager::new(SyncConfig::default());
        
        let result = manager.open(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_status_clean_empty_repo() {
        let tmp = TempDir::new().unwrap();
        let manager = SyncManager::new(SyncConfig::default());
        let repo = manager.init(tmp.path()).unwrap();
        
        // Initial commit
        manager.commit(&repo, "initial").unwrap();
        
        let status = manager.status(&repo).unwrap();
        assert_eq!(status, SyncStatus::NoRemote);
    }

    #[test]
    fn test_status_modified() {
        let tmp = TempDir::new().unwrap();
        let manager = SyncManager::new(SyncConfig::default());
        let repo = manager.init(tmp.path()).unwrap();
        
        // Initial commit
        manager.commit(&repo, "initial").unwrap();
        
        // Create a new file
        std::fs::write(tmp.path().join("test.md"), "hello").unwrap();
        
        let status = manager.status(&repo).unwrap();
        match status {
            SyncStatus::Modified { files } => {
                assert!(files.contains(&"test.md".to_string()));
            }
            _ => panic!("Expected Modified status"),
        }
    }

    #[test]
    fn test_commit() {
        let tmp = TempDir::new().unwrap();
        let manager = SyncManager::new(SyncConfig::default());
        let repo = manager.init(tmp.path()).unwrap();
        
        // Create a file
        std::fs::write(tmp.path().join("test.md"), "hello").unwrap();
        
        // Commit
        let oid = manager.commit(&repo, "add test file").unwrap();
        assert!(!oid.is_zero());
        
        // Check status is now clean (or NoRemote)
        let status = manager.status(&repo).unwrap();
        assert!(matches!(status, SyncStatus::NoRemote | SyncStatus::Clean));
    }

    #[test]
    fn test_add_remote() {
        let tmp = TempDir::new().unwrap();
        let manager = SyncManager::new(SyncConfig::default());
        let repo = manager.init(tmp.path()).unwrap();
        
        manager.add_remote(&repo, "https://github.com/test/repo.git").unwrap();
        
        let remote = repo.find_remote("origin").unwrap();
        assert_eq!(remote.url().unwrap(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_conflict_resolution_enum() {
        assert_eq!(ConflictResolution::KeepLocal, ConflictResolution::KeepLocal);
        assert_ne!(ConflictResolution::KeepLocal, ConflictResolution::KeepRemote);
    }

    #[test]
    fn test_sync_error_display() {
        let err = SyncError::NoRemote;
        assert_eq!(format!("{}", err), "no remote configured");
        
        let err = SyncError::Conflicts(vec!["file1.md".to_string(), "file2.md".to_string()]);
        assert!(format!("{}", err).contains("file1.md"));
    }
}
