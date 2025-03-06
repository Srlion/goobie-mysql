use anyhow::{bail, Result};
use gmod::{push_to_lua::PushToLua, *};
use sqlx::{Executor as _, MySqlConnection};

pub mod process;
pub mod result;

pub use result::QueryResult;

use process::{convert_row, convert_rows};

use crate::error::handle_error;

#[derive(Debug)]
pub enum QueryType {
    Run,
    Execute,
    FetchOne,
    FetchAll,
}

#[derive(Debug, Clone)]
pub enum Param {
    Number(i32),
    String(Vec<u8>),
    Boolean(bool),
}

#[derive(Debug)]
pub struct Query {
    pub query: String,
    pub r#type: QueryType,
    pub params: Vec<Param>,
    pub callback: LuaReference,
    pub raw: bool,
    pub result: Result<QueryResult>,
}

impl Query {
    pub fn new(query: String, r#type: QueryType) -> Self {
        Self {
            query,
            r#type,
            raw: false,
            params: Vec::new(),
            callback: LUA_NOREF,
            result: Ok(QueryResult::Run), // we just need a placeholder
        }
    }

    pub fn parse_options(&mut self, l: lua::State, arg_n: i32) -> Result<()> {
        if !l.is_none_or_nil(arg_n) {
            l.check_table(arg_n)?;
        } else {
            return Ok(());
        }

        if l.get_field_type_or_nil(arg_n, c"params", LUA_TTABLE)? {
            self.bind_params(l)?
        }

        if l.get_field_type_or_nil(arg_n, c"callback", LUA_TFUNCTION)? {
            self.callback = l.reference();
        }

        if l.get_field_type_or_nil(arg_n, c"raw", LUA_TBOOLEAN)? {
            self.raw = l.get_boolean(-1);
            l.pop();
        }

        Ok(())
    }

    pub fn bind_params(&mut self, l: lua::State) -> Result<()> {
        for i in 1..=l.len(-1) {
            l.raw_geti(-1, i);
            match l.lua_type(-1) {
                LUA_TNUMBER => {
                    let num = l.to_number(-1);
                    self.params.push(Param::Number(num as i32));
                }
                LUA_TSTRING => {
                    // SAFETY: We just checked the type
                    let s = l.get_binary_string(-1).unwrap();
                    self.params.push(Param::String(s));
                }
                LUA_TBOOLEAN => {
                    let b = l.get_boolean(-1);
                    self.params.push(Param::Boolean(b));
                }
                _ => {
                    bail!(
                        "Unsupported type for parameter {}: {}",
                        i,
                        l.lua_type_name(-1)
                    );
                }
            }

            l.pop();
        }
        Ok(())
    }

    #[inline]
    pub async fn start(&mut self, conn: &'_ mut MySqlConnection) {
        let r#type = &self.r#type;
        if self.raw {
            // &str gets treated as raw query in sqlx
            self.result = handle_query(self.query.as_str(), conn, r#type).await;
        } else {
            let mut query = sqlx::query(self.query.as_str());
            for param in self.params.drain(..) {
                match param {
                    Param::Number(n) => query = query.bind(n),
                    Param::String(s) => query = query.bind(s),
                    Param::Boolean(b) => query = query.bind(b),
                };
            }
            self.result = handle_query(query, conn, r#type).await;
        }
    }

    pub fn process_result(&mut self, l: lua::State) {
        l.pcall_ignore_func_ref(self.callback.as_static(), || {
            match &self.result {
                Ok(query_result) => {
                    query_result.push_to_lua(&l);
                }
                Err(e) => {
                    handle_error(&l, e);
                }
            };
            0
        });
    }
}

async fn handle_query<'a, 'q, E>(
    query: E,
    conn: &'q mut MySqlConnection,
    query_type: &QueryType,
) -> Result<QueryResult>
where
    E: 'q + sqlx::Execute<'q, sqlx::MySql>,
{
    match query_type {
        QueryType::Run => {
            conn.execute(query).await?;
            Ok(QueryResult::Run)
        }
        QueryType::Execute => {
            let info = conn.execute(query).await?;
            Ok(QueryResult::Execute(info))
        }
        QueryType::FetchAll => {
            let rows = conn.fetch_all(query).await?;
            let rows = convert_rows(&rows);
            Ok(QueryResult::Rows(rows))
        }
        QueryType::FetchOne => {
            let row = conn.fetch_optional(query).await?;
            let row = convert_row(&row);
            Ok(QueryResult::Row(row))
        }
    }
}
