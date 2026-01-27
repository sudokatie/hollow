use std::time::{Duration, Instant};

/// Tracks session statistics
pub struct Session {
    start_time: Instant,
    initial_word_count: usize,
    current_word_count: usize,
}

impl Session {
    /// Create a new session with the given initial word count
    pub fn new(initial_word_count: usize) -> Self {
        Self {
            start_time: Instant::now(),
            initial_word_count,
            current_word_count: initial_word_count,
        }
    }

    /// Update the current word count
    pub fn update_word_count(&mut self, count: usize) {
        self.current_word_count = count;
    }

    /// Get number of words written this session (never negative)
    pub fn words_written(&self) -> usize {
        self.current_word_count.saturating_sub(self.initial_word_count)
    }

    /// Get elapsed time since session start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get formatted elapsed time string
    pub fn elapsed_formatted(&self) -> String {
        let total_secs = self.elapsed().as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;

        if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m", minutes)
        }
    }

    /// Get the current word count
    pub fn current_word_count(&self) -> usize {
        self.current_word_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_new_session() {
        let session = Session::new(100);
        assert_eq!(session.current_word_count(), 100);
        assert_eq!(session.words_written(), 0);
    }

    #[test]
    fn test_words_written() {
        let mut session = Session::new(100);
        session.update_word_count(150);
        assert_eq!(session.words_written(), 50);
    }

    #[test]
    fn test_words_written_negative_is_zero() {
        let mut session = Session::new(100);
        session.update_word_count(50);
        assert_eq!(session.words_written(), 0);
    }

    #[test]
    fn test_elapsed_formatting_minutes() {
        let session = Session::new(0);
        // Just test that it doesn't panic and returns valid format
        let formatted = session.elapsed_formatted();
        assert!(formatted.ends_with('m'));
    }

    #[test]
    fn test_elapsed_increases() {
        let session = Session::new(0);
        let initial = session.elapsed();
        sleep(Duration::from_millis(10));
        let later = session.elapsed();
        assert!(later > initial);
    }
}
