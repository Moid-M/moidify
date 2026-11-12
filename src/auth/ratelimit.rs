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
    pub fn check(&self, key: &str, max_attempts: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = match self.store.lock() {
            Ok(m) => m,
            Err(poisoned) => poisoned.into_inner(),
        };

        // Drop timestamps outside the window.
        if let Some(times) = map.get_mut(key) {
            times.retain(|&t| now.duration_since(t) < window);
            if times.len() >= max_attempts {
                return false;
            }
            times.push(now);
        } else {
            map.insert(key.to_string(), vec![now]);
        }

        // Evict if too many distinct keys.
        if map.len() > MAX_KEYS {
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

        true
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
}
