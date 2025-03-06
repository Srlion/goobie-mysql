use std::{
    self,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;
use gmod::{lua::*, rstruct::RStruct, task_queue::TaskQueue, *};
use sqlx::mysql::MySqlConnection;
use tokio::sync::mpsc;

mod connect;
mod disconnect;
mod options;
mod ping;
mod query;
pub mod state;

use options::Options as ConnectOptions;
use state::{AtomicState, State};

use crate::{cstr_from_args, print_goobie, run_async, GLOBAL_TABLE_NAME, GLOBAL_TABLE_NAME_C};

const META_TABLE_NAME: LuaCStr = cstr_from_args!(GLOBAL_TABLE_NAME, "_connection");

enum ConnMessage {
    Connect(LuaReference),
    Disconnect(LuaReference),
    Query(crate::query::Query),
    Ping(LuaReference),
    Close,
}

pub struct ConnMeta {
    // each connection needs a unique id for each inner connection
    // this is to be used for transactions to know if they are still in a transaction or not
    // if it's a new connection, it's not in a transaction, so it MUST forget about it
    // we don't use the state alone because it could switch back to Connected quickly and the
    // transaction would think it's still in a transaction
    id: AtomicUsize,
    state: AtomicState,
    opts: ConnectOptions,
    task_queue: TaskQueue,
}

impl ConnMeta {
    pub fn set_state(&self, state: State) {
        self.state.store(state, Ordering::Release);
    }
}

pub struct Conn {
    meta: Arc<ConnMeta>,
    sender: mpsc::UnboundedSender<ConnMessage>,
}

impl Conn {
    pub fn new(l: lua::State, opts: ConnectOptions) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();

        let conn = Conn {
            meta: Arc::new(ConnMeta {
                id: AtomicUsize::new(0),
                state: AtomicState::new(State::NotConnected),
                opts,
                task_queue: TaskQueue::new(l),
            }),
            sender,
        };

        let meta = conn.meta.clone();
        run_async(async move {
            let mut db_conn: Option<MySqlConnection> = None;
            while let Some(msg) = receiver.recv().await {
                match msg {
                    ConnMessage::Connect(callback) => {
                        // result is handed off to the query callback
                        let _ = connect::connect(&mut db_conn, &meta, callback).await;
                    }
                    ConnMessage::Disconnect(callback) => {
                        disconnect::disconnect(&mut db_conn, &meta, callback).await
                    }
                    ConnMessage::Query(query) => {
                        query::query(&mut db_conn, &meta, query).await;
                    }
                    ConnMessage::Ping(callback) => {
                        ping::ping(&mut db_conn, &meta, callback).await;
                    }
                    // This should be called after "disconnect"
                    ConnMessage::Close => {
                        break;
                    }
                }
            }
        });

        conn
    }

    #[inline]
    fn id(&self) -> usize {
        self.meta.id.load(Ordering::Acquire)
    }

    #[inline]
    fn state(&self) -> State {
        self.meta.state.load(Ordering::Acquire)
    }

    #[inline]
    fn poll(&self, l: lua::State) {
        self.meta.task_queue.poll(l);
    }
}

register_lua_rstruct!(
    Conn,
    META_TABLE_NAME,
    &[
        (c"Poll", poll),
        //
        (c"Start", start_connect),
        (c"Disconnect", start_disconnect),
        //
        (c"State", get_state),
        (c"Ping", ping),
        //
        (c"Run", run),
        (c"Execute", execute),
        (c"FetchOne", fetch_one),
        (c"Fetch", fetch),
        //
        (c"ID", get_id),
        (c"Host", get_host),
        (c"Port", get_port),
        //
        (c"__tostring", __tostring),
    ]
);

pub fn on_gmod_open(l: lua::State) {
    state::setup(l);

    l.get_global(GLOBAL_TABLE_NAME_C);
    l.get_metatable_name(META_TABLE_NAME);
    l.set_field(-2, c"CONN_META");
    l.pop(); // pop the global table
}

impl std::fmt::Display for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Goobie MySQL Connection [ID: {} | IP: {} | Port: {} | State: {}]",
            self.id(),
            self.meta.opts.inner.get_host(),
            self.meta.opts.inner.get_port(),
            self.state()
        )
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        print_goobie!("GCing connection!");
        let _ = self
            .sender
            .send(ConnMessage::Disconnect(LUA_NOREF));
        let _ = self.sender.send(ConnMessage::Close);
    }
}

#[lua_function]
pub fn new_conn(l: lua::State) -> Result<i32> {
    let mut opts = ConnectOptions::new();
    opts.parse(l)?;

    l.pop();

    let conn = Conn::new(l, opts);
    l.push_struct(conn);

    Ok(1)
}

#[lua_function]
fn poll(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    conn.poll(l);
    Ok(0)
}

#[lua_function]
fn start_connect(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    let callback_ref = l.check_function(2)?;

    let _ = conn
        .sender
        .send(ConnMessage::Connect(callback_ref));

    Ok(0)
}

#[lua_function]
fn start_disconnect(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    let callback_ref = l.check_function(2)?;

    let _ = conn
        .sender
        .send(ConnMessage::Disconnect(callback_ref));

    Ok(0)
}

fn start_query(l: lua::State, query_type: crate::query::QueryType) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;

    let query_str = l.check_string(2)?;
    let mut query = crate::query::Query::new(query_str, query_type);
    query.parse_options(l, 3)?;

    let _ = conn.sender.send(ConnMessage::Query(query));

    Ok(0)
}

#[lua_function]
fn run(l: lua::State) -> Result<i32> {
    start_query(l, crate::query::QueryType::Run)
}

#[lua_function]
fn execute(l: lua::State) -> Result<i32> {
    start_query(l, crate::query::QueryType::Execute)
}

#[lua_function]
fn fetch_one(l: lua::State) -> Result<i32> {
    start_query(l, crate::query::QueryType::FetchOne)
}

#[lua_function]
fn fetch(l: lua::State) -> Result<i32> {
    start_query(l, crate::query::QueryType::FetchAll)
}

#[lua_function]
fn get_state(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    l.push_number(conn.state().to_usize());
    Ok(1)
}

#[lua_function]
fn ping(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    let callback_ref = l.check_function(2)?;

    let _ = conn.sender.send(ConnMessage::Ping(callback_ref));

    Ok(0)
}

#[lua_function]
fn get_id(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    let id = conn.meta.id.load(Ordering::Acquire);
    l.push_number(id);
    Ok(1)
}

#[lua_function]
fn get_host(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    l.push_string(conn.meta.opts.inner.get_host());
    Ok(1)
}

#[lua_function]
fn get_port(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    l.push_number(conn.meta.opts.inner.get_port());
    Ok(1)
}

#[lua_function]
fn __tostring(l: lua::State) -> Result<i32> {
    let conn = l.get_struct::<Conn>(1)?;
    l.push_string(&conn.to_string());
    Ok(1)
}
