// SPDX-License-Identifier: GPL-3.0-or-later

//! Apple's limit on the Search API, "approximately 20 calls per minute":
//! a sliding window of calls, which says how long to wait rather than
//! calling past it.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Calls allowed in any [`WINDOW`].
pub const CALLS_PER_MINUTE: usize = 20;
pub const WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct RateLimiter {
    calls: usize,
    window: Duration,
    /// When each call in the window was made, oldest first.
    made: VecDeque<Instant>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(CALLS_PER_MINUTE, WINDOW)
    }
}

impl RateLimiter {
    pub fn new(calls: usize, window: Duration) -> Self {
        Self {
            calls,
            window,
            made: VecDeque::with_capacity(calls),
        }
    }

    /// Count a call made at `now`, or say how long until one may be made.
    pub fn acquire(&mut self, now: Instant) -> Result<(), Duration> {
        while let Some(&oldest) = self.made.front() {
            if now.saturating_duration_since(oldest) >= self.window {
                self.made.pop_front();
            } else {
                break;
            }
        }
        if self.made.len() < self.calls {
            self.made.push_back(now);
            return Ok(());
        }
        let oldest = self.made.front().copied().unwrap_or(now);
        Err(self.window - now.saturating_duration_since(oldest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_twenty_calls_a_minute_then_says_how_long_to_wait() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        for i in 0..20 {
            assert!(limiter.acquire(start + Duration::from_secs(i)).is_ok());
        }
        let wait = limiter
            .acquire(start + Duration::from_secs(30))
            .unwrap_err();
        assert_eq!(wait, Duration::from_secs(30));
        // A refused call is not counted.
        assert_eq!(
            limiter
                .acquire(start + Duration::from_secs(59))
                .unwrap_err(),
            Duration::from_secs(1)
        );
        // The first call leaves the window after a minute, making room for one.
        assert!(limiter.acquire(start + Duration::from_secs(60)).is_ok());
        assert!(limiter.acquire(start + Duration::from_secs(60)).is_err());
        assert!(limiter.acquire(start + Duration::from_secs(61)).is_ok());
    }

    #[test]
    fn a_quiet_minute_empties_the_window() {
        let start = Instant::now();
        let mut limiter = RateLimiter::new(2, WINDOW);
        assert!(limiter.acquire(start).is_ok());
        assert!(limiter.acquire(start).is_ok());
        assert!(limiter.acquire(start).is_err());
        let later = start + Duration::from_secs(120);
        assert!(limiter.acquire(later).is_ok());
        assert!(limiter.acquire(later).is_ok());
        assert!(limiter.acquire(later).is_err());
    }
}
