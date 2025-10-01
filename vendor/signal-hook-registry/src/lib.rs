use libc::{self, c_int};
use std::{
    io,
    mem,
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        OnceLock,
    },
};

pub const FORBIDDEN: [c_int; 2] = [libc::SIGKILL, libc::SIGSTOP];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalId {
    signal: c_int,
}

type Callback = dyn Fn() + Send + Sync + 'static;

struct CallbackStorage {
    callbacks: Vec<&'static Callback>,
}

impl CallbackStorage {
    fn new() -> Self {
        Self { callbacks: Vec::new() }
    }

    fn iter(&self) -> impl Iterator<Item = &&'static Callback> {
        self.callbacks.iter()
    }
}

struct SignalSlot {
    callbacks: AtomicPtr<CallbackStorage>,
    installed: AtomicBool,
}

impl SignalSlot {
    const fn new() -> Self {
        Self {
            callbacks: AtomicPtr::new(std::ptr::null_mut()),
            installed: AtomicBool::new(false),
        }
    }

    fn load(&self) -> Option<&'static CallbackStorage> {
        let ptr = self.callbacks.load(Ordering::SeqCst);
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &*ptr })
        }
    }

    fn push(&self, cb: &'static Callback) {
        let mut storage = CallbackStorage::new();
        if let Some(existing) = self.load() {
            storage.callbacks.extend(existing.iter().copied());
        }
        storage.callbacks.push(cb);
        let boxed = Box::new(storage);
        let ptr = Box::into_raw(boxed);
        let old = self.callbacks.swap(ptr, Ordering::SeqCst);
        if !old.is_null() {
            std::mem::forget(unsafe { Box::from_raw(old) });
        }
    }

    fn mark_installed(&self) -> bool {
        !self.installed.swap(true, Ordering::SeqCst)
    }
}

struct Registry {
    slots: Vec<SignalSlot>,
}

impl Registry {
    fn new() -> Self {
        let len = (libc::SIGRTMAX() + 1) as usize;
        let mut slots = Vec::with_capacity(len);
        for _ in 0..len {
            slots.push(SignalSlot::new());
        }
        Self { slots }
    }

    fn slot(&self, signal: c_int) -> Option<&SignalSlot> {
        if signal <= 0 {
            return None;
        }
        let idx = signal as usize;
        self.slots.get(idx)
    }
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(Registry::new)
}

extern "C" fn dispatch(signal: c_int) {
    unsafe {
        handler(signal);
    }
}

unsafe extern "C" fn handler(signal: c_int) {
    if let Some(registry) = REGISTRY.get() {
        if let Some(slot) = registry.slot(signal) {
            if let Some(storage) = slot.load() {
                for callback in storage.iter() {
                    callback();
                }
            }
        }
    }
}

pub fn register<F>(signal: c_int, action: F) -> io::Result<SignalId>
where
    F: Fn() + Send + Sync + 'static,
{
    if FORBIDDEN.contains(&signal) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot register handler for forbidden signal",
        ));
    }

    let registry = registry();
    let slot = registry
        .slot(signal)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid signal"))?;

    let cb: Box<Callback> = Box::new(action);
    let leaked: &'static Callback = Box::leak(cb);
    slot.push(leaked);

    if slot.mark_installed() {
        install_handler(signal)?;
    }

    Ok(SignalId { signal })
}

fn install_handler(signal: c_int) -> io::Result<()> {
    unsafe {
        let mut sa: libc::sigaction = mem::zeroed();
        sa.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut sa.sa_mask);
        sa.sa_sigaction = dispatch as usize;
        let ret = libc::sigaction(signal, &sa, std::ptr::null_mut());
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
