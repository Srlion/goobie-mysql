use std::{
    self,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};
use gmod::{lua::*, *};
use sqlx::{mysql::MySqlConnection, Connection};
use tokio::sync::Mutex;

pub mod on_gmod_open;
mod options;
mod state;
mod transaction;

use options::Options as ConnectOptions;
use state::{AtomicState, State};

use crate::{cstr_from_args, error::handle_error, query, run_async, wait_async, GLOBAL_TABLE_NAME};

const META_NAME: LuaCStr = cstr_from_args!(GLOBAL_TABLE_NAME, "_connection");

// Used in on_gmod_open.rs
pub const METHODS: &[LuaReg] = lua_regs![
    "Start" => start_connect,
    "StartSync" => start_connect_sync,

    "Disconnect" => start_disconnect,
    "DisconnectSync" => start_disconnect_sync,

    "State" => get_state,
    "Ping" => ping,

    "Execute" => execute,
    "FetchOne" => fetch_one,
    "Fetch" => fetch,

    "Begin" => transaction::new,
    "BeginSync" => transaction::new_sync,

    "IsConnected" => is_connected,
    "IsConnecting" => is_connecting,
    "IsDisconnected" => is_disconnected,
    "IsError" => is_error,

    "__tostring" => __tostring,
    "__gc" => __gc,
];

#[repr(C)]
pub struct Conn {
    pub inner: Arc<Mutex<Option<MySqlConnection>>>,
    pub connect_options: ConnectOptions,
    pub state: AtomicState,
    pub traceback: String,

    // this is to avoid deadlock when someone mistakenly tries to run a sync conn:query while in a transaction
    pub transaction_coroutine_ref: AtomicI32, // if any transaction is running
}

impl Conn {
    pub fn new(opts: ConnectOptions, traceback: String) -> Self {
        Conn {
            inner: Arc::default(),
            connect_options: opts,
            state: AtomicState::new(State::NotConnected),
            traceback,
            transaction_coroutine_ref: AtomicI32::new(LUA_NOREF),
        }
    }

    #[inline]
    pub fn new_userdata(self, l: lua::State) {
        let ud = Arc::new(self);
        let ud = Arc::into_raw(ud);
        l.new_userdata(ud, Some(META_NAME));
    }

    #[inline]
    pub fn extract_userdata(l: lua::State) -> Result<Arc<Self>> {
        let conn_ptr = l.get_userdata::<*const Self>(1, Some(META_NAME))?;
        let conn_ptr = *conn_ptr;

        unsafe {
            Arc::increment_strong_count(conn_ptr);
        }

        let conn = unsafe { Arc::from_raw(conn_ptr) };
        {
            let transaction_coroutine_ref = conn
                .transaction_coroutine_ref
                .load(Ordering::Acquire);

            if transaction_coroutine_ref != LUA_NOREF
                && l == transaction::get_coroutine(l, transaction_coroutine_ref)
            {
                bail!("DEADLOCK DETECTED: cannot run a query in a transaction while it's running");
            }
        }
        Ok(conn)
    }

    #[inline]
    pub fn extract_userdata_no_lock(l: lua::State) -> Result<Arc<Self>> {
        let conn_ptr = l.get_userdata::<*const Self>(1, Some(META_NAME))?;
        let conn_ptr = *conn_ptr;

        unsafe {
            Arc::increment_strong_count(conn_ptr);
        }

        let conn = unsafe { Arc::from_raw(conn_ptr) };
        Ok(conn)
    }

    #[inline]
    pub fn extract_userdata_consumed(l: lua::State) -> Result<Arc<Self>> {
        let conn_ptr = l.get_userdata::<*const Self>(1, Some(META_NAME))?;
        let conn = unsafe { Arc::from_raw(*conn_ptr) };
        Ok(conn)
    }

    #[inline]
    pub async fn start(&self) -> Result<()> {
        let state = self.state();
        if state == State::Connecting {
            return Ok(());
        }

        let mut inner_conn_mutex = self.inner.lock().await;
        let mut inner_conn = inner_conn_mutex.take();

        if let Some(conn) = inner_conn.take() {
            // let's gracefully close the connection if there is any
            // if it fails, we will still try to connect. we just try to close it
            let _ = conn.close().await;
        }

        self.set_state(State::Connecting);

        let connect_opts = &self.connect_options.inner;

        match MySqlConnection::connect_with(connect_opts).await {
            Ok(conn) => {
                inner_conn_mutex.replace(conn);
            }
            Err(e) => {
                self.set_state(State::Error);
                return Err(e.into());
            }
        };

        self.set_state(State::Connected);

        Ok(())
    }

    #[inline]
    pub async fn disconnect(&self) -> Result<()> {
        let mut inner_conn = self.inner.lock().await;

        let state = self.state();
        if state != State::Connected {
            return Ok(());
        }

        // even though conn.close could fail, it will still be disconnected so it's better to
        // mark it before attempting to close
        self.set_state(State::Disconnected);

        if let Some(conn) = inner_conn.take() {
            conn.close().await?;
        }

        Ok(())
    }

    #[inline]
    fn state(&self) -> State {
        self.state.load(Ordering::Acquire)
    }

    #[inline]
    fn set_state(&self, state: State) {
        self.state.store(state, Ordering::Release);
    }

    #[inline]
    async fn ping(&self) -> Result<()> {
        let mut inner_conn = self.inner.lock().await;
        let inner_conn = match inner_conn.as_mut() {
            Some(conn) => conn,
            None => bail!("connection is not established"),
        };

        inner_conn.ping().await?;

        Ok(())
    }
}

impl std::fmt::Display for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Goobie MySQL Connection ({})", self.state(),)
    }
}

// impl Drop for Conn {
//     fn drop(&mut self) {
//         println!("Dropping connection");
//     }
// }

#[lua_function]
fn new(l: lua::State) -> Result<i32> {
    let traceback = l.get_traceback(l, 1).into_owned();

    let mut opts = ConnectOptions::new();
    opts.parse(l, true)?;

    let conn = Conn::new(opts, traceback);
    conn.new_userdata(l);

    Ok(1)
}

#[lua_function]
fn start_connect(l: lua::State) -> Result<i32> {
    let traceback = l.get_traceback(l, 1).into_owned();
    let conn = Conn::extract_userdata(l)?;

    // this is dumb but works lol
    l.push_value(1); // push the connection userdata
    let conn_ref = l.reference();
    let on_connected = conn.connect_options.on_connected;
    let on_error = conn.connect_options.on_error;

    run_async(async move {
        let res = conn.start().await;
        wait_lua_tick(traceback.clone(), move |l| {
            match res {
                Ok(_) => {
                    l.from_reference(conn_ref); // push the connection userdata
                    l.pcall_ignore_function_ref(on_connected, 1, 0);
                }
                Err(e) => {
                    l.from_reference(conn_ref); // push the connection userdata
                    let msg = handle_error(l, e);
                    let (called_function, _) = l.pcall_ignore_function_ref(on_error, 2, 0);
                    if !called_function {
                        l.error_no_halt(&msg, Some(&traceback));
                    }
                }
            };

            l.dereference(conn_ref);
        });
    });

    Ok(0)
}

#[lua_function]
fn start_connect_sync(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata(l)?;
    wait_async(l, async move { conn.start().await })?;
    Ok(0)
}

#[lua_function]
fn start_disconnect(l: lua::State) -> Result<i32> {
    let traceback = l.get_traceback(l, 1).into_owned();
    let conn = Conn::extract_userdata(l)?;

    // this is dumb but works lol
    l.push_value(1); // push the connection userdata
    let conn_ref = l.reference();
    let on_disconnected = conn.connect_options.on_disconnected;

    run_async(async move {
        let res = conn.disconnect().await;
        wait_lua_tick(traceback.clone(), move |l| {
            match res {
                Ok(_) => {
                    l.from_reference(conn_ref); // push the connection userdata
                    l.pcall_ignore_function_ref(on_disconnected, 1, 0);
                }
                Err(e) => {
                    l.from_reference(conn_ref); // push the connection userdata
                    let msg = handle_error(l, e);
                    l.pcall_ignore_function_ref(on_disconnected, 2, 0);
                    l.error_no_halt(&msg, Some(&traceback));
                }
            };

            l.dereference(conn_ref);
        });
    });

    Ok(0)
}

#[lua_function]
fn start_disconnect_sync(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata(l)?;
    let res = wait_async(l, async move { conn.disconnect().await });
    if let Err(e) = res {
        handle_error(l, e);
        return Ok(1);
    }
    Ok(0)
}

async fn internal_query(conn: Arc<Conn>, query: &mut query::Query) -> Result<query::QueryResult> {
    let mut inner_conn_mutex = conn.inner.lock().await;
    let inner_conn = match inner_conn_mutex.as_mut() {
        Some(conn) => conn,
        None => bail!("connection is not established"),
    };
    query.start(inner_conn).await
}

fn start_query(l: lua::State, query_type: query::QueryType) -> Result<i32> {
    let traceback = l.get_traceback(l, 1).into_owned();
    let conn = Conn::extract_userdata(l)?;

    let query_str = l.check_string(2)?.to_string();
    let mut query = query::Query::new(query_str, query_type);
    query.parse_options(l, 3, true)?;

    if query.sync {
        let (mut query, res) = wait_async(l, async move {
            let res = internal_query(conn, &mut query).await;
            (query, res)
        });
        return Ok(query.process_result(l, res, None));
    }

    run_async(async move {
        let res = internal_query(conn, &mut query).await;
        wait_lua_tick(traceback.clone(), move |l| {
            query.process_result(l, res, Some(&traceback));
        });
    });

    Ok(0)
}

#[lua_function]
fn execute(l: lua::State) -> Result<i32> {
    start_query(l, query::QueryType::Execute)
}

#[lua_function]
fn fetch_one(l: lua::State) -> Result<i32> {
    start_query(l, query::QueryType::FetchOne)
}

#[lua_function]
fn fetch(l: lua::State) -> Result<i32> {
    start_query(l, query::QueryType::FetchAll)
}

#[lua_function]
fn is_connected(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata_no_lock(l)?;
    l.push_bool(conn.state() == State::Connected);
    Ok(1)
}

#[lua_function]
fn is_connecting(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata_no_lock(l)?;
    l.push_bool(conn.state() == State::Connecting);
    Ok(1)
}

#[lua_function]
fn is_disconnected(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata_no_lock(l)?;
    l.push_bool(conn.state() == State::Disconnected);
    Ok(1)
}

#[lua_function]
fn is_error(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata_no_lock(l)?;
    l.push_bool(conn.state() == State::Error);
    Ok(1)
}

#[lua_function]
fn get_state(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata_no_lock(l)?;
    l.push_number(conn.state() as i32);
    Ok(1)
}

#[lua_function]
fn ping(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata(l)?;

    let res = wait_async(l, async move { conn.ping().await });
    match res {
        Ok(_) => {
            l.push_bool(true);
            Ok(1)
        }
        Err(e) => {
            l.push_bool(false);
            handle_error(l, e);
            Ok(2)
        }
    }
}

#[lua_function]
fn __tostring(l: lua::State) -> Result<i32> {
    let conn = Conn::extract_userdata_no_lock(l)?;
    l.push_string(&conn.to_string());
    Ok(1)
}

#[lua_function]
fn __gc(l: lua::State) -> Result<i32> {
    // this will Drop the connection (unless there are still references to it)
    let conn = match Conn::extract_userdata_consumed(l) {
        Ok(conn) => conn,
        Err(_) => {
            return Ok(0);
        }
    };

    let ConnectOptions {
        on_connected,
        on_error,
        on_disconnected,
        ..
    } = conn.connect_options;

    // if gmod closed, then runtime is already closed too
    // this is a safety, normally __gc should be called before gmod13_close but it's GMOD
    if !crate::is_gmod_closed() {
        let conn_cloned = conn.clone();
        run_async(async move {
            let traceback = conn_cloned.traceback.clone();

            {
                // let's wait for any pending operations to finish, as the database could be disconnecting right now
                let _ = conn_cloned.inner.lock().await;
                //

                if conn_cloned.state() == State::Disconnected {
                    return;
                }
            };

            let res = conn_cloned.disconnect().await;
            if let Err(e) = res {
                let err = e.to_string();
                wait_lua_tick(traceback.clone(), move |l| {
                    l.error_no_halt(&err, Some(&traceback));
                });
            }
        });
    }

    l.dereference(on_connected);
    l.dereference(on_error);
    l.dereference(on_disconnected);

    Ok(0)
}
