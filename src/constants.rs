use const_format::{formatcp, str_index, str_replace};
use gmod::*;

use crate::cstr_from_args;

const fn index_of_dot(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'.' {
            return i;
        }
        i += 1;
    }
    s.len() // if no dot is found, return the length of the string
}

pub const VERSION: &str = str_index!(
    env!("CARGO_PKG_VERSION"),
    ..env!("CARGO_PKG_VERSION").len() - 2
);
pub const MAJOR_VERSION: &str = str_index!(
    env!("CARGO_PKG_VERSION"),
    ..index_of_dot(env!("CARGO_PKG_VERSION"))
);

pub const GLOBAL_TABLE_NAME: &str =
    formatcp!("goobie_mysql_{}", str_replace!(MAJOR_VERSION, ".", "_"));
pub const GLOBAL_TABLE_NAME_C: LuaCStr = cstr_from_args!(GLOBAL_TABLE_NAME);

// How many threads to use for the runtime
pub const DEFAULT_WORKER_THREADS: u16 = 1;

pub const DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT: u32 = 20;
