use anyhow::Result;
use gmod::push_to_lua::PushToLua;
use sqlx::mysql::MySqlQueryResult;

use crate::error::handle_error;

use super::process::ColumnValue;

#[derive(Debug)]
pub enum QueryResult {
    Run,
    Execute(MySqlQueryResult),
    Rows(Result<Vec<Vec<ColumnValue>>>),
    Row(Result<Option<Vec<ColumnValue>>>), // Option is used incase of no row was found
}

impl PushToLua for QueryResult {
    fn push_to_lua(&self, l: &gmod::State) {
        use QueryResult::*;
        match self {
            Run => {} // nothing to push
            Execute(info) => {
                l.push_nil(); // error is nil
                l.create_table(0, 2);
                {
                    l.push_number(info.rows_affected());
                    l.set_field(-2, c"rows_affected");

                    l.push_number(info.last_insert_id());
                    l.set_field(-2, c"last_insert_id");
                }
            }
            Rows(rows) => {
                let rows = match rows {
                    Ok(rows) => rows,
                    Err(e) => {
                        handle_error(l, e);
                        return;
                    }
                };

                l.push_nil(); // error is nil
                l.create_table(rows.len() as i32, 0);
                for (idx, row) in rows.iter().enumerate() {
                    l.create_table(0, row.len() as i32);
                    for value in row.iter() {
                        value.push_to_lua(l);
                        l.set_field(-2, &value.column_name);
                    }
                    l.raw_seti(-2, idx as i32 + 1);
                }
            }
            Row(row) => {
                let row = match row {
                    Ok(Some(row)) => row,
                    Ok(None) => return,
                    Err(e) => {
                        handle_error(l, e);
                        return;
                    }
                };

                l.push_nil(); // error is nil
                l.create_table(0, row.len() as i32);
                for value in row.iter() {
                    value.push_to_lua(l);
                    l.set_field(-2, &value.column_name);
                }
            }
        }
    }
}
