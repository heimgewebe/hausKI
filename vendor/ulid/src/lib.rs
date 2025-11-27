use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Ulid(u128);

impl Ulid {
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u128)
            .unwrap_or_default();
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = (COUNTER.fetch_add(1, Ordering::Relaxed) as u128) & 0xffff_ffff_ffff;
        let value = (timestamp << 48) | counter;
        Ulid(value)
    }
}

impl fmt::Display for Ulid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl fmt::Debug for Ulid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ulid({:032x})", self.0)
    }
}
