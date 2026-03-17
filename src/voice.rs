//! Voice dictation and command support.
//!
//! Provides speech-to-text transcription using OpenAI Whisper CLI
//! and voice command parsing for hands-free navigation.

#![allow(dead_code)] // Module scaffolded for future TUI integration

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Voice module configuration.
#[derive(Debug, Clone)]
pub struct VoiceConfig {
    /// Path to whisper CLI executable.
    pub whisper_path: String,
    /// Whisper model to use (tiny, base, small, medium, large).
    pub model: WhisperModel,
    /// Language for transcription (or "auto" for detection).
    pub language: String,
    /// Temporary directory for audio files.
    pub temp_dir: PathBuf,
    /// Recording duration limit in seconds (0 = unlimited).
    pub max_duration: u32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            whisper_path: "whisper".to_string(),
            model: WhisperModel::Base,
            language: "en".to_string(),
            temp_dir: std::env::temp_dir(),
            max_duration: 60,
        }
    }
}

/// Whisper model sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

impl WhisperModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Base => "base",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

impl std::fmt::Display for WhisperModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Voice command types for navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceCommand {
    /// Insert transcribed text at cursor.
    Insert(String),
    /// Navigation commands.
    GoToLine(u32),
    GoToStart,
    GoToEnd,
    NextParagraph,
    PrevParagraph,
    /// Editing commands.
    DeleteLine,
    DeleteWord,
    Undo,
    Redo,
    /// Mode commands.
    Save,
    Quit,
    Help,
    /// Unknown or unrecognized command.
    Unknown(String),
}

impl VoiceCommand {
    /// Convert voice command to input Action for execution.
    pub fn to_action(&self) -> Option<crate::input::Action> {
        use crate::editor::{Direction, Unit};
        use crate::input::Action;
        
        match self {
            VoiceCommand::GoToStart => Some(Action::MoveCursor(Direction::Up, Unit::Document)),
            VoiceCommand::GoToEnd => Some(Action::MoveCursor(Direction::Down, Unit::Document)),
            VoiceCommand::NextParagraph => Some(Action::MoveCursor(Direction::Down, Unit::Paragraph)),
            VoiceCommand::PrevParagraph => Some(Action::MoveCursor(Direction::Up, Unit::Paragraph)),
            VoiceCommand::DeleteLine => Some(Action::DeleteLine),
            VoiceCommand::Undo => Some(Action::Undo),
            VoiceCommand::Redo => Some(Action::Redo),
            VoiceCommand::Save => Some(Action::Save),
            VoiceCommand::Quit => Some(Action::Quit),
            VoiceCommand::Help => Some(Action::ShowHelp),
            // These need special handling in the app
            VoiceCommand::GoToLine(_) => None,
            VoiceCommand::Insert(_) => None,
            VoiceCommand::DeleteWord => None, // Not directly mapped
            VoiceCommand::Unknown(_) => None,
        }
    }
}

/// Errors from voice operations.
#[derive(Debug)]
pub enum VoiceError {
    /// Whisper CLI not found.
    WhisperNotFound,
    /// Recording failed.
    RecordingFailed(String),
    /// Transcription failed.
    TranscriptionFailed(String),
    /// IO error.
    Io(io::Error),
}

impl std::fmt::Display for VoiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WhisperNotFound => write!(f, "Whisper CLI not found in PATH"),
            Self::RecordingFailed(msg) => write!(f, "Recording failed: {}", msg),
            Self::TranscriptionFailed(msg) => write!(f, "Transcription failed: {}", msg),
            Self::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for VoiceError {}

impl From<io::Error> for VoiceError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Voice transcription manager.
pub struct VoiceManager {
    config: VoiceConfig,
}

impl VoiceManager {
    /// Create a new voice manager with the given config.
    pub fn new(config: VoiceConfig) -> Self {
        Self { config }
    }

    /// Check if Whisper CLI is available.
    pub fn is_available(&self) -> bool {
        Command::new(&self.config.whisper_path)
            .arg("--help")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Transcribe an audio file using Whisper.
    pub fn transcribe(&self, audio_path: &Path) -> Result<String, VoiceError> {
        if !self.is_available() {
            return Err(VoiceError::WhisperNotFound);
        }

        let output = Command::new(&self.config.whisper_path)
            .arg(audio_path)
            .arg("--model")
            .arg(self.config.model.as_str())
            .arg("--language")
            .arg(&self.config.language)
            .arg("--output_format")
            .arg("txt")
            .arg("--output_dir")
            .arg(&self.config.temp_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VoiceError::TranscriptionFailed(stderr.to_string()));
        }

        // Read the output text file
        let stem = audio_path.file_stem().unwrap_or_default();
        let txt_path = self.config.temp_dir.join(format!("{}.txt", stem.to_string_lossy()));
        
        std::fs::read_to_string(&txt_path)
            .map(|s| s.trim().to_string())
            .map_err(|e| VoiceError::TranscriptionFailed(e.to_string()))
    }

    /// Parse transcribed text into a voice command.
    pub fn parse_command(&self, text: &str) -> VoiceCommand {
        let lower = text.to_lowercase().trim().to_string();
        
        // Check for navigation commands
        if let Some(rest) = lower.strip_prefix("go to line ") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                return VoiceCommand::GoToLine(n);
            }
        }
        
        match lower.as_str() {
            "go to start" | "beginning" | "start" => VoiceCommand::GoToStart,
            "go to end" | "end" => VoiceCommand::GoToEnd,
            "next paragraph" | "next para" => VoiceCommand::NextParagraph,
            "previous paragraph" | "prev paragraph" | "prev para" => VoiceCommand::PrevParagraph,
            "delete line" | "remove line" => VoiceCommand::DeleteLine,
            "delete word" | "remove word" => VoiceCommand::DeleteWord,
            "undo" => VoiceCommand::Undo,
            "redo" => VoiceCommand::Redo,
            "save" | "save file" => VoiceCommand::Save,
            "quit" | "exit" | "close" => VoiceCommand::Quit,
            "help" => VoiceCommand::Help,
            _ => {
                // If not a command, treat as text to insert
                if !text.trim().is_empty() {
                    VoiceCommand::Insert(text.trim().to_string())
                } else {
                    VoiceCommand::Unknown(text.to_string())
                }
            }
        }
    }

    /// Check if SoX recording is available.
    pub fn is_recording_available(&self) -> bool {
        Command::new("rec")
            .arg("--help")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Record audio to a file using SoX.
    /// Returns the path to the recorded file.
    pub fn record(&self, duration_secs: u32) -> Result<PathBuf, VoiceError> {
        let output_path = self.config.temp_dir.join("hollow_voice.wav");
        
        // Use SoX's rec command
        let mut cmd = Command::new("rec");
        cmd.arg("-q") // Quiet
            .arg(&output_path)
            .arg("rate")
            .arg("16k") // 16kHz for Whisper
            .arg("channels")
            .arg("1"); // Mono
        
        // Add duration limit if specified
        if duration_secs > 0 {
            cmd.arg("trim")
                .arg("0")
                .arg(duration_secs.to_string());
        }
        
        let output = cmd.output()?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VoiceError::RecordingFailed(stderr.to_string()));
        }
        
        Ok(output_path)
    }

    /// Record audio, transcribe it, and parse as a command.
    /// This is the main entry point for voice dictation.
    pub fn listen_and_transcribe(&self, duration_secs: u32) -> Result<VoiceCommand, VoiceError> {
        let audio_path = self.record(duration_secs)?;
        let text = self.transcribe(&audio_path)?;
        
        // Clean up audio file
        let _ = std::fs::remove_file(&audio_path);
        
        Ok(self.parse_command(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_config_default() {
        let config = VoiceConfig::default();
        assert_eq!(config.whisper_path, "whisper");
        assert_eq!(config.model, WhisperModel::Base);
        assert_eq!(config.language, "en");
        assert_eq!(config.max_duration, 60);
    }

    #[test]
    fn test_whisper_model_display() {
        assert_eq!(WhisperModel::Tiny.as_str(), "tiny");
        assert_eq!(WhisperModel::Base.as_str(), "base");
        assert_eq!(WhisperModel::Small.as_str(), "small");
        assert_eq!(WhisperModel::Medium.as_str(), "medium");
        assert_eq!(WhisperModel::Large.as_str(), "large");
    }

    #[test]
    fn test_parse_command_navigation() {
        let manager = VoiceManager::new(VoiceConfig::default());
        
        assert_eq!(manager.parse_command("go to line 42"), VoiceCommand::GoToLine(42));
        assert_eq!(manager.parse_command("go to start"), VoiceCommand::GoToStart);
        assert_eq!(manager.parse_command("beginning"), VoiceCommand::GoToStart);
        assert_eq!(manager.parse_command("go to end"), VoiceCommand::GoToEnd);
        assert_eq!(manager.parse_command("end"), VoiceCommand::GoToEnd);
    }

    #[test]
    fn test_parse_command_editing() {
        let manager = VoiceManager::new(VoiceConfig::default());
        
        assert_eq!(manager.parse_command("delete line"), VoiceCommand::DeleteLine);
        assert_eq!(manager.parse_command("delete word"), VoiceCommand::DeleteWord);
        assert_eq!(manager.parse_command("undo"), VoiceCommand::Undo);
        assert_eq!(manager.parse_command("redo"), VoiceCommand::Redo);
    }

    #[test]
    fn test_parse_command_mode() {
        let manager = VoiceManager::new(VoiceConfig::default());
        
        assert_eq!(manager.parse_command("save"), VoiceCommand::Save);
        assert_eq!(manager.parse_command("quit"), VoiceCommand::Quit);
        assert_eq!(manager.parse_command("help"), VoiceCommand::Help);
    }

    #[test]
    fn test_parse_command_insert() {
        let manager = VoiceManager::new(VoiceConfig::default());
        
        assert_eq!(
            manager.parse_command("Hello, this is my text"),
            VoiceCommand::Insert("Hello, this is my text".to_string())
        );
    }

    #[test]
    fn test_parse_command_case_insensitive() {
        let manager = VoiceManager::new(VoiceConfig::default());
        
        assert_eq!(manager.parse_command("UNDO"), VoiceCommand::Undo);
        assert_eq!(manager.parse_command("Save"), VoiceCommand::Save);
        assert_eq!(manager.parse_command("GO TO END"), VoiceCommand::GoToEnd);
    }

    #[test]
    fn test_voice_error_display() {
        assert_eq!(
            VoiceError::WhisperNotFound.to_string(),
            "Whisper CLI not found in PATH"
        );
        assert_eq!(
            VoiceError::RecordingFailed("mic error".to_string()).to_string(),
            "Recording failed: mic error"
        );
    }

    #[test]
    fn test_voice_command_to_action() {
        use crate::input::Action;
        
        // Commands that map to actions
        assert!(VoiceCommand::GoToStart.to_action().is_some());
        assert!(VoiceCommand::GoToEnd.to_action().is_some());
        assert!(VoiceCommand::DeleteLine.to_action().is_some());
        assert!(VoiceCommand::Undo.to_action().is_some());
        assert!(VoiceCommand::Redo.to_action().is_some());
        assert!(VoiceCommand::Save.to_action().is_some());
        assert!(VoiceCommand::Quit.to_action().is_some());
        assert!(VoiceCommand::Help.to_action().is_some());
        
        // Commands that need special handling
        assert!(VoiceCommand::GoToLine(42).to_action().is_none());
        assert!(VoiceCommand::Insert("text".to_string()).to_action().is_none());
        assert!(VoiceCommand::Unknown("?".to_string()).to_action().is_none());
    }

    #[test]
    fn test_voice_command_to_action_types() {
        use crate::input::Action;
        
        assert_eq!(VoiceCommand::Save.to_action(), Some(Action::Save));
        assert_eq!(VoiceCommand::Quit.to_action(), Some(Action::Quit));
        assert_eq!(VoiceCommand::Undo.to_action(), Some(Action::Undo));
        assert_eq!(VoiceCommand::Redo.to_action(), Some(Action::Redo));
        assert_eq!(VoiceCommand::DeleteLine.to_action(), Some(Action::DeleteLine));
        assert_eq!(VoiceCommand::Help.to_action(), Some(Action::ShowHelp));
    }
}
