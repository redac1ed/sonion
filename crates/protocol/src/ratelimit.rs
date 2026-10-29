use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_TRACKED: usize = 10_000;

pub struct RateLimiter { 
    burst: f64, 
    per_sec: f64,
    buckets: Mutex<HashMap<IpAddr, Bucket>>
}

struct Bucket {
    tokens: f64,
    last: Instant
}

impl RateLimiter {
    pub fn new(burst: u32, per_sec: f64) -> Self {
        Self {
            burst: burst.max(1) as f64,
            per_sec, 
            buckets: Mutex::new(HashMap::new())
        }
    }
    pub fn try_acquire(&self, ip: IpAddr) -> Result<(), Duration> {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap();
        if map.len() >= MAX_TRACKED {
            map.retain(|_, b| self.refilled(b, now) < self.burst);
        }
        let b = map.entry(key(ip)).or_insert(Bucket {
            tokens: self.burst,
            last: now
        });
        b.tokens = self.refilled(b, now);
        b.last = now;
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            return Ok(());
        }
        let wait = if self.per_sec > 0.0 { (1.0 - b.tokens) / self.per_sec } else { 3600.0 };
        Err(Duration::from_secs_f64(wait.max(1.0)))
    }
    fn refilled(&self, b: &Bucket, now: Instant) -> f64 {
        (b.tokens + now.duration_since(b.last).as_secs_f64() * self.per_sec).min(self.burst)
    }
}

fn key(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let s = v6.segments();
            IpAddr::V6(Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0))
        }
        v4 => v4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] 
    fn allows_burst_then_limits() {
        let rl = RateLimiter::new(3, 0.0);
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(rl.try_acquire(ip).is_ok());
        assert!(rl.try_acquire(ip).is_ok());
        assert!(rl.try_acquire(ip).is_ok());
        assert!(rl.try_acquire(ip).is_err());
        assert!(rl.try_acquire("10.0.0.2".parse().unwrap()).is_ok());
    }
    #[test]
    fn ipv6_shares_a_bucket_per_64() {
        let rl = RateLimiter::new(1, 0.0);
        assert!(rl.try_acquire("2001:db8::1".parse().unwrap()).is_ok());
        assert!(rl.try_acquire("2001:db8::2".parse().unwrap()).is_err());
        assert!(rl.try_acquire("2001:db9::1".parse().unwrap()).is_ok());
    }
}