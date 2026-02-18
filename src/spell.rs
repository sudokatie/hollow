use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use spellbook::Dictionary;

/// Spell checker with personal dictionary support
pub struct SpellChecker {
    dictionary: Option<Dictionary>,
    personal_words: HashSet<String>,
    personal_dict_path: PathBuf,
    enabled: bool,
    language: String,
}

/// A misspelled word with its position
#[derive(Debug, Clone, PartialEq)]
pub struct Misspelling {
    pub word: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

/// Result of checking text
#[derive(Debug, Default)]
pub struct SpellCheckResult {
    pub misspellings: Vec<Misspelling>,
}

impl SpellChecker {
    /// Create a new spell checker with the given language
    pub fn new(language: &str) -> Self {
        let personal_dict_path = dirs::config_dir()
            .map(|p| p.join("hollow").join("personal.dic"))
            .unwrap_or_else(|| PathBuf::from("personal.dic"));

        let mut checker = Self {
            dictionary: None,
            personal_words: HashSet::new(),
            personal_dict_path,
            enabled: true,
            language: language.to_string(),
        };

        checker.load_dictionary();
        checker.load_personal_dictionary();
        checker
    }

    /// Load the main dictionary for the configured language
    fn load_dictionary(&mut self) {
        let dic_paths = Self::get_dictionary_paths(&self.language);

        for (aff_path, dic_path) in dic_paths {
            if aff_path.exists() && dic_path.exists() {
                if let Ok(aff) = fs::read_to_string(&aff_path) {
                    if let Ok(dic) = fs::read_to_string(&dic_path) {
                        match Dictionary::new(&aff, &dic) {
                            Ok(dict) => {
                                self.dictionary = Some(dict);
                                return;
                            }
                            Err(_) => continue,
                        }
                    }
                }
            }
        }
        // No dictionary found - spell checking will be disabled
    }

    /// Get potential dictionary paths for a language
    fn get_dictionary_paths(language: &str) -> Vec<(PathBuf, PathBuf)> {
        let mut paths = Vec::new();

        // System paths
        #[cfg(target_os = "linux")]
        {
            paths.push((
                PathBuf::from(format!("/usr/share/hunspell/{}.aff", language)),
                PathBuf::from(format!("/usr/share/hunspell/{}.dic", language)),
            ));
            paths.push((
                PathBuf::from(format!("/usr/share/myspell/{}.aff", language)),
                PathBuf::from(format!("/usr/share/myspell/{}.dic", language)),
            ));
            if let Some(home) = dirs::home_dir() {
                paths.push((
                    home.join(format!(".local/share/hunspell/{}.aff", language)),
                    home.join(format!(".local/share/hunspell/{}.dic", language)),
                ));
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                paths.push((
                    home.join(format!("Library/Spelling/{}.aff", language)),
                    home.join(format!("Library/Spelling/{}.dic", language)),
                ));
            }
            paths.push((
                PathBuf::from(format!("/Library/Spelling/{}.aff", language)),
                PathBuf::from(format!("/Library/Spelling/{}.dic", language)),
            ));
            // Homebrew hunspell
            paths.push((
                PathBuf::from(format!(
                    "/usr/local/share/hunspell/{}.aff",
                    language
                )),
                PathBuf::from(format!(
                    "/usr/local/share/hunspell/{}.dic",
                    language
                )),
            ));
            paths.push((
                PathBuf::from(format!(
                    "/opt/homebrew/share/hunspell/{}.aff",
                    language
                )),
                PathBuf::from(format!(
                    "/opt/homebrew/share/hunspell/{}.dic",
                    language
                )),
            ));
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                let base = PathBuf::from(local_app_data);
                paths.push((
                    base.join(format!("hunspell/{}.aff", language)),
                    base.join(format!("hunspell/{}.dic", language)),
                ));
            }
        }

        paths
    }

    /// Load personal dictionary
    fn load_personal_dictionary(&mut self) {
        if let Ok(file) = fs::File::open(&self.personal_dict_path) {
            let reader = io::BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let word = line.trim().to_lowercase();
                if !word.is_empty() {
                    self.personal_words.insert(word);
                }
            }
        }
    }

    /// Save personal dictionary
    fn save_personal_dictionary(&self) -> io::Result<()> {
        if let Some(parent) = self.personal_dict_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(&self.personal_dict_path)?;
        let mut words: Vec<_> = self.personal_words.iter().collect();
        words.sort();
        for word in words {
            writeln!(file, "{}", word)?;
        }
        Ok(())
    }

    /// Check if a word is spelled correctly
    pub fn check_word(&self, word: &str) -> bool {
        if !self.enabled {
            return true;
        }

        let lower = word.to_lowercase();

        // Check personal dictionary first
        if self.personal_words.contains(&lower) {
            return true;
        }

        // Check main dictionary
        if let Some(ref dict) = self.dictionary {
            dict.check(word)
        } else {
            // No dictionary loaded - assume correct
            true
        }
    }

    /// Get spelling suggestions for a word
    pub fn suggest(&self, word: &str) -> Vec<String> {
        if let Some(ref dict) = self.dictionary {
            let mut suggestions = Vec::new();
            dict.suggest(word, &mut suggestions);
            suggestions.truncate(5);
            suggestions
        } else {
            Vec::new()
        }
    }

    /// Add a word to the personal dictionary
    pub fn add_to_personal(&mut self, word: &str) {
        let lower = word.to_lowercase();
        self.personal_words.insert(lower);
        let _ = self.save_personal_dictionary();
    }

    /// Check if spell checking is available (dictionary loaded)
    pub fn is_available(&self) -> bool {
        self.dictionary.is_some()
    }

    /// Check if spell checking is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.dictionary.is_some()
    }

    /// Toggle spell checking on/off
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Set enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check a line of text and return misspellings
    pub fn check_line(&self, line: &str, line_num: usize) -> Vec<Misspelling> {
        if !self.is_enabled() {
            return Vec::new();
        }

        let mut misspellings = Vec::new();
        let mut word_start = None;
        let mut col = 0;

        for (i, c) in line.char_indices() {
            if c.is_alphabetic() || c == '\'' {
                if word_start.is_none() {
                    word_start = Some((i, col));
                }
            } else if let Some((start, start_col)) = word_start {
                let word = &line[start..i];
                if word.len() > 1 && !self.check_word(word) {
                    misspellings.push(Misspelling {
                        word: word.to_string(),
                        start,
                        end: i,
                        line: line_num,
                        col: start_col,
                    });
                }
                word_start = None;
            }
            col += 1;
        }

        // Check last word
        if let Some((start, start_col)) = word_start {
            let word = &line[start..];
            if word.len() > 1 && !self.check_word(word) {
                misspellings.push(Misspelling {
                    word: word.to_string(),
                    start,
                    end: line.len(),
                    line: line_num,
                    col: start_col,
                });
            }
        }

        misspellings
    }

    /// Check entire text and return all misspellings
    pub fn check_text(&self, text: &str) -> SpellCheckResult {
        let mut result = SpellCheckResult::default();

        if !self.is_enabled() {
            return result;
        }

        for (line_num, line) in text.lines().enumerate() {
            result.misspellings.extend(self.check_line(line, line_num));
        }

        result
    }

    /// Get the word at a specific position in a line
    pub fn word_at_position(&self, line: &str, col: usize) -> Option<(String, usize, usize)> {
        let chars: Vec<char> = line.chars().collect();
        if col >= chars.len() {
            return None;
        }

        // Find word boundaries
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphabetic() || chars[start - 1] == '\'') {
            start -= 1;
        }

        let mut end = col;
        while end < chars.len() && (chars[end].is_alphabetic() || chars[end] == '\'') {
            end += 1;
        }

        if start == end {
            return None;
        }

        let word: String = chars[start..end].iter().collect();
        Some((word, start, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spell_checker_creation() {
        let checker = SpellChecker::new("en_US");
        // May or may not have dictionary depending on system
        assert!(checker.enabled);
    }

    #[test]
    fn test_personal_dictionary() {
        let mut checker = SpellChecker::new("en_US");
        checker.personal_words.insert("customword".to_string());
        assert!(checker.check_word("customword"));
    }

    #[test]
    fn test_check_word_without_dict() {
        let mut checker = SpellChecker::new("nonexistent_lang");
        checker.dictionary = None;
        // Without dictionary, all words are "correct"
        assert!(checker.check_word("anything"));
    }

    #[test]
    fn test_toggle() {
        let mut checker = SpellChecker::new("en_US");
        assert!(checker.enabled);
        checker.toggle();
        assert!(!checker.enabled);
        checker.toggle();
        assert!(checker.enabled);
    }

    #[test]
    fn test_word_at_position() {
        let checker = SpellChecker::new("en_US");
        let line = "hello world test";

        let result = checker.word_at_position(line, 2);
        assert!(result.is_some());
        let (word, start, end) = result.unwrap();
        assert_eq!(word, "hello");
        assert_eq!(start, 0);
        assert_eq!(end, 5);

        let result = checker.word_at_position(line, 7);
        assert!(result.is_some());
        let (word, _, _) = result.unwrap();
        assert_eq!(word, "world");
    }

    #[test]
    fn test_word_at_position_with_apostrophe() {
        let checker = SpellChecker::new("en_US");
        let line = "don't stop";

        let result = checker.word_at_position(line, 2);
        assert!(result.is_some());
        let (word, _, _) = result.unwrap();
        assert_eq!(word, "don't");
    }

    #[test]
    fn test_check_line_disabled() {
        let mut checker = SpellChecker::new("en_US");
        checker.set_enabled(false);
        let result = checker.check_line("xyzzy zzyzx", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_check_text_empty() {
        let checker = SpellChecker::new("en_US");
        let result = checker.check_text("");
        assert!(result.misspellings.is_empty());
    }

    #[test]
    fn test_add_to_personal() {
        let mut checker = SpellChecker::new("en_US");
        checker.add_to_personal("MyCustomWord");
        assert!(checker.personal_words.contains("mycustomword"));
    }

    #[test]
    fn test_single_char_words_ignored() {
        let checker = SpellChecker::new("en_US");
        // Single char words should not be flagged
        let line = "a b c I x y z";
        let result = checker.check_line(line, 0);
        // Even without dict, single char words aren't checked
        for m in &result {
            assert!(m.word.len() > 1, "Single char word flagged: {}", m.word);
        }
    }

    #[test]
    fn test_misspelling_struct() {
        let m = Misspelling {
            word: "tset".to_string(),
            start: 0,
            end: 4,
            line: 0,
            col: 0,
        };
        assert_eq!(m.word, "tset");
        assert_eq!(m.end - m.start, 4);
    }

    #[test]
    fn test_suggest_without_dict() {
        let mut checker = SpellChecker::new("en_US");
        checker.dictionary = None;
        let suggestions = checker.suggest("tset");
        assert!(suggestions.is_empty());
    }
}
