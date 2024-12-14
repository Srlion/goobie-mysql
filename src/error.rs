use anyhow::Result;
use gmod::*;
use sqlx::mysql::MySqlDatabaseError;

use crate::cstr_from_args;

const META_NAME: LuaCStr = cstr_from_args!(crate::GLOBAL_TABLE_NAME, "_error");

// call this function after creating a table
fn handle_database_error(l: lua::State, db_e: &MySqlDatabaseError) -> String {
    if let Some(sqlstate) = db_e.code() {
        l.push_string(sqlstate);
        l.set_field(-2, c"sqlstate");
    }

    l.push_number(db_e.number());
    l.set_field(-2, c"code");

    db_e.message().to_string()
}

// call this function after creating a table
fn handle_sqlx_error_internal(l: lua::State, e: &sqlx::Error) -> String {
    let msg = match e {
        sqlx::Error::Database(ref db_e) => match db_e.try_downcast_ref::<MySqlDatabaseError>() {
            Some(mysql_e) => handle_database_error(l, mysql_e),
            _ => e.to_string(),
        },
        _ => e.to_string(),
    };

    l.push_string(&msg);
    l.set_field(-2, c"message");

    msg
}

pub fn handle_error(l: lua::State, e: anyhow::Error) -> String {
    l.create_table(0, 3);
    l.get_metatable_name(META_NAME);
    unsafe { l.set_metatable(-2) };

    let msg = match e.downcast_ref::<sqlx::Error>() {
        Some(sqlx_e) => handle_sqlx_error_internal(l, sqlx_e),
        _ => e.to_string(),
    };

    l.push_string(&msg);
    l.set_field(-2, c"message");

    msg
}

pub fn handle_sqlx_error(l: lua::State, e: sqlx::Error) -> String {
    l.create_table(0, 3);
    l.get_metatable_name(META_NAME);
    unsafe { l.set_metatable(-2) };

    handle_sqlx_error_internal(l, &e)
}

#[lua_function]
fn __tostring(l: lua::State) -> Result<i32> {
    // retrieve the code field and the message field
    l.get_field(-1, c"code");
    let code = l.check_number(-1);
    l.pop();

    l.get_field(-1, c"message");
    let message = l.check_string(-1); // copy the string

    let message = message.or_else(|_| anyhow::Ok("unknown error".into()))?;
    if let Ok(code) = code {
        l.push_string(&format!("({}) {}", code, message));
    } else {
        l.push_string(&message);
    }

    Ok(1)
}

pub fn init(l: lua::State) {
    l.new_metatable(META_NAME);
    {
        l.push_function(__tostring);
        l.set_field(-2, c"__tostring");
    }
    l.pop();
}
