//! Statistics tracking for writing goals and streaks
//!
//! Stores daily word counts in SQLite database at ~/.config/hollow/stats.db

use chrono::{Local, NaiveDate};
use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;

/// Daily writing statistics
#[derive(Debug, Clone)]
pub struct DailyStats {
    pub date: NaiveDate,
    pub words_written: usize,
    pub goal_met: bool,
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
        
        // Create table if not exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_stats (
                date TEXT PRIMARY KEY,
                words_written INTEGER NOT NULL,
                goal_met INTEGER NOT NULL DEFAULT 0
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
}
