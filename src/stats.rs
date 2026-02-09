//! Statistics tracking for writing goals, streaks, and detailed analytics
//!
//! Stores daily word counts and session data in SQLite database at ~/.config/hollow/stats.db

use chrono::{Local, NaiveDate, NaiveDateTime};
use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;

/// Daily writing statistics
#[derive(Debug, Clone)]
pub struct DailyStats {
    pub date: NaiveDate,
    pub words_written: usize,
    pub goal_met: bool,
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub words_written: usize,
    pub duration_minutes: u32,
}

/// Aggregate writing statistics
#[derive(Debug, Clone, Default)]
pub struct WritingStats {
    pub total_words: usize,
    pub total_sessions: usize,
    pub total_minutes: u32,
    pub avg_words_per_session: usize,
    pub avg_session_minutes: u32,
    pub longest_streak: usize,
    pub current_streak: usize,
    pub most_productive_hour: Option<u32>,
    pub words_last_7_days: Vec<(String, usize)>, // (date, words)
}

/// Statistics tracker with SQLite persistence
pub struct StatsTracker {
    conn: Connection,
    daily_goal: usize,
}

impl StatsTracker {
    /// Create a new stats tracker
    pub fn new(daily_goal: usize) -> SqlResult<Self> {
        let db_path = Self::db_path();
        
        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        
        let conn = Connection::open(&db_path)?;
        
        // Create tables if not exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_stats (
                date TEXT PRIMARY KEY,
                words_written INTEGER NOT NULL,
                goal_met INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time TEXT NOT NULL,
                end_time TEXT NOT NULL,
                words_written INTEGER NOT NULL,
                duration_minutes INTEGER NOT NULL
            )",
            [],
        )?;
        
        Ok(Self { conn, daily_goal })
    }
    
    /// Get the database path
    fn db_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hollow")
            .join("stats.db")
    }
    
    /// Record words written for today
    pub fn record_words(&self, words: usize) -> SqlResult<()> {
        let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
        let goal_met = if self.daily_goal > 0 { words >= self.daily_goal } else { false };
        
        self.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) 
             VALUES (?1, ?2, ?3)
             ON CONFLICT(date) DO UPDATE SET 
                words_written = ?2,
                goal_met = ?3",
            (&today, words as i64, goal_met as i64),
        )?;
        
        Ok(())
    }
    
    /// Get words written today
    pub fn get_today_words(&self) -> SqlResult<usize> {
        let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
        
        let result: SqlResult<i64> = self.conn.query_row(
            "SELECT words_written FROM daily_stats WHERE date = ?1",
            [&today],
            |row| row.get(0),
        );
        
        match result {
            Ok(words) => Ok(words as usize),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e),
        }
    }
    
    /// Calculate current streak (consecutive days meeting goal)
    pub fn get_streak(&self) -> SqlResult<usize> {
        if self.daily_goal == 0 {
            return Ok(0);
        }
        
        let today = Local::now().date_naive();
        let mut streak = 0;
        let mut check_date = today;
        
        loop {
            let date_str = check_date.format("%Y-%m-%d").to_string();
            
            let goal_met: SqlResult<i64> = self.conn.query_row(
                "SELECT goal_met FROM daily_stats WHERE date = ?1",
                [&date_str],
                |row| row.get(0),
            );
            
            match goal_met {
                Ok(1) => {
                    streak += 1;
                    // Go back one day
                    check_date = check_date.pred_opt().unwrap_or(check_date);
                }
                Ok(0) | Ok(_) => {
                    // Goal not met (or unexpected value), streak ends
                    // But if it's today and goal not yet met, don't break streak from yesterday
                    if check_date == today {
                        check_date = check_date.pred_opt().unwrap_or(check_date);
                        continue;
                    }
                    break;
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // No record for this day
                    // If it's today, check yesterday
                    if check_date == today {
                        check_date = check_date.pred_opt().unwrap_or(check_date);
                        continue;
                    }
                    break;
                }
                Err(_) => break,
            }
        }
        
        Ok(streak)
    }
    
    /// Get progress toward daily goal (0.0 to 1.0+)
    pub fn get_progress(&self, current_words: usize) -> f64 {
        if self.daily_goal == 0 {
            return 0.0;
        }
        current_words as f64 / self.daily_goal as f64
    }
    
    /// Check if daily goal is met
    pub fn is_goal_met(&self, current_words: usize) -> bool {
        self.daily_goal > 0 && current_words >= self.daily_goal
    }
    
    /// Get the daily goal
    pub fn daily_goal(&self) -> usize {
        self.daily_goal
    }
    
    /// Record a writing session
    pub fn record_session(&self, start_time: NaiveDateTime, end_time: NaiveDateTime, words_written: usize) -> SqlResult<()> {
        let duration_minutes = (end_time - start_time).num_minutes().max(0) as u32;
        
        self.conn.execute(
            "INSERT INTO sessions (start_time, end_time, words_written, duration_minutes) 
             VALUES (?1, ?2, ?3, ?4)",
            (
                start_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                end_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                words_written as i64,
                duration_minutes as i64,
            ),
        )?;
        
        Ok(())
    }
    
    /// Get aggregate writing statistics
    pub fn get_writing_stats(&self) -> SqlResult<WritingStats> {
        let mut stats = WritingStats::default();
        
        // Total words and sessions
        let totals: SqlResult<(i64, i64, i64)> = self.conn.query_row(
            "SELECT COALESCE(COUNT(*), 0), COALESCE(SUM(words_written), 0), COALESCE(SUM(duration_minutes), 0) FROM sessions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );
        
        if let Ok((sessions, words, minutes)) = totals {
            stats.total_sessions = sessions as usize;
            stats.total_words = words as usize;
            stats.total_minutes = minutes as u32;
            
            if sessions > 0 {
                stats.avg_words_per_session = (words / sessions) as usize;
                stats.avg_session_minutes = (minutes / sessions) as u32;
            }
        }
        
        // Current streak
        stats.current_streak = self.get_streak().unwrap_or(0);
        
        // Longest streak
        stats.longest_streak = self.get_longest_streak().unwrap_or(0);
        
        // Most productive hour
        stats.most_productive_hour = self.get_most_productive_hour().ok().flatten();
        
        // Words last 7 days
        stats.words_last_7_days = self.get_words_last_n_days(7).unwrap_or_default();
        
        Ok(stats)
    }
    
    /// Get the longest streak ever
    fn get_longest_streak(&self) -> SqlResult<usize> {
        let mut stmt = self.conn.prepare(
            "SELECT date, goal_met FROM daily_stats ORDER BY date ASC"
        )?;
        
        let mut longest = 0;
        let mut current = 0;
        
        let rows = stmt.query_map([], |row| {
            let goal_met: i64 = row.get(1)?;
            Ok(goal_met == 1)
        })?;
        
        for row in rows {
            if row.unwrap_or(false) {
                current += 1;
                longest = longest.max(current);
            } else {
                current = 0;
            }
        }
        
        Ok(longest)
    }
    
    /// Get the most productive hour (0-23)
    fn get_most_productive_hour(&self) -> SqlResult<Option<u32>> {
        let result: SqlResult<i64> = self.conn.query_row(
            "SELECT CAST(substr(start_time, 12, 2) AS INTEGER) as hour 
             FROM sessions 
             GROUP BY hour 
             ORDER BY SUM(words_written) DESC 
             LIMIT 1",
            [],
            |row| row.get(0),
        );
        
        match result {
            Ok(hour) => Ok(Some(hour as u32)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
    
    /// Get words written per day for last N days
    fn get_words_last_n_days(&self, n: usize) -> SqlResult<Vec<(String, usize)>> {
        let today = Local::now().date_naive();
        let mut results = Vec::new();
        
        for i in (0..n).rev() {
            let date = today - chrono::Duration::days(i as i64);
            let date_str = date.format("%Y-%m-%d").to_string();
            
            let words: SqlResult<i64> = self.conn.query_row(
                "SELECT words_written FROM daily_stats WHERE date = ?1",
                [&date_str],
                |row| row.get(0),
            );
            
            let words = match words {
                Ok(w) => w as usize,
                Err(_) => 0,
            };
            
            results.push((date.format("%m/%d").to_string(), words));
        }
        
        Ok(results)
    }
    
    /// Export statistics to JSON string
    pub fn export_json(&self) -> SqlResult<String> {
        let stats = self.get_writing_stats()?;
        
        let json = format!(
            r#"{{"total_words":{},"total_sessions":{},"total_minutes":{},"avg_words_per_session":{},"avg_session_minutes":{},"longest_streak":{},"current_streak":{},"most_productive_hour":{}}}"#,
            stats.total_words,
            stats.total_sessions,
            stats.total_minutes,
            stats.avg_words_per_session,
            stats.avg_session_minutes,
            stats.longest_streak,
            stats.current_streak,
            stats.most_productive_hour.map(|h| h.to_string()).unwrap_or_else(|| "null".to_string()),
        );
        
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn test_tracker(daily_goal: usize) -> StatsTracker {
        // Use in-memory database for tests
        let conn = Connection::open_in_memory().unwrap();
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_stats (
                date TEXT PRIMARY KEY,
                words_written INTEGER NOT NULL,
                goal_met INTEGER NOT NULL DEFAULT 0
            )",
            [],
        ).unwrap();
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time TEXT NOT NULL,
                end_time TEXT NOT NULL,
                words_written INTEGER NOT NULL,
                duration_minutes INTEGER NOT NULL
            )",
            [],
        ).unwrap();
        
        StatsTracker { conn, daily_goal }
    }
    
    #[test]
    fn test_record_and_get_words() {
        let tracker = test_tracker(500);
        
        tracker.record_words(250).unwrap();
        let words = tracker.get_today_words().unwrap();
        assert_eq!(words, 250);
        
        // Update
        tracker.record_words(400).unwrap();
        let words = tracker.get_today_words().unwrap();
        assert_eq!(words, 400);
    }
    
    #[test]
    fn test_progress_calculation() {
        let tracker = test_tracker(500);
        
        assert_eq!(tracker.get_progress(0), 0.0);
        assert_eq!(tracker.get_progress(250), 0.5);
        assert_eq!(tracker.get_progress(500), 1.0);
        assert_eq!(tracker.get_progress(750), 1.5);
    }
    
    #[test]
    fn test_goal_met() {
        let tracker = test_tracker(500);
        
        assert!(!tracker.is_goal_met(499));
        assert!(tracker.is_goal_met(500));
        assert!(tracker.is_goal_met(501));
    }
    
    #[test]
    fn test_goal_disabled() {
        let tracker = test_tracker(0);
        
        assert_eq!(tracker.get_progress(1000), 0.0);
        assert!(!tracker.is_goal_met(1000));
        assert_eq!(tracker.get_streak().unwrap(), 0);
    }
    
    #[test]
    fn test_streak_calculation() {
        let tracker = test_tracker(100);
        
        // Record consecutive days meeting goal
        let today = Local::now().date_naive();
        let yesterday = today.pred_opt().unwrap();
        let two_days_ago = yesterday.pred_opt().unwrap();
        
        // Insert test data directly
        tracker.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 150, 1)",
            [&two_days_ago.format("%Y-%m-%d").to_string()],
        ).unwrap();
        
        tracker.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 120, 1)",
            [&yesterday.format("%Y-%m-%d").to_string()],
        ).unwrap();
        
        tracker.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 200, 1)",
            [&today.format("%Y-%m-%d").to_string()],
        ).unwrap();
        
        let streak = tracker.get_streak().unwrap();
        assert_eq!(streak, 3);
    }
    
    #[test]
    fn test_streak_broken() {
        let tracker = test_tracker(100);
        
        let today = Local::now().date_naive();
        let yesterday = today.pred_opt().unwrap();
        let two_days_ago = yesterday.pred_opt().unwrap();
        
        // Two days ago: met goal
        tracker.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 150, 1)",
            [&two_days_ago.format("%Y-%m-%d").to_string()],
        ).unwrap();
        
        // Yesterday: did NOT meet goal (breaks streak)
        tracker.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 50, 0)",
            [&yesterday.format("%Y-%m-%d").to_string()],
        ).unwrap();
        
        // Today: met goal
        tracker.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 200, 1)",
            [&today.format("%Y-%m-%d").to_string()],
        ).unwrap();
        
        let streak = tracker.get_streak().unwrap();
        assert_eq!(streak, 1); // Only today counts
    }
    
    #[test]
    fn test_record_session() {
        let tracker = test_tracker(500);
        
        let start = NaiveDateTime::parse_from_str("2026-02-09 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let end = NaiveDateTime::parse_from_str("2026-02-09 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        
        tracker.record_session(start, end, 500).unwrap();
        
        let stats = tracker.get_writing_stats().unwrap();
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.total_words, 500);
        assert_eq!(stats.total_minutes, 30);
    }
    
    #[test]
    fn test_writing_stats_averages() {
        let tracker = test_tracker(500);
        
        // Record 3 sessions
        for i in 0..3 {
            let start = NaiveDateTime::parse_from_str(&format!("2026-02-0{} 10:00:00", i+1), "%Y-%m-%d %H:%M:%S").unwrap();
            let end = NaiveDateTime::parse_from_str(&format!("2026-02-0{} 10:30:00", i+1), "%Y-%m-%d %H:%M:%S").unwrap();
            tracker.record_session(start, end, 300).unwrap();
        }
        
        let stats = tracker.get_writing_stats().unwrap();
        assert_eq!(stats.total_sessions, 3);
        assert_eq!(stats.total_words, 900);
        assert_eq!(stats.avg_words_per_session, 300);
        assert_eq!(stats.avg_session_minutes, 30);
    }
    
    #[test]
    fn test_export_json() {
        let tracker = test_tracker(500);
        
        let json = tracker.export_json().unwrap();
        assert!(json.contains("\"total_words\":0"));
        assert!(json.contains("\"total_sessions\":0"));
    }
    
    #[test]
    fn test_longest_streak() {
        let tracker = test_tracker(100);
        
        // Create a streak of 5, then break, then streak of 2
        for i in 1..=5 {
            tracker.conn.execute(
                "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 150, 1)",
                [&format!("2026-01-0{}", i)],
            ).unwrap();
        }
        tracker.conn.execute(
            "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 50, 0)",
            ["2026-01-06"],
        ).unwrap();
        for i in 7..=8 {
            tracker.conn.execute(
                "INSERT INTO daily_stats (date, words_written, goal_met) VALUES (?1, 150, 1)",
                [&format!("2026-01-0{}", i)],
            ).unwrap();
        }
        
        let longest = tracker.get_longest_streak().unwrap();
        assert_eq!(longest, 5);
    }
}
