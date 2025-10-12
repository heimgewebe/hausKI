use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Ulid(u128);

impl Ulid {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
        // Compose a monotonic identifier using milliseconds precision and a counter.
        let value = (now.as_millis() << 16) ^ (counter & 0xFFFF);
        Ulid(value)
    }
}

impl fmt::Display for Ulid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}
