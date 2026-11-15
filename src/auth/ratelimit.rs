//! In-memory sliding-window rate limiter — same behavior as Python
//! `check_rate_limit`, including the 10 000-key eviction cap.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_KEYS: usize = 10_000;

pub struct RateLimiter {
    store: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` if allowed (and records the attempt), `false` if limited.
    ///
    /// Same semantics as Python `check_rate_limit`: every allowed call is
    /// recorded, so even successful requests count toward the window.
    pub fn check(&self, key: &str, max_attempts: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = self.lock();
        if Self::prune_limited(&mut map, key, max_attempts, window, now) {
            return false;
        }
        Self::push(&mut map, key, window, now);
        Self::evict(&mut map);
        true
    }

    /// Is `key` currently rate limited? Prunes the window but records nothing.
    pub fn is_limited(&self, key: &str, max_attempts: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = self.lock();
        Self::prune_limited(&mut map, key, max_attempts, window, now)
    }

    /// Record an attempt for `key` (used for failure-only limiter keys such as
    /// `subsonic:{user}`, where successes must not consume the budget).
    pub fn record(&self, key: &str, window: Duration) {
        let now = Instant::now();
        let mut map = self.lock();
        Self::push(&mut map, key, window, now);
        Self::evict(&mut map);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<Instant>>> {
        match self.store.lock() {
            Ok(m) => m,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Drop timestamps outside the window, then report whether the key is
    /// already at its attempt cap.
    fn prune_limited(
        map: &mut HashMap<String, Vec<Instant>>,
        key: &str,
        max_attempts: usize,
        window: Duration,
        now: Instant,
    ) -> bool {
        let times = map.entry(key.to_string()).or_default();
        times.retain(|&t| now.duration_since(t) < window);
        times.len() >= max_attempts
    }

    fn push(map: &mut HashMap<String, Vec<Instant>>, key: &str, window: Duration, now: Instant) {
        let times = map.entry(key.to_string()).or_default();
        times.retain(|&t| now.duration_since(t) < window);
        times.push(now);
    }

    /// Evict if too many distinct keys (Python's `_RATE_LIMIT_MAX_KEYS` cap).
    fn evict(map: &mut HashMap<String, Vec<Instant>>) {
        if map.len() <= MAX_KEYS {
            return;
        }
        // First drop keys whose windows have fully elapsed.
        let empty: Vec<String> = map
            .iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(k, _)| k.clone())
            .collect();
        for k in empty.iter().take(empty.len() / 2) {
            map.remove(k);
        }
        // Still too many → drop oldest-inserted half wholesale.
        if map.len() > MAX_KEYS {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.truncate(MAX_KEYS / 2);
            for k in keys {
                map.remove(&k);
            }
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max_then_blocks() {
        let rl = RateLimiter::new();
        for _ in 0..5 {
            assert!(rl.check("login:a", 5, Duration::from_secs(900)));
        }
        assert!(!rl.check("login:a", 5, Duration::from_secs(900)));
    }

    #[test]
    fn keys_are_independent() {
        let rl = RateLimiter::new();
        for _ in 0..5 {
            assert!(rl.check("login:a", 5, Duration::from_secs(900)));
        }
        assert!(!rl.check("login:a", 5, Duration::from_secs(900)));
        assert!(rl.check("login:b", 5, Duration::from_secs(900)));
    }

    #[test]
    fn window_expiry_allows_again() {
        let rl = RateLimiter::new();
        assert!(rl.check("k", 1, Duration::from_millis(10)));
        assert!(!rl.check("k", 1, Duration::from_millis(10)));
        std::thread::sleep(Duration::from_millis(20));
        assert!(rl.check("k", 1, Duration::from_millis(10)));
    }

    #[test]
    fn evicts_when_over_max_keys() {
        let rl = RateLimiter::new();
        // Insert well past the cap with a huge window so they stay "active".
        for i in 0..(MAX_KEYS + 50) {
            rl.check(&format!("key-{i}"), 10, Duration::from_secs(3600));
        }
        let len = rl.store.lock().unwrap().len();
        assert!(len <= MAX_KEYS, "expected <= {MAX_KEYS}, got {len}");
    }

    #[test]
    fn is_limited_checks_without_recording() {
        let rl = RateLimiter::new();
        // 50 cheap "checks" must not consume any budget.
        for _ in 0..50 {
            assert!(!rl.is_limited("subsonic:u", 5, Duration::from_secs(900)));
        }
        // Explicit records do consume it.
        for _ in 0..5 {
            rl.record("subsonic:u", Duration::from_secs(900));
        }
        assert!(rl.is_limited("subsonic:u", 5, Duration::from_secs(900)));
    }

    #[test]
    fn recorded_failures_expire_with_window() {
        let rl = RateLimiter::new();
        rl.record("k", Duration::from_millis(10));
        rl.record("k", Duration::from_millis(10));
        assert!(rl.is_limited("k", 2, Duration::from_millis(10)));
        std::thread::sleep(Duration::from_millis(20));
        assert!(!rl.is_limited("k", 2, Duration::from_millis(10)));
    }
}
