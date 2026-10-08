use std::{
    thread::sleep,
    time::{Duration, Instant},
};

pub struct RateLimiter {
    pub rate: f64, // bytes/second
    pub capacity: f64,
    pub tokens: f64,
    pub last_update: Instant,
}

impl RateLimiter {
    pub fn new(bandwidth: u64, burst: u64) -> Self {
        assert!(bandwidth > 0);
        assert!(burst > 0);

        Self {
            rate: bandwidth as f64,
            capacity: burst as f64,
            tokens: burst as f64,
            last_update: Instant::now(),
        }
    }

    pub fn max_chunk_size(&self) -> usize {
        self.capacity as usize
    }

    pub fn refill_by(&mut self, elapsed: Duration) {
        self.tokens = (self.tokens + elapsed.as_secs_f64() * self.rate).min(self.capacity);
    }

    pub fn refill(&mut self) {
        let now = Instant::now();

        let elapsed = now.duration_since(self.last_update);

        self.refill_by(elapsed);
        self.last_update = now;
    }

    pub fn consume_after(&mut self, elapsed: Duration, bytes: usize) -> Duration {
        self.refill_by(elapsed);

        let required = bytes as f64;
        if self.tokens >= required {
            self.tokens -= required;
            return Duration::ZERO;
        }

        let missing = required - self.tokens;

        self.tokens = 0.0;

        Duration::from_secs_f64(missing / self.rate)
    }

    pub fn consume(&mut self, bytes: usize) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);

        let wait = self.consume_after(elapsed, bytes);

        if !wait.is_zero() {
            sleep(wait);
        }

        self.last_update = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_bucket_starts_full() {
        let limiter = RateLimiter::new(1000, 10);

        assert_eq!(limiter.rate, 1000.0);
        assert_eq!(limiter.capacity, 100.0);
        assert_eq!(limiter.tokens, 100.0);
    }

    #[test]
    fn refill_adds_tokens() {
        let mut limiter = RateLimiter::new(1000, 10);
        limiter.tokens = 0.0;
        limiter.refill_by(Duration::from_millis(50));

        assert_eq!(limiter.tokens, 50.0);
    }

    #[test]
    fn refill_does_not_excced_capacity() {
        let mut limiter = RateLimiter::new(1000, 10);

        limiter.tokens = 90.0;
        limiter.refill_by(Duration::from_millis(100));
        assert_eq!(limiter.tokens, 100.0);
    }

    #[test]
    fn consume_without_wait_when_tokens_are_enough() {
        let mut limiter = RateLimiter::new(1000, 10);

        // 初始 capacity = 100 bytes
        let wait = limiter.consume_after(Duration::ZERO, 40);

        assert_eq!(wait, Duration::ZERO);
        assert_eq!(limiter.tokens, 60.0);
    }

    #[test]
    fn consume_returns_wait_when_tokens_are_insufficient() {
        let mut limiter = RateLimiter::new(1000, 10);

        // 初始 100 tokens
        let wait = limiter.consume_after(Duration::ZERO, 150);

        // 缺 50 bytes
        // 1000 bytes/s
        // 50 / 1000 = 0.05 s
        assert_eq!(wait, Duration::from_millis(50));
        assert_eq!(limiter.tokens, 0.0);
    }

    #[test]
    fn elapsed_time_refills_before_consuming() {
        let mut limiter = RateLimiter::new(1000, 10);

        limiter.tokens = 0.0;

        // 经过 50ms -> 补 50 tokens
        // 消耗 30 -> 剩 20
        let wait = limiter.consume_after(Duration::from_millis(50), 30);

        assert_eq!(wait, Duration::ZERO);
        assert_eq!(limiter.tokens, 20.0);
    }
}
