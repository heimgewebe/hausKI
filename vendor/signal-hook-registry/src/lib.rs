use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type SigId = usize;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[cfg(unix)]
pub const FORBIDDEN: &[i32] = &[libc::SIGKILL, libc::SIGSTOP];
#[cfg(not(unix))]
pub const FORBIDDEN: &[i32] = &[];

/// A minimal stand-in for the real signal registration API.
pub fn register<F>(_signal: i32, _action: F) -> io::Result<SigId>
where
    F: Fn() + Send + Sync + 'static,
{
    Ok(NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

pub fn unregister(_signal: i32, _id: SigId) -> io::Result<()> {
    Ok(())
}
