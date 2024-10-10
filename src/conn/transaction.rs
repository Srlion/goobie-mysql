use std::sync::{atomic::Ordering, Arc};

use anyhow::{bail, Result};
use gmod::{lua::*, *};
use sqlx::{Connection as _, Executor, MySqlConnection};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::{
    cstr_from_args,
    error::{handle_error, handle_sqlx_error},
    query::{Query, QueryType},
    run_async, wait_async, GLOBAL_TABLE_NAME,
};

use super::Conn;

const META_NAME: LuaCStr = cstr_from_args!(GLOBAL_TABLE_NAME, "_transaction");

pub const METHODS: &[LuaReg] = lua_regs![
    "IsOpen" => is_open,
    "Ping" => ping,

    "Execute" => execute,
    "FetchOne" => fetch_one,
    "Fetch" => fetch,

    "Commit" => commit,
    "Rollback" => rollback,

    "__gc" => __gc,
];

pub fn setup(l: lua::State) {
    // let meta_name = GLOBAL_TABLE_NAME_C.concat(META_NAME);
    l.new_metatable(META_NAME);
    {
        l.register(std::ptr::null(), METHODS.as_ptr());

        l.push_value(-1); // Pushes the metatable to the top of the stack
        l.set_field(-2, c"__index");
    }
    l.pop();
}

macro_rules! get_connection {
    ($mutex:expr, $ident:ident => $body:expr) => {{
        let conn_guard = $mutex
            .as_mut()
            .expect("Connection guard should exist when get_connection is called");

        let connection = conn_guard
            .as_mut()
            .expect("MySqlConnection should exist when get_connection is called");

        let $ident = connection;

        $body
    }};
}

#[derive(Debug)]
enum Action {
    Commit,
    Rollback,
}

#[repr(C)]
pub struct Transaction {
    conn: Arc<Conn>,
    conn_guard: Option<OwnedMutexGuard<Option<MySqlConnection>>>,
    coroutine_ref: i32,
    open: bool,
    sync: bool,
    finalizing: bool,
    traceback: String,
}

impl Transaction {
    pub async fn new(conn: Arc<Conn>, coroutine_ref: i32, traceback: String) -> Result<Self> {
        let mut conn_guard = conn.inner.clone().lock_owned().await;

        {
            let inner_conn = match conn_guard.as_mut() {
                Some(conn) => conn,
                None => {
                    bail!("connection is closed");
                }
            };

            inner_conn
                .execute("SET autocommit = 0; BEGIN;")
                .await?;
        }

        Ok(Transaction {
            conn,
            conn_guard: Some(conn_guard),
            coroutine_ref,
            open: true,
            sync: false,
            finalizing: false,
            traceback,
        })
    }

    #[inline]
    pub fn new_userdata(self, l: lua::State) -> Arc<Mutex<Self>> {
        // SAFETY: srlion gives you best safety
        let ud = Arc::new(Mutex::new(self));
        let ud_ptr: *const Mutex<Transaction> = Arc::into_raw(ud);
        l.new_userdata(ud_ptr, Some(META_NAME));
        unsafe {
            Arc::increment_strong_count(ud_ptr);
            Arc::from_raw(ud_ptr)
        }
    }

    #[inline]
    pub fn extract_userdata(l: lua::State) -> Result<Arc<Mutex<Self>>> {
        let txn_ptr = l.get_userdata::<*const Mutex<Self>>(1, Some(META_NAME))?;
        let txn_ptr = *txn_ptr;

        unsafe {
            Arc::increment_strong_count(txn_ptr);
        }

        let txn_mutex: Arc<Mutex<Transaction>> = unsafe { Arc::from_raw(txn_ptr) };
        {
            let txn = txn_mutex.blocking_lock();
            if !txn.is_open() {
                bail!("transaction is closed");
            }

            // let's make sure people don't try to access the transaction from outside the coroutine
            if l.push_thread() == 1 || {
                l.pop();
                l != get_coroutine(l, txn.coroutine_ref)
            } {
                // caught you b
                bail!("transaction can only be accessed from the coroutine it was created in");
            }
        }

        Ok(txn_mutex)
    }

    #[inline]
    pub fn extract_userdata_consumed(l: lua::State) -> Result<Arc<Mutex<Self>>> {
        let txn_ptr = l.get_userdata::<*const Mutex<Self>>(1, Some(META_NAME))?;
        let txn_mutex: Arc<Mutex<Transaction>> = unsafe { Arc::from_raw(*txn_ptr) };
        Ok(txn_mutex)
    }

    #[inline]
    fn resume(txn_mutex: Arc<Mutex<Self>>, co: lua::State, narg: i32, traceback: &str) {
        let res = if co.coroutine_status() != LUA_YIELD && co.coroutine_status() != LUA_OK {
            Ok(LUA_OK)
        } else {
            co.coroutine_resume_ignore(narg, Some(traceback))
        };
        match res {
            Ok(LUA_OK) | Err(_) => {
                run_async(async move {
                    let mut txn = txn_mutex.lock().await;
                    if txn.is_open() {
                        if let Ok(LUA_OK) = res {
                            eprintln!(
                                "[ERROR] forgot to finalize transaction!\n{}\n",
                                txn.traceback
                            );
                        }
                    }

                    let _ = txn.finalize(Action::Rollback).await;
                });
            }
            _ => {}
        };
    }

    #[inline]
    async fn finalize(&mut self, action: Action) -> Result<(), sqlx::Error> {
        if !self.open {
            return Ok(());
        }

        self.set_open(false);

        let res = get_connection!(self.conn_guard, conn => {
            let res = match action {
                Action::Commit => conn.execute("COMMIT;").await,
                Action::Rollback => conn.execute("ROLLBACK;").await,
            };

            let _ = conn.execute("SET autocommit = 1;").await;

            res
        });

        let _ = self.conn_guard.take(); // drop the connection guard

        self.conn
            .transaction_coroutine_ref
            .store(LUA_NOREF, Ordering::Release);

        res.map(|_| ())
    }

    #[inline]
    pub fn is_open(&self) -> bool {
        self.open && !self.finalizing
    }

    #[inline]
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
    }
}

impl std::fmt::Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Goobie.MySQL.Transaction")
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        let coroutine_ref = self.coroutine_ref;
        wait_lua_tick(self.traceback.clone(), move |l| {
            l.dereference(coroutine_ref);
        });
    }
}

fn internal_new(l: lua::State, sync: bool) -> Result<i32> {
    let traceback = l.get_traceback(l, 1).into_owned();
    let conn = Conn::extract_userdata(l)?;
    l.check_function(2)?;

    // we create a coroutine and pass the function to it
    let co = l.coroutine_new();
    l.push_value(2);
    l.coroutine_exchange(co, 1);
    let co_ref = l.reference();

    // this is to avoid deadlock when someone mistakenly tries to run a sync conn:query while in a transaction
    conn.transaction_coroutine_ref
        .store(co_ref, Ordering::Release);

    let traceback_clone = traceback.clone();
    let handle_new_txn = move |l: lua::State, txn: Result<Transaction>| match txn {
        Ok(txn) => {
            let co = get_coroutine(l, co_ref);
            co.push_nil();

            let txn_mutex: Arc<Mutex<Transaction>> = txn.new_userdata(co);
            {
                let mut txn = txn_mutex.blocking_lock();
                txn.sync = sync;
            }

            Transaction::resume(txn_mutex, co, 2, &traceback_clone);
        }
        Err(e) => {
            let co = get_coroutine(l, co_ref);
            handle_error(co, e);
            let _ = co.coroutine_resume_ignore(1, Some(&traceback_clone));
        }
    };

    let traceback = traceback.clone();
    if sync {
        let res = wait_async(l, Transaction::new(conn, co_ref, traceback.clone()));
        handle_new_txn(l, res);
    } else {
        run_async(async move {
            let res = Transaction::new(conn, co_ref, traceback.clone()).await;
            wait_lua_tick(traceback.clone(), move |l| handle_new_txn(l, res));
        });
    }

    Ok(0)
}

#[lua_function]
pub fn new(l: lua::State) -> Result<i32> {
    internal_new(l, false)
}

#[lua_function]
pub fn new_sync(l: lua::State) -> Result<i32> {
    internal_new(l, true)
}

#[lua_function]
fn is_open(l: lua::State) -> Result<i32> {
    match Transaction::extract_userdata(l) {
        Ok(_) => {
            // if it was closed, extract_userdata would have errored
            l.push_boolean(true);
        }
        Err(_) => {
            l.push_boolean(false);
        }
    };
    Ok(1)
}

#[lua_function]
fn ping(l: lua::State) -> Result<i32> {
    let txn_mutex = Transaction::extract_userdata(l)?;

    let res = wait_async(l, async move {
        let mut txn = txn_mutex.lock().await;
        get_connection!(txn.conn_guard, conn => conn.ping().await)
    });

    let res = match res {
        Ok(_) => {
            l.push_boolean(true);
            1
        }
        Err(e) => {
            l.push_boolean(false);
            handle_sqlx_error(l, e);
            2
        }
    };

    Ok(res)
}

fn internal_query(l: lua::State, query_type: QueryType) -> Result<i32> {
    let traceback = l.get_traceback(l, 1).into_owned();

    let txn_mutex = Transaction::extract_userdata(l)?;
    let (mut query, is_sync, coroutine_ref) = {
        let txn = txn_mutex.blocking_lock();

        let query = l.check_string(2)?;
        let mut query = Query::new(query.to_string(), query_type);
        query.parse_options(l, 3, false)?;

        (query, txn.sync, txn.coroutine_ref)
    };

    let txn_mutex_clone = txn_mutex.clone();

    if is_sync {
        let res = wait_async(l, async move {
            let mut txn = txn_mutex_clone.lock().await;

            let (res, query) = get_connection!(txn.conn_guard, conn => {
                let res = query.start(conn).await ;
                (res, query)
            });

            (res, query)
        });

        let (res, mut query) = res;
        return Ok(query.process_result(l, res, None));
    }

    run_async(async move {
        let res = {
            let mut txn = txn_mutex_clone.lock().await;
            let (res, query) =
                get_connection!(txn.conn_guard, conn => (query.start(conn).await, query));

            (res, query)
        };

        let (res, mut query) = res;
        wait_lua_tick(traceback.clone(), move |l| {
            let co = get_coroutine(l, coroutine_ref);
            let rets = query.process_result(co, res, Some(&traceback));
            Transaction::resume(txn_mutex_clone, co, rets, &traceback);
        });
    });

    Ok(l.coroutine_yield(0))
}

#[lua_function]
pub fn execute(l: lua::State) -> Result<i32> {
    internal_query(l, QueryType::Execute)
}

#[lua_function]
fn fetch_one(l: lua::State) -> Result<i32> {
    internal_query(l, QueryType::FetchOne)
}

#[lua_function]
fn fetch(l: lua::State) -> Result<i32> {
    internal_query(l, QueryType::FetchAll)
}

fn finalize(l: lua::State, action: Action) -> Result<i32> {
    let traceback = l.get_traceback(l, 1).into_owned();
    let txn_mutex = Transaction::extract_userdata(l)?;
    let is_sync = {
        let mut txn = txn_mutex.blocking_lock();
        txn.finalizing = true;
        txn.sync
    };

    if is_sync {
        let res = wait_async(l, async move {
            let mut txn = txn_mutex.lock().await;
            txn.finalize(action).await
        });
        return match res {
            Ok(_) => Ok(0),
            Err(e) => {
                handle_sqlx_error(l, e);
                Ok(1)
            }
        };
    } else {
        let coroutine_ref = {
            let txn = txn_mutex.blocking_lock();
            txn.coroutine_ref
        };

        run_async(async move {
            let res = {
                let mut txn = txn_mutex.lock().await;
                txn.finalize(action).await
            };

            // let txn = txn_mutex.blocking_lock();
            wait_lua_tick(traceback.clone(), move |l| {
                let co = get_coroutine(l, coroutine_ref);
                match res {
                    Ok(_) => {
                        Transaction::resume(txn_mutex, co, 0, &traceback);
                    }
                    Err(e) => {
                        handle_sqlx_error(l, e);
                        Transaction::resume(txn_mutex, co, 1, &traceback);
                    }
                };
            });
        });
    }

    Ok(l.coroutine_yield(0))
}

#[lua_function]
fn commit(l: lua::State) -> Result<i32> {
    finalize(l, Action::Commit)
}

#[lua_function]
fn rollback(l: lua::State) -> Result<i32> {
    finalize(l, Action::Rollback)
}

#[lua_function]
fn __gc(l: lua::State) -> i32 {
    // This will Drop the transaction (unless there are still references to it)
    let txn_mutex = match Transaction::extract_userdata_consumed(l) {
        Ok(txn) => txn,
        Err(_) => return 0,
    };

    // if gmod closed, then runtime is already closed too
    // this is a safety, normally __gc should be called before gmod13_close but it's gmod
    if !crate::is_gmod_closed() {
        run_async(async move {
            let mut txn = txn_mutex.lock().await;
            let _ = txn.finalize(Action::Rollback).await;
        });
    }

    0
}

pub(super) fn get_coroutine(l: lua::State, co_ref: i32) -> lua::State {
    l.from_reference(co_ref);
    let co = l.to_thread(-1);
    l.pop();
    co
}
