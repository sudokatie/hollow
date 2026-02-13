//! Project management for multiple documents
//!
//! A project is a collection of related documents with shared settings.
//! Projects are defined by a .hollow-project file (YAML format).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A project containing multiple documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Project name
    pub name: String,
    /// Documents in the project (relative paths)
    pub documents: Vec<String>,
    /// Project-specific settings (override global config)
    #[serde(default)]
    pub settings: ProjectSettings,
    /// Path to the project file (not serialized)
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

/// Project-specific settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSettings {
    /// Daily word count goal for the project
    pub daily_goal: Option<u32>,
    /// Whether to show progress bar
    pub show_progress: Option<bool>,
    /// Whether to show streak counter
    pub show_streak: Option<bool>,
    /// Custom theme for this project
    pub theme: Option<String>,
}

/// Project-wide statistics
#[derive(Debug, Clone, Default)]
pub struct ProjectStats {
    /// Total word count across all documents
    pub total_words: u64,
    /// Document count
    pub document_count: usize,
    /// Per-document word counts
    pub document_words: Vec<(String, u64)>,
}

impl Project {
    /// Create a new empty project
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            documents: Vec::new(),
            settings: ProjectSettings::default(),
            path: None,
        }
    }

    /// Load a project from a .hollow-project file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| ProjectError::Io(e.to_string()))?;
        
        let mut project: Project = serde_yaml::from_str(&content)
            .map_err(|e| ProjectError::Parse(e.to_string()))?;
        
        project.path = Some(path.to_path_buf());
        Ok(project)
    }

    /// Save the project to its file (or a new path)
    pub fn save(&self, path: Option<&Path>) -> Result<(), ProjectError> {
        let path = path
            .or(self.path.as_deref())
            .ok_or(ProjectError::NoPath)?;
        
        let content = serde_yaml::to_string(self)
            .map_err(|e| ProjectError::Serialize(e.to_string()))?;
        
        fs::write(path, content)
            .map_err(|e| ProjectError::Io(e.to_string()))?;
        
        Ok(())
    }

    /// Add a document to the project
    pub fn add_document(&mut self, path: impl Into<String>) {
        let path = path.into();
        if !self.documents.contains(&path) {
            self.documents.push(path);
        }
    }

    /// Remove a document from the project
    pub fn remove_document(&mut self, path: &str) {
        self.documents.retain(|p| p != path);
    }

    /// Get the directory containing the project file
    pub fn base_dir(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent().map(PathBuf::from))
    }

    /// Resolve a document path relative to the project
    pub fn resolve_document(&self, doc: &str) -> Option<PathBuf> {
        self.base_dir().map(|base| base.join(doc))
    }

    /// Calculate project statistics
    pub fn stats(&self) -> Result<ProjectStats, ProjectError> {
        let base = self.base_dir().ok_or(ProjectError::NoPath)?;
        let mut stats = ProjectStats {
            document_count: self.documents.len(),
            ..Default::default()
        };

        for doc in &self.documents {
            let path = base.join(doc);
            if let Ok(content) = fs::read_to_string(&path) {
                let words = count_words(&content);
                stats.total_words += words;
                stats.document_words.push((doc.clone(), words));
            }
        }

        Ok(stats)
    }
}

/// Count words in text
fn count_words(text: &str) -> u64 {
    text.split_whitespace().count() as u64
}

/// Project-related errors
#[derive(Debug)]
pub enum ProjectError {
    /// IO error
    Io(String),
    /// Parse error
    Parse(String),
    /// Serialization error
    Serialize(String),
    /// No path set for project
    NoPath,
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Parse(e) => write!(f, "Parse error: {}", e),
            Self::Serialize(e) => write!(f, "Serialize error: {}", e),
            Self::NoPath => write!(f, "No project path set"),
        }
    }
}

impl std::error::Error for ProjectError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_project() {
        let project = Project::new("My Novel");
        assert_eq!(project.name, "My Novel");
        assert!(project.documents.is_empty());
    }

    #[test]
    fn test_add_document() {
        let mut project = Project::new("Test");
        project.add_document("chapter1.md");
        project.add_document("chapter2.md");
        project.add_document("chapter1.md"); // duplicate
        
        assert_eq!(project.documents.len(), 2);
    }

    #[test]
    fn test_remove_document() {
        let mut project = Project::new("Test");
        project.add_document("a.md");
        project.add_document("b.md");
        project.remove_document("a.md");
        
        assert_eq!(project.documents, vec!["b.md"]);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".hollow-project");
        
        let mut project = Project::new("Test Project");
        project.add_document("chapter1.md");
        project.settings.daily_goal = Some(1000);
        
        project.save(Some(&path)).unwrap();
        
        let loaded = Project::load(&path).unwrap();
        assert_eq!(loaded.name, "Test Project");
        assert_eq!(loaded.documents, vec!["chapter1.md"]);
        assert_eq!(loaded.settings.daily_goal, Some(1000));
    }

    #[test]
    fn test_resolve_document() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".hollow-project");
        
        let mut project = Project::new("Test");
        project.path = Some(path);
        
        let resolved = project.resolve_document("chapter1.md").unwrap();
        assert!(resolved.ends_with("chapter1.md"));
        assert!(resolved.starts_with(dir.path()));
    }

    #[test]
    fn test_project_stats() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".hollow-project");
        
        // Create test documents
        fs::write(dir.path().join("ch1.md"), "one two three").unwrap();
        fs::write(dir.path().join("ch2.md"), "four five").unwrap();
        
        let mut project = Project::new("Test");
        project.path = Some(path);
        project.add_document("ch1.md");
        project.add_document("ch2.md");
        
        let stats = project.stats().unwrap();
        assert_eq!(stats.total_words, 5);
        assert_eq!(stats.document_count, 2);
    }

    #[test]
    fn test_count_words() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("one  two   three"), 3);
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("   "), 0);
    }
}
