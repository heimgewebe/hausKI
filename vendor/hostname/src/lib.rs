use std::ffi::OsString;
use std::io;
use std::sync::OnceLock;

static HOSTNAME: OnceLock<OsString> = OnceLock::new();

pub fn get() -> io::Result<OsString> {
    let value = HOSTNAME
        .get_or_init(|| {
            std::env::var_os("HOSTNAME").unwrap_or_else(|| OsString::from("localhost"))
        })
        .clone();
    Ok(value)
}
