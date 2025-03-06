use gmod::*;
use sqlx::mysql::MySqlDatabaseError;

use crate::GLOBAL_TABLE_NAME_C;

// call this function after creating a table
fn handle_database_error(l: &lua::State, db_e: &MySqlDatabaseError) -> String {
    if let Some(sqlstate) = db_e.code() {
        l.push_string(sqlstate);
        l.set_field(-2, c"sqlstate");
    }

    l.push_number(db_e.number());
    l.set_field(-2, c"code");

    db_e.message().to_string()
}

// call this function after creating a table
fn handle_sqlx_error_internal(l: &lua::State, e: &sqlx::Error) {
    let msg = match e {
        sqlx::Error::Database(ref db_e) => match db_e.try_downcast_ref::<MySqlDatabaseError>() {
            Some(mysql_e) => handle_database_error(l, mysql_e),
            _ => e.to_string(),
        },
        _ => e.to_string(),
    };

    l.push_string(&msg);
    l.set_field(-2, c"message");
}

pub fn handle_error(l: &lua::State, e: &anyhow::Error) {
    l.create_table(0, 3);

    l.get_global(GLOBAL_TABLE_NAME_C);
    if l.is_table(-1) {
        l.get_field(-1, c"ERROR_META");
        l.dump_stack();
        l.set_metatable(-3);
    }
    l.pop();

    match e.downcast_ref::<sqlx::Error>() {
        Some(sqlx_e) => {
            handle_sqlx_error_internal(l, sqlx_e);
        }
        _ => {
            let err_msg = e.to_string();
            l.push_string(&err_msg);
            l.set_field(-2, c"message");
        }
    };
}
