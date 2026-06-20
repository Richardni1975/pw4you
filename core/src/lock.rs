//! Anti-tampering lock state machine.
//!
//! This module provides a simplified lock state that is stored inside
//! the `.pw4lock` metadata file (one per encrypted folder).
//!
//! ## Features
//!
//! **Clock monotonicity detection:**
//!   Stores `last_seen_time`. If `current_time < last_seen_time`, clock
//!   rollback is detected and a penalty is applied.
//!
//! **n² Lock Duration:**
//!   `lock_days = n²` where n = consecutive failed attempts.
//!   Exception: n=1 uses 2 days instead of 1²=1.
//!
//! | n   | Lock Duration |
//! |-----|---------------|
//! | 1   | 2 days        |
//! | 2   | 4 days        |
//! | 3   | 9 days        |
//! | ... | ...           |
//! | 10  | 100 days      |
//!
//! The lock state is serialized as JSON inside the `.pw4lock` metadata file
//! and is per-folder (each encrypted folder tracks its own attempt count).

use serde::{Deserialize, Serialize};

use crate::password::MAX_ATTEMPTS;

/// The lock state, stored inside `.pw4lock`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LockState {
    /// Number of consecutive failed attempts on this folder.
    pub attempt_count: u32,

    /// Unix timestamp (seconds) until which the folder is locked.
    /// 0 means not locked.
    pub lock_until: i64,

    /// The last system time we recorded. Used for clock rollback detection.
    pub last_seen_time: i64,

    /// Monotonic counter — increments on every state change.
    pub monotonic_counter: u64,
}

impl LockState {
    /// Create a new default (unlocked) lock state.
    pub fn default_state() -> Self {
        Self {
            attempt_count: 0,
            lock_until: 0,
            last_seen_time: 0,
            monotonic_counter: 0,
        }
    }

    /// Check if the folder is currently locked at the given time.
    pub fn is_locked(&self, now: i64) -> bool {
        self.lock_until > now
    }

    /// Get remaining lock time in seconds.
    pub fn remaining_lock_seconds(&self, now: i64) -> i64 {
        if self.is_locked(now) {
            self.lock_until - now
        } else {
            0
        }
    }

    /// Calculate lock duration in seconds for a given attempt count.
    ///
    /// Formula: days = n², with n=1 exception → 2 days.
    pub fn lock_duration_seconds(attempt_count: u32) -> i64 {
        if attempt_count == 0 {
            return 0;
        }
        let days: i64 = if attempt_count == 1 {
            2
        } else {
            (attempt_count as i64).pow(2)
        };
        days * 24 * 3600
    }

    /// Calculate lock days for display.
    pub fn lock_duration_days(attempt_count: u32) -> u64 {
        if attempt_count == 0 {
            return 0;
        }
        if attempt_count == 1 {
            2
        } else {
            (attempt_count as u64).pow(2)
        }
    }

    /// Record a successful password attempt — resets all counters.
    pub fn record_success(&mut self, now: i64) {
        self.attempt_count = 0;
        self.lock_until = 0;
        self.last_seen_time = now;
        self.monotonic_counter += 1;
    }

    /// Record a failed password attempt — increments counter and sets lock.
    ///
    /// Returns the lock duration in seconds.
    pub fn record_failure(&mut self, now: i64) -> i64 {
        // Check for clock rollback first
        if now < self.last_seen_time && self.last_seen_time > 0 {
            let rollback_penalty = self.last_seen_time - now;
            // Extend existing lock by the rollback amount
            if self.lock_until > 0 {
                self.lock_until += rollback_penalty;
            }
        }

        self.attempt_count += 1;
        self.last_seen_time = now;
        self.monotonic_counter += 1;

        let lock_duration = Self::lock_duration_seconds(self.attempt_count);
        self.lock_until = now + lock_duration;

        lock_duration
    }

    /// Check for and handle clock rollback.
    ///
    /// If clock rollback is detected, the lock time is extended as a penalty.
    /// Returns the penalty in seconds (0 if no rollback).
    pub fn check_clock_rollback(&mut self, now: i64) -> i64 {
        if self.last_seen_time > 0 && now < self.last_seen_time {
            let penalty = self.last_seen_time - now;
            // Penalty: extend lock by the rollback amount
            if self.lock_until > 0 {
                self.lock_until += penalty;
            }
            self.last_seen_time = now;
            self.monotonic_counter += 1;
            return penalty;
        }
        0
    }
}

/// Simplified lock engine — wraps LockState with in/out from `.pw4lock` JSON.
pub struct LockEngine {
    state: LockState,
}

impl LockEngine {
    /// Create a new LockEngine with default (unlocked) state.
    pub fn new() -> Self {
        Self {
            state: LockState::default_state(),
        }
    }

    /// Create a LockEngine from an existing LockState (loaded from `.pw4lock`).
    pub fn from_state(state: LockState) -> Self {
        Self { state }
    }

    /// Get the current lock state (immutable reference).
    pub fn state(&self) -> &LockState {
        &self.state
    }

    /// Get a mutable reference to the lock state (for serialization).
    pub fn state_mut(&mut self) -> &mut LockState {
        &mut self.state
    }

    /// Take the lock state (consumes the engine).
    pub fn into_state(self) -> LockState {
        self.state
    }

    /// Check if the folder is currently locked at the given time.
    pub fn is_locked(&self, now: i64) -> bool {
        self.state.is_locked(now)
    }

    /// Get remaining lock time in seconds.
    pub fn remaining_lock(&self, now: i64) -> i64 {
        self.state.remaining_lock_seconds(now)
    }

    /// Get the current attempt count.
    pub fn attempt_count(&self) -> u32 {
        self.state.attempt_count
    }

    /// Check if max attempts have been reached.
    pub fn is_max_attempts(&self) -> bool {
        self.state.attempt_count >= MAX_ATTEMPTS
    }

    /// Record a successful unlock attempt. Resets all counters.
    pub fn record_success(&mut self, now: i64) {
        self.state.record_success(now);
    }

    /// Record a failed unlock attempt. Increments counter and sets lock.
    ///
    /// Returns the lock duration in seconds.
    pub fn record_failure(&mut self, now: i64) -> i64 {
        // Check clock rollback first
        let penalty = self.state.check_clock_rollback(now);
        if penalty > 0 {
            log::warn!(
                "Clock rollback detected: {}s penalty applied",
                penalty
            );
        }
        self.state.record_failure(now)
    }

    /// Get the current system time as Unix timestamp.
    pub fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_duration_formula() {
        assert_eq!(LockState::lock_duration_days(1), 2);
        assert_eq!(LockState::lock_duration_days(2), 4);
        assert_eq!(LockState::lock_duration_days(3), 9);
        assert_eq!(LockState::lock_duration_days(5), 25);
        assert_eq!(LockState::lock_duration_days(10), 100);
        assert_eq!(LockState::lock_duration_days(0), 0);
    }

    #[test]
    fn test_lock_state_lifecycle() {
        let mut state = LockState::default_state();
        let base_time = 1_700_000_000;

        // Initially unlocked
        assert!(!state.is_locked(base_time));

        // First failure → lock for 2 days
        let duration = state.record_failure(base_time);
        assert_eq!(duration, 2 * 24 * 3600);
        assert_eq!(state.attempt_count, 1);
        assert!(state.is_locked(base_time));
        assert!(state.is_locked(base_time + 2 * 24 * 3600 - 1));
        assert!(!state.is_locked(base_time + 2 * 24 * 3600 + 1));

        // Success resets everything
        state.record_success(base_time + 10);
        assert_eq!(state.attempt_count, 0);
        assert!(!state.is_locked(base_time + 10));
    }

    #[test]
    fn test_clock_rollback_detection() {
        let mut state = LockState::default_state();
        let t1 = 1_700_000_000;

        // Record at t1
        state.record_failure(t1);
        assert_eq!(state.last_seen_time, t1);

        // Clock rollback to t1 - 3600 (1 hour earlier)
        let t2 = t1 - 3600;
        let penalty = state.check_clock_rollback(t2);
        assert!(penalty > 0);
    }

    #[test]
    fn test_lock_engine() {
        let mut engine = LockEngine::new();
        let now = LockEngine::now();

        assert!(!engine.is_locked(now));

        engine.record_failure(now);
        assert_eq!(engine.attempt_count(), 1);

        engine.record_success(now + 10);
        assert_eq!(engine.attempt_count(), 0);
    }

    #[test]
    fn test_max_attempts_detection() {
        let mut state = LockState::default_state();
        state.attempt_count = 10;
        let engine = LockEngine::from_state(state);
        assert!(engine.is_max_attempts());
    }
}
