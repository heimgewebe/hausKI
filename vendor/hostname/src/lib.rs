use std::env;
use std::ffi::OsString;
use std::io;

pub fn get() -> Result<OsString, io::Error> {
    if let Ok(value) = env::var("HOSTNAME") {
        return Ok(OsString::from(value));
    }
    if let Ok(value) = env::var("COMPUTERNAME") {
        return Ok(OsString::from(value));
    }
    Ok(OsString::from("unknown"))
}
