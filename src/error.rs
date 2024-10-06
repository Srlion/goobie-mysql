use gmod::*;
use sqlx::mysql::MySqlDatabaseError;

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

    handle_sqlx_error_internal(l, &e)
}
