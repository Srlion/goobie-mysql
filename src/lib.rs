use gmod::*;

mod conn;
mod constants;
mod error;
mod query;
mod runtime;

pub use constants::*;
pub use runtime::{run_async, wait_async};

pub static mut GMOD_CLOSED: bool = false;

#[inline]
pub fn is_gmod_closed() -> bool {
    unsafe { GMOD_CLOSED }
}

#[gmod13_open]
fn gmod13_open(l: lua::State) -> i32 {
    // this is for hosting servers that don't reclaim memory on map changes
    unsafe {
        GMOD_CLOSED = false;
    }

    runtime::load(get_max_worker_threads(l));

    conn::on_gmod_open::init(l);

    0
}

#[gmod13_close]
fn gmod13_close(l: lua::State) -> i32 {
    unsafe {
        GMOD_CLOSED = true;
    }

    runtime::unload();

    0
}

fn get_max_worker_threads(l: lua::State) -> u16 {
    let mut max_worker_threads = DEFAULT_WORKER_THREADS;

    l.get_global(c"CreateConVar");
    if l.is_function(-1) {
        {
            l.push_string("GOOBIE_MYSQL_WORKER_THREADS");
            l.push_number(DEFAULT_WORKER_THREADS);
            l.create_table(2, 0);
            {
                l.get_global(c"FCVAR_ARCHIVE");
                l.raw_seti(-2, 1);

                l.get_global(c"FCVAR_PROTECTED");
                l.raw_seti(-2, 2);
            }
            l.push_string("Number of worker threads for the mysql connection pool");
        }

        if l.pcall(4, 1, 0).is_ok() {
            l.get_field(-1, c"GetInt");
            {
                l.push_value(-2);
            }
            if l.pcall(1, 1, 0).is_ok() {
                max_worker_threads = l.to_number(-1) as u16;
                l.pop(); // pop the number
            } else {
                l.pop(); // pop the error
            }
            l.pop(); // pop the convar
        } else {
            l.pop(); // pop the error
        }
    } else {
        l.pop(); // pop the nil or whatever non function value
    }

    max_worker_threads
}

#[macro_export]
macro_rules! print_goobie {
    ($($arg:tt)*) => {
        println!("Goobie MySQL (v{}): {}", $crate::VERSION, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! cstr_from_args {
    ($($arg:expr),+) => {{
        use std::ffi::{c_char, CStr};
        const BYTES: &[u8] = constcat::concat!($($arg),+, "\0").as_bytes();
        let ptr: *const c_char = BYTES.as_ptr().cast();
        unsafe { CStr::from_ptr(ptr) }
    }};
}
