use ropey::Rope;

/// Search functionality for the editor
pub struct Search {
    query: String,
    query_lower: String,
}

impl Search {
    /// Create a new empty search
    pub fn new() -> Self {
        Self {
            query: String::new(),
            query_lower: String::new(),
        }
    }

    /// Set the search query
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.query_lower = query.to_lowercase();
    }

    /// Get the current query
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Clear the search
    pub fn clear(&mut self) {
        self.query.clear();
        self.query_lower.clear();
    }

    /// Check if search is active
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Find next match after the given char position
    /// Returns (start, end) char positions if found
    pub fn find_next(&self, content: &Rope, from_char: usize) -> Option<(usize, usize)> {
        if self.query.is_empty() {
            return None;
        }

        let text: String = content.chars().collect();
        let text_lower = text.to_lowercase();
        let query_len = self.query.chars().count();

        // Search from position
        if let Some(pos) = text_lower[from_char..].find(&self.query_lower) {
            let start = from_char + pos;
            return Some((start, start + query_len));
        }

        // Wrap around: search from beginning
        if from_char > 0 {
            if let Some(pos) = text_lower[..from_char].find(&self.query_lower) {
                return Some((pos, pos + query_len));
            }
        }

        None
    }

    /// Find previous match before the given char position
    /// Returns (start, end) char positions if found
    pub fn find_prev(&self, content: &Rope, from_char: usize) -> Option<(usize, usize)> {
        if self.query.is_empty() {
            return None;
        }

        let text: String = content.chars().collect();
        let text_lower = text.to_lowercase();
        let query_len = self.query.chars().count();

        // Search backwards from position
        if from_char > 0 {
            if let Some(pos) = text_lower[..from_char].rfind(&self.query_lower) {
                return Some((pos, pos + query_len));
            }
        }

        // Wrap around: search from end
        if from_char < text.len() {
            if let Some(pos) = text_lower[from_char..].rfind(&self.query_lower) {
                let start = from_char + pos;
                return Some((start, start + query_len));
            }
        }

        None
    }

    /// Find all matches in the content
    /// Returns vec of (start, end) char positions
    pub fn all_matches(&self, content: &Rope) -> Vec<(usize, usize)> {
        if self.query.is_empty() {
            return Vec::new();
        }

        let text: String = content.chars().collect();
        let text_lower = text.to_lowercase();
        let query_len = self.query.chars().count();

        text_lower
            .match_indices(&self.query_lower)
            .map(|(pos, _)| (pos, pos + query_len))
            .collect()
    }
}

impl Default for Search {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rope(s: &str) -> Rope {
        Rope::from_str(s)
    }

    #[test]
    fn test_new_search_is_empty() {
        let search = Search::new();
        assert!(search.query().is_empty());
        assert!(!search.is_active());
    }

    #[test]
    fn test_set_query() {
        let mut search = Search::new();
        search.set_query("hello");
        assert_eq!(search.query(), "hello");
        assert!(search.is_active());
    }

    #[test]
    fn test_clear() {
        let mut search = Search::new();
        search.set_query("hello");
        search.clear();
        assert!(search.query().is_empty());
        assert!(!search.is_active());
    }

    #[test]
    fn test_find_exact_match() {
        let mut search = Search::new();
        search.set_query("world");
        let rope = make_rope("hello world");

        let result = search.find_next(&rope, 0);
        assert_eq!(result, Some((6, 11)));
    }

    #[test]
    fn test_find_case_insensitive() {
        let mut search = Search::new();
        search.set_query("HELLO");
        let rope = make_rope("hello world");

        let result = search.find_next(&rope, 0);
        assert_eq!(result, Some((0, 5)));
    }

    #[test]
    fn test_find_no_match() {
        let mut search = Search::new();
        search.set_query("xyz");
        let rope = make_rope("hello world");

        let result = search.find_next(&rope, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_wrap_around() {
        let mut search = Search::new();
        search.set_query("hello");
        let rope = make_rope("hello world hello");

        // Start after both hellos, should wrap to first
        let result = search.find_next(&rope, 15);
        assert_eq!(result, Some((0, 5)));
    }

    #[test]
    fn test_find_prev() {
        let mut search = Search::new();
        search.set_query("hello");
        let rope = make_rope("hello world hello");

        // From end, find previous (second hello)
        let result = search.find_prev(&rope, 17);
        assert_eq!(result, Some((12, 17)));
    }

    #[test]
    fn test_find_prev_wrap() {
        let mut search = Search::new();
        search.set_query("world");
        let rope = make_rope("hello world");

        // From start, wrap to find world
        let result = search.find_prev(&rope, 0);
        assert_eq!(result, Some((6, 11)));
    }

    #[test]
    fn test_all_matches() {
        let mut search = Search::new();
        search.set_query("o");
        let rope = make_rope("hello world");

        let matches = search.all_matches(&rope);
        // "hello world" has 'o' at positions 4, 7
        assert_eq!(matches.len(), 2);
        assert_eq!(matches, vec![(4, 5), (7, 8)]);
    }

    #[test]
    fn test_empty_query_returns_none() {
        let search = Search::new();
        let rope = make_rope("hello world");

        assert_eq!(search.find_next(&rope, 0), None);
        assert_eq!(search.find_prev(&rope, 10), None);
        assert!(search.all_matches(&rope).is_empty());
    }
}
