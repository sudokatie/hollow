//! Focus tracking with Pomodoro timer and distraction detection
//!
//! Tracks focus sessions, provides a Pomodoro timer, and detects idle/distraction periods.

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;

/// Pomodoro timer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    /// Timer is idle/stopped
    Idle,
    /// Working period
    Working,
    /// On a short break
    ShortBreak,
    /// On a long break
    LongBreak,
    /// Timer is paused (preserves work/break state)
    Paused,
}

impl TimerState {
    pub fn is_active(&self) -> bool {
        matches!(self, TimerState::Working | TimerState::ShortBreak | TimerState::LongBreak)
    }
    
    pub fn is_work(&self) -> bool {
        matches!(self, TimerState::Working)
    }
    
    pub fn display(&self) -> &'static str {
        match self {
            TimerState::Idle => "Idle",
            TimerState::Working => "Working",
            TimerState::ShortBreak => "Short Break",
            TimerState::LongBreak => "Long Break",
            TimerState::Paused => "Paused",
        }
    }
}

/// Pomodoro timer configuration
#[derive(Debug, Clone)]
pub struct PomodoroConfig {
    /// Work period duration in minutes
    pub work_minutes: u32,
    /// Short break duration in minutes
    pub short_break_minutes: u32,
    /// Long break duration in minutes
    pub long_break_minutes: u32,
    /// Number of work periods before long break
    pub periods_before_long_break: u32,
    /// Idle threshold in seconds (for distraction detection)
    pub idle_threshold_secs: u64,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            periods_before_long_break: 4,
            idle_threshold_secs: 120, // 2 minutes of no activity = idle
        }
    }
}

/// Pomodoro timer
#[derive(Debug)]
pub struct PomodoroTimer {
    pub config: PomodoroConfig,
    pub state: TimerState,
    /// When current period started
    pub period_start: Option<DateTime<Local>>,
    /// Completed work periods in current cycle
    pub completed_periods: u32,
    /// State before pause (to resume correctly)
    paused_state: Option<TimerState>,
    /// Total seconds remaining when paused
    paused_remaining: Option<i64>,
}

impl PomodoroTimer {
    pub fn new(config: PomodoroConfig) -> Self {
        Self {
            config,
            state: TimerState::Idle,
            period_start: None,
            completed_periods: 0,
            paused_state: None,
            paused_remaining: None,
        }
    }
    
    /// Start a work period
    pub fn start_work(&mut self) {
        self.state = TimerState::Working;
        self.period_start = Some(Local::now());
        self.paused_state = None;
        self.paused_remaining = None;
    }
    
    /// Start a break (short or long based on completed periods)
    pub fn start_break(&mut self) {
        self.completed_periods += 1;
        
        if self.completed_periods >= self.config.periods_before_long_break {
            self.state = TimerState::LongBreak;
            self.completed_periods = 0;
        } else {
            self.state = TimerState::ShortBreak;
        }
        
        self.period_start = Some(Local::now());
        self.paused_state = None;
        self.paused_remaining = None;
    }
    
    /// Pause the timer
    pub fn pause(&mut self) {
        if self.state.is_active() {
            self.paused_state = Some(self.state);
            self.paused_remaining = Some(self.remaining_seconds());
            self.state = TimerState::Paused;
        }
    }
    
    /// Resume from pause
    pub fn resume(&mut self) {
        if self.state == TimerState::Paused {
            if let (Some(state), Some(remaining)) = (self.paused_state, self.paused_remaining) {
                self.state = state;
                // Adjust period_start so remaining time is preserved
                self.period_start = Some(Local::now() - Duration::seconds(self.period_duration_secs() as i64 - remaining));
                self.paused_state = None;
                self.paused_remaining = None;
            }
        }
    }
    
    /// Stop the timer completely
    pub fn stop(&mut self) {
        self.state = TimerState::Idle;
        self.period_start = None;
        self.paused_state = None;
        self.paused_remaining = None;
    }
    
    /// Reset the cycle (completed periods)
    pub fn reset_cycle(&mut self) {
        self.completed_periods = 0;
        self.stop();
    }
    
    /// Get duration of current period type in seconds
    fn period_duration_secs(&self) -> u32 {
        match self.state {
            TimerState::Working => self.config.work_minutes * 60,
            TimerState::ShortBreak => self.config.short_break_minutes * 60,
            TimerState::LongBreak => self.config.long_break_minutes * 60,
            TimerState::Paused => {
                // Use paused state to determine duration
                match self.paused_state {
                    Some(TimerState::Working) => self.config.work_minutes * 60,
                    Some(TimerState::ShortBreak) => self.config.short_break_minutes * 60,
                    Some(TimerState::LongBreak) => self.config.long_break_minutes * 60,
                    _ => 0,
                }
            }
            TimerState::Idle => 0,
        }
    }
    
    /// Get remaining seconds in current period
    pub fn remaining_seconds(&self) -> i64 {
        if self.state == TimerState::Paused {
            return self.paused_remaining.unwrap_or(0);
        }
        
        if let Some(start) = self.period_start {
            let elapsed = (Local::now() - start).num_seconds();
            let duration = self.period_duration_secs() as i64;
            (duration - elapsed).max(0)
        } else {
            0
        }
    }
    
    /// Format remaining time as MM:SS
    pub fn format_remaining(&self) -> String {
        let secs = self.remaining_seconds();
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
    
    /// Check if current period is complete
    pub fn is_period_complete(&self) -> bool {
        self.state.is_active() && self.remaining_seconds() <= 0
    }
}

/// A recorded focus session
#[derive(Debug, Clone)]
pub struct FocusSession {
    pub id: Option<i64>,
    pub start_time: NaiveDateTime,
    pub end_time: Option<NaiveDateTime>,
    pub focus_minutes: u32,
    pub idle_minutes: u32,
    pub interruptions: u32,
    pub words_written: usize,
    pub completed: bool,
}

impl FocusSession {
    pub fn new() -> Self {
        Self {
            id: None,
            start_time: Local::now().naive_local(),
            end_time: None,
            focus_minutes: 0,
            idle_minutes: 0,
            interruptions: 0,
            words_written: 0,
            completed: false,
        }
    }
    
    /// Calculate focus score (0-100)
    pub fn focus_score(&self) -> u32 {
        let total_minutes = self.focus_minutes + self.idle_minutes;
        if total_minutes == 0 {
            return 100;
        }
        
        // Base score from focus ratio
        let focus_ratio = self.focus_minutes as f64 / total_minutes as f64;
        let mut score = (focus_ratio * 100.0) as u32;
        
        // Penalty for interruptions (5 points each, max 30)
        let interruption_penalty = (self.interruptions * 5).min(30);
        score = score.saturating_sub(interruption_penalty);
        
        score.min(100)
    }
    
    /// Get total duration in minutes
    pub fn total_minutes(&self) -> u32 {
        self.focus_minutes + self.idle_minutes
    }
}

impl Default for FocusSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Focus tracker with persistence and idle detection
pub struct FocusTracker {
    conn: Connection,
    config: PomodoroConfig,
    /// Current session being tracked
    current_session: Option<FocusSession>,
    /// Last activity timestamp
    last_activity: DateTime<Local>,
    /// Whether we're currently in idle state
    is_idle: bool,
}

impl FocusTracker {
    pub fn new(config: PomodoroConfig) -> SqlResult<Self> {
        let db_path = Self::db_path();
        
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        
        let conn = Connection::open(&db_path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS focus_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time TEXT NOT NULL,
                end_time TEXT,
                focus_minutes INTEGER NOT NULL DEFAULT 0,
                idle_minutes INTEGER NOT NULL DEFAULT 0,
                interruptions INTEGER NOT NULL DEFAULT 0,
                words_written INTEGER NOT NULL DEFAULT 0,
                completed INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        
        Ok(Self {
            conn,
            config,
            current_session: None,
            last_activity: Local::now(),
            is_idle: false,
        })
    }
    
    fn db_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hollow")
            .join("stats.db")
    }
    
    /// Start a new focus session
    pub fn start_session(&mut self) {
        self.current_session = Some(FocusSession::new());
        self.last_activity = Local::now();
        self.is_idle = false;
    }
    
    /// Record activity (call on keypress, etc.)
    pub fn record_activity(&mut self) {
        let now = Local::now();
        
        if let Some(ref mut session) = self.current_session {
            if self.is_idle {
                // Coming back from idle
                self.is_idle = false;
                session.interruptions += 1;
            }
            
            // Add time since last activity
            let elapsed_secs = (now - self.last_activity).num_seconds();
            
            if elapsed_secs > self.config.idle_threshold_secs as i64 {
                // Was idle - add to idle time
                session.idle_minutes += (elapsed_secs / 60) as u32;
            } else {
                // Was focused
                session.focus_minutes += (elapsed_secs / 60).max(0) as u32;
            }
        }
        
        self.last_activity = now;
    }
    
    /// Check for idle state (call periodically)
    pub fn check_idle(&mut self) -> bool {
        let elapsed = (Local::now() - self.last_activity).num_seconds();
        let was_idle = self.is_idle;
        self.is_idle = elapsed > self.config.idle_threshold_secs as i64;
        
        // Return true if just became idle
        !was_idle && self.is_idle
    }
    
    /// Get current idle duration in seconds
    pub fn idle_seconds(&self) -> i64 {
        if self.is_idle {
            (Local::now() - self.last_activity).num_seconds()
        } else {
            0
        }
    }
    
    /// End current session
    pub fn end_session(&mut self, words_written: usize, completed: bool) -> SqlResult<FocusSession> {
        self.record_activity(); // Final activity recording
        
        let mut session = self.current_session.take().unwrap_or_else(FocusSession::new);
        session.end_time = Some(Local::now().naive_local());
        session.words_written = words_written;
        session.completed = completed;
        
        // Save to database
        self.conn.execute(
            "INSERT INTO focus_sessions 
             (start_time, end_time, focus_minutes, idle_minutes, interruptions, words_written, completed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                session.start_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                session.end_time.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
                session.focus_minutes as i64,
                session.idle_minutes as i64,
                session.interruptions as i64,
                session.words_written as i64,
                session.completed as i64,
            ),
        )?;
        
        session.id = Some(self.conn.last_insert_rowid());
        
        Ok(session)
    }
    
    /// Get focus session history
    pub fn get_history(&self, limit: usize) -> SqlResult<Vec<FocusSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start_time, end_time, focus_minutes, idle_minutes, interruptions, words_written, completed
             FROM focus_sessions
             ORDER BY start_time DESC
             LIMIT ?1"
        )?;
        
        let sessions = stmt.query_map([limit as i64], |row| {
            let end_time_str: Option<String> = row.get(2)?;
            Ok(FocusSession {
                id: Some(row.get(0)?),
                start_time: NaiveDateTime::parse_from_str(&row.get::<_, String>(1)?, "%Y-%m-%d %H:%M:%S")
                    .unwrap_or_else(|_| Local::now().naive_local()),
                end_time: end_time_str.and_then(|s| 
                    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()
                ),
                focus_minutes: row.get::<_, i64>(3)? as u32,
                idle_minutes: row.get::<_, i64>(4)? as u32,
                interruptions: row.get::<_, i64>(5)? as u32,
                words_written: row.get::<_, i64>(6)? as usize,
                completed: row.get::<_, i64>(7)? != 0,
            })
        })?;
        
        sessions.collect()
    }
    
    /// Get aggregate focus statistics
    pub fn get_stats(&self) -> SqlResult<FocusStats> {
        let sessions = self.get_history(100)?;
        
        let total_sessions = sessions.len();
        let completed_sessions = sessions.iter().filter(|s| s.completed).count();
        let total_focus_minutes: u32 = sessions.iter().map(|s| s.focus_minutes).sum();
        let total_idle_minutes: u32 = sessions.iter().map(|s| s.idle_minutes).sum();
        let total_interruptions: u32 = sessions.iter().map(|s| s.interruptions).sum();
        let total_words: usize = sessions.iter().map(|s| s.words_written).sum();
        
        let avg_focus_score = if !sessions.is_empty() {
            sessions.iter().map(|s| s.focus_score()).sum::<u32>() / sessions.len() as u32
        } else {
            0
        };
        
        let avg_session_minutes = if total_sessions > 0 {
            (total_focus_minutes + total_idle_minutes) / total_sessions as u32
        } else {
            0
        };
        
        Ok(FocusStats {
            total_sessions,
            completed_sessions,
            total_focus_minutes,
            total_idle_minutes,
            total_interruptions,
            total_words,
            avg_focus_score,
            avg_session_minutes,
        })
    }
    
    /// Check if currently tracking a session
    pub fn has_active_session(&self) -> bool {
        self.current_session.is_some()
    }
    
    /// Get current session (if any)
    pub fn current_session(&self) -> Option<&FocusSession> {
        self.current_session.as_ref()
    }
}

/// Aggregate focus statistics
#[derive(Debug, Clone, Default)]
pub struct FocusStats {
    pub total_sessions: usize,
    pub completed_sessions: usize,
    pub total_focus_minutes: u32,
    pub total_idle_minutes: u32,
    pub total_interruptions: u32,
    pub total_words: usize,
    pub avg_focus_score: u32,
    pub avg_session_minutes: u32,
}

impl FocusStats {
    /// Calculate overall focus percentage
    pub fn focus_percentage(&self) -> u32 {
        let total = self.total_focus_minutes + self.total_idle_minutes;
        if total == 0 {
            return 100;
        }
        (self.total_focus_minutes as f64 / total as f64 * 100.0) as u32
    }
    
    /// Calculate completion rate
    pub fn completion_rate(&self) -> u32 {
        if self.total_sessions == 0 {
            return 0;
        }
        (self.completed_sessions as f64 / self.total_sessions as f64 * 100.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timer_state_display() {
        assert_eq!(TimerState::Idle.display(), "Idle");
        assert_eq!(TimerState::Working.display(), "Working");
        assert_eq!(TimerState::ShortBreak.display(), "Short Break");
        assert_eq!(TimerState::LongBreak.display(), "Long Break");
        assert_eq!(TimerState::Paused.display(), "Paused");
    }
    
    #[test]
    fn test_timer_state_is_active() {
        assert!(!TimerState::Idle.is_active());
        assert!(TimerState::Working.is_active());
        assert!(TimerState::ShortBreak.is_active());
        assert!(TimerState::LongBreak.is_active());
        assert!(!TimerState::Paused.is_active());
    }
    
    #[test]
    fn test_timer_state_is_work() {
        assert!(!TimerState::Idle.is_work());
        assert!(TimerState::Working.is_work());
        assert!(!TimerState::ShortBreak.is_work());
        assert!(!TimerState::LongBreak.is_work());
        assert!(!TimerState::Paused.is_work());
    }
    
    #[test]
    fn test_pomodoro_config_default() {
        let config = PomodoroConfig::default();
        assert_eq!(config.work_minutes, 25);
        assert_eq!(config.short_break_minutes, 5);
        assert_eq!(config.long_break_minutes, 15);
        assert_eq!(config.periods_before_long_break, 4);
        assert_eq!(config.idle_threshold_secs, 120);
    }
    
    #[test]
    fn test_timer_start_work() {
        let mut timer = PomodoroTimer::new(PomodoroConfig::default());
        
        assert_eq!(timer.state, TimerState::Idle);
        timer.start_work();
        assert_eq!(timer.state, TimerState::Working);
        assert!(timer.period_start.is_some());
    }
    
    #[test]
    fn test_timer_start_break() {
        let mut timer = PomodoroTimer::new(PomodoroConfig::default());
        
        timer.start_work();
        timer.start_break();
        
        assert_eq!(timer.state, TimerState::ShortBreak);
        assert_eq!(timer.completed_periods, 1);
    }
    
    #[test]
    fn test_timer_long_break_after_4_periods() {
        let mut timer = PomodoroTimer::new(PomodoroConfig::default());
        
        // Complete 4 work periods
        for _ in 0..4 {
            timer.start_work();
            timer.start_break();
        }
        
        // After 4th period, should be long break
        assert_eq!(timer.state, TimerState::LongBreak);
        assert_eq!(timer.completed_periods, 0); // Reset after long break
    }
    
    #[test]
    fn test_timer_pause_resume() {
        let mut timer = PomodoroTimer::new(PomodoroConfig::default());
        
        timer.start_work();
        timer.pause();
        
        assert_eq!(timer.state, TimerState::Paused);
        assert_eq!(timer.paused_state, Some(TimerState::Working));
        
        timer.resume();
        assert_eq!(timer.state, TimerState::Working);
        assert!(timer.paused_state.is_none());
    }
    
    #[test]
    fn test_timer_stop() {
        let mut timer = PomodoroTimer::new(PomodoroConfig::default());
        
        timer.start_work();
        timer.stop();
        
        assert_eq!(timer.state, TimerState::Idle);
        assert!(timer.period_start.is_none());
    }
    
    #[test]
    fn test_timer_format_remaining() {
        let timer = PomodoroTimer::new(PomodoroConfig::default());
        
        // Idle timer should show 00:00
        assert_eq!(timer.format_remaining(), "00:00");
    }
    
    #[test]
    fn test_focus_session_new() {
        let session = FocusSession::new();
        
        assert!(session.id.is_none());
        assert!(session.end_time.is_none());
        assert_eq!(session.focus_minutes, 0);
        assert_eq!(session.idle_minutes, 0);
        assert_eq!(session.interruptions, 0);
        assert_eq!(session.words_written, 0);
        assert!(!session.completed);
    }
    
    #[test]
    fn test_focus_session_score_perfect() {
        let session = FocusSession {
            id: None,
            start_time: Local::now().naive_local(),
            end_time: None,
            focus_minutes: 25,
            idle_minutes: 0,
            interruptions: 0,
            words_written: 500,
            completed: true,
        };
        
        assert_eq!(session.focus_score(), 100);
    }
    
    #[test]
    fn test_focus_session_score_with_idle() {
        let session = FocusSession {
            id: None,
            start_time: Local::now().naive_local(),
            end_time: None,
            focus_minutes: 20,
            idle_minutes: 5, // 20% idle
            interruptions: 0,
            words_written: 400,
            completed: true,
        };
        
        assert_eq!(session.focus_score(), 80);
    }
    
    #[test]
    fn test_focus_session_score_with_interruptions() {
        let session = FocusSession {
            id: None,
            start_time: Local::now().naive_local(),
            end_time: None,
            focus_minutes: 25,
            idle_minutes: 0,
            interruptions: 3, // -15 points
            words_written: 500,
            completed: true,
        };
        
        assert_eq!(session.focus_score(), 85);
    }
    
    #[test]
    fn test_focus_session_total_minutes() {
        let session = FocusSession {
            id: None,
            start_time: Local::now().naive_local(),
            end_time: None,
            focus_minutes: 20,
            idle_minutes: 5,
            interruptions: 0,
            words_written: 0,
            completed: false,
        };
        
        assert_eq!(session.total_minutes(), 25);
    }
    
    #[test]
    fn test_focus_stats_focus_percentage() {
        let stats = FocusStats {
            total_sessions: 10,
            completed_sessions: 8,
            total_focus_minutes: 200,
            total_idle_minutes: 50,
            total_interruptions: 5,
            total_words: 5000,
            avg_focus_score: 80,
            avg_session_minutes: 25,
        };
        
        assert_eq!(stats.focus_percentage(), 80);
    }
    
    #[test]
    fn test_focus_stats_completion_rate() {
        let stats = FocusStats {
            total_sessions: 10,
            completed_sessions: 8,
            total_focus_minutes: 200,
            total_idle_minutes: 50,
            total_interruptions: 5,
            total_words: 5000,
            avg_focus_score: 80,
            avg_session_minutes: 25,
        };
        
        assert_eq!(stats.completion_rate(), 80);
    }
    
    #[test]
    fn test_focus_stats_empty() {
        let stats = FocusStats::default();
        
        assert_eq!(stats.focus_percentage(), 100);
        assert_eq!(stats.completion_rate(), 0);
    }
    
    fn test_tracker() -> FocusTracker {
        let conn = Connection::open_in_memory().unwrap();
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS focus_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time TEXT NOT NULL,
                end_time TEXT,
                focus_minutes INTEGER NOT NULL DEFAULT 0,
                idle_minutes INTEGER NOT NULL DEFAULT 0,
                interruptions INTEGER NOT NULL DEFAULT 0,
                words_written INTEGER NOT NULL DEFAULT 0,
                completed INTEGER NOT NULL DEFAULT 0
            )",
            [],
        ).unwrap();
        
        FocusTracker {
            conn,
            config: PomodoroConfig::default(),
            current_session: None,
            last_activity: Local::now(),
            is_idle: false,
        }
    }
    
    #[test]
    fn test_tracker_start_session() {
        let mut tracker = test_tracker();
        
        assert!(!tracker.has_active_session());
        tracker.start_session();
        assert!(tracker.has_active_session());
    }
    
    #[test]
    fn test_tracker_end_session() {
        let mut tracker = test_tracker();
        
        tracker.start_session();
        let session = tracker.end_session(500, true).unwrap();
        
        assert!(session.id.is_some());
        assert_eq!(session.words_written, 500);
        assert!(session.completed);
        assert!(!tracker.has_active_session());
    }
    
    #[test]
    fn test_tracker_get_history() {
        let mut tracker = test_tracker();
        
        // Add some sessions
        for i in 0..3 {
            tracker.start_session();
            tracker.end_session(100 * (i + 1), true).unwrap();
        }
        
        let history = tracker.get_history(10).unwrap();
        assert_eq!(history.len(), 3);
    }
    
    #[test]
    fn test_tracker_get_stats() {
        let tracker = test_tracker();
        
        // Add some sessions manually
        tracker.conn.execute(
            "INSERT INTO focus_sessions (start_time, end_time, focus_minutes, idle_minutes, interruptions, words_written, completed)
             VALUES ('2026-01-01 10:00:00', '2026-01-01 10:25:00', 25, 0, 0, 500, 1)",
            [],
        ).unwrap();
        
        tracker.conn.execute(
            "INSERT INTO focus_sessions (start_time, end_time, focus_minutes, idle_minutes, interruptions, words_written, completed)
             VALUES ('2026-01-01 11:00:00', '2026-01-01 11:25:00', 20, 5, 2, 400, 1)",
            [],
        ).unwrap();
        
        let stats = tracker.get_stats().unwrap();
        
        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.completed_sessions, 2);
        assert_eq!(stats.total_focus_minutes, 45);
        assert_eq!(stats.total_idle_minutes, 5);
        assert_eq!(stats.total_interruptions, 2);
        assert_eq!(stats.total_words, 900);
    }
    
    #[test]
    fn test_tracker_idle_detection() {
        let mut tracker = test_tracker();
        tracker.config.idle_threshold_secs = 1; // 1 second for testing
        
        tracker.start_session();
        
        // Initially not idle
        assert!(!tracker.is_idle);
        assert_eq!(tracker.idle_seconds(), 0);
        
        // Simulate time passing by setting last_activity in the past
        tracker.last_activity = Local::now() - Duration::seconds(5);
        
        // Now should detect idle
        let became_idle = tracker.check_idle();
        assert!(became_idle);
        assert!(tracker.is_idle);
        assert!(tracker.idle_seconds() > 0);
    }
}
