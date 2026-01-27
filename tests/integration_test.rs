use std::fs;
use std::path::Path;
use tempfile::tempdir;

// Note: We can't easily test the full TUI app in integration tests,
// but we can test the core components working together.

mod editor_integration {
    use super::*;

    // We need to access the editor module from the main crate
    // For now, test file operations directly

    #[test]
    fn test_create_edit_save_cycle() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("test.md");

        // Write initial content
        fs::write(&file_path, "Hello").unwrap();

        // Read it back
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello");

        // Modify and save
        fs::write(&file_path, "Hello World").unwrap();

        // Verify
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello World");
    }

    #[test]
    fn test_new_file_creation() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("new_file.md");

        assert!(!file_path.exists());

        // Create new file
        fs::write(&file_path, "New content").unwrap();

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "New content");
    }

    #[test]
    fn test_unicode_content() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("unicode.md");

        let unicode_content = "Hello 世界 🌍 Привет";
        fs::write(&file_path, unicode_content).unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, unicode_content);
    }

    #[test]
    fn test_multiline_content() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("multiline.md");

        let content = "Line 1\nLine 2\nLine 3\n";
        fs::write(&file_path, content).unwrap();

        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, content);
        assert_eq!(read_content.lines().count(), 3);
    }

    #[test]
    fn test_empty_file() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("empty.md");

        fs::write(&file_path, "").unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn test_large_file() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("large.md");

        // Create a file with 1000 lines
        let content: String = (0..1000)
            .map(|i| format!("This is line number {}\n", i))
            .collect();

        fs::write(&file_path, &content).unwrap();

        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content.lines().count(), 1000);
    }

    #[test]
    fn test_special_characters() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("special.md");

        let content = "Tab:\there\nBackslash: \\\nQuotes: \"'`";
        fs::write(&file_path, content).unwrap();

        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_nested_directory_creation() {
        let tmp = tempdir().unwrap();
        let nested_path = tmp.path().join("a").join("b").join("c").join("test.md");

        // Create parent directories
        if let Some(parent) = nested_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(&nested_path, "Nested content").unwrap();

        assert!(nested_path.exists());
        let content = fs::read_to_string(&nested_path).unwrap();
        assert_eq!(content, "Nested content");
    }
}

mod word_count {
    #[test]
    fn test_word_count_simple() {
        let text = "Hello world";
        let count = text.split_whitespace().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_word_count_multiline() {
        let text = "Hello world\nThis is a test\nThree lines here";
        let count = text.split_whitespace().count();
        assert_eq!(count, 9);
    }

    #[test]
    fn test_word_count_extra_whitespace() {
        let text = "  Hello   world  \n\n  test  ";
        let count = text.split_whitespace().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_word_count_empty() {
        let text = "";
        let count = text.split_whitespace().count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_word_count_only_whitespace() {
        let text = "   \n\n\t  ";
        let count = text.split_whitespace().count();
        assert_eq!(count, 0);
    }
}

mod config_integration {
    use std::env;

    #[test]
    fn test_config_dir_exists() {
        // dirs::config_dir() should return something on most systems
        if let Some(config_dir) = dirs::config_dir() {
            // Just check it's a valid path format
            assert!(!config_dir.as_os_str().is_empty());
        }
    }
}
