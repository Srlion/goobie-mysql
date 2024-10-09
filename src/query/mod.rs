use anyhow::{bail, Result};
use gmod::*;
use sqlx::{Executor as _, MySqlConnection};

pub mod param;
pub mod process;
pub mod result;

pub use result::{QueryResult, QueryType};

use param::Param;
use process::{process_info, process_row, process_rows};

use crate::error::handle_error;

pub type Params = Vec<Param>;

#[derive(Debug)]
pub struct Query {
    pub query: String,
    pub r#type: QueryType,
    pub params: Params,
    pub callback: i32,
    pub sync: bool,
    pub raw: bool,
}

impl Query {
    pub fn new(query: String, r#type: QueryType) -> Self {
        Self {
            query,
            r#type,
            sync: true,
            raw: false,
            params: Vec::new(),
            callback: LUA_NOREF,
        }
    }

    pub fn parse_options(&mut self, l: lua::State, arg_n: i32, parse_fns: bool) -> Result<()> {
        if !l.is_none_or_nil(arg_n) {
            l.check_table(arg_n)?;
        } else {
            if parse_fns {
                self.sync = false;
            }
            return Ok(());
        }

        if l.get_field_type_or_nil(arg_n, c"params", LUA_TTABLE)? {
            self.bind_params(l)?
        }

        if parse_fns {
            if l.get_field_type_or_nil(arg_n, c"sync", LUA_TBOOLEAN)? {
                self.sync = l.get_boolean(-1);
                l.pop();
            } else {
                self.sync = false;
                self.parse_on_fns(l, arg_n)?;
            }
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
                    self.params.push(Param::String(s.to_owned()));
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

    fn parse_on_fns(&mut self, l: lua::State, arg_n: i32) -> Result<()> {
        if l.get_field_type_or_nil(arg_n, c"callback", LUA_TFUNCTION)? {
            self.callback = l.reference();
        }

        Ok(())
    }

    #[inline]
    pub async fn start<'q>(&mut self, conn: &'q mut MySqlConnection) -> Result<QueryResult> {
        let r#type = &self.r#type;
        if self.raw {
            handle_query(self.query.as_str(), conn, r#type).await
        } else {
            let mut query = sqlx::query(self.query.as_str());
            for param in self.params.drain(..) {
                match param {
                    Param::Number(n) => query = query.bind(n),
                    Param::String(s) => query = query.bind(s),
                    Param::Boolean(b) => query = query.bind(b),
                };
            }
            handle_query(query, conn, r#type).await
        }
    }

    pub fn process_result(
        &mut self,
        l: lua::State,
        res: Result<QueryResult>,
        traceback: Option<&str>,
    ) -> i32 {
        let res = match res {
            Ok(QueryResult::Execute(info)) => process_info(l, info),
            Ok(QueryResult::Row(row)) => process_row(l, row),
            Ok(QueryResult::Rows(rows)) => process_rows(l, &rows),
            Err(e) => Err(e),
        };

        let (returns_count, err_msg) = match res {
            Ok(0) => {
                l.push_nil();
                (1, None)
            }
            Ok(n) => {
                l.push_nil();
                l.insert(-n - 1);
                (n + 1, None)
            }
            Err(e) => {
                // handle_error pushes the error as a table to the stack
                let err_msg = handle_error(l, e);
                (1, Some(err_msg))
            }
        };

        if self.sync {
            return returns_count;
        }

        let (called_function, _) = l.pcall_ignore_function_ref(self.callback, returns_count, 0);
        // make sure that if there is an error, it doesn't go silent
        // can't combine these two if statements because it's not stabliized yet for using "if let" statement :)
        if !called_function {
            if let Some(err_msg) = err_msg {
                l.error_no_halt(&err_msg, traceback);
            }
        }

        l.dereference(self.callback);

        0
    }
}

async fn handle_query<'q, E>(
    query: E,
    conn: &'q mut MySqlConnection,
    query_type: &QueryType,
) -> Result<QueryResult>
where
    E: 'q + sqlx::Execute<'q, sqlx::MySql>,
{
    match query_type {
        QueryType::Execute => {
            let info = conn.execute(query).await?;
            Ok(QueryResult::Execute(info))
        }
        QueryType::FetchAll => {
            let rows = conn.fetch_all(query).await?;
            Ok(QueryResult::Rows(rows))
        }
        QueryType::FetchOne => {
            let row = conn.fetch_optional(query).await?;
            Ok(QueryResult::Row(row))
        }
    }
}
