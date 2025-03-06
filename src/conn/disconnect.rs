use std::{self, sync::Arc};

use gmod::lua::*;
use sqlx::{mysql::MySqlConnection, Connection};

use super::{state::State, ConnMeta};
use crate::error::handle_error;

#[inline(always)]
pub async fn disconnect(
    conn: &mut Option<MySqlConnection>,
    meta: &Arc<ConnMeta>,
    callback: LuaReference,
) {
    meta.set_state(State::Disconnected);

    let res = match conn.take() {
        Some(old_conn) => old_conn.close().await,
        None => Ok(()),
    };

    meta.task_queue.add(move |l| {
        match res {
            Ok(_) => {
                l.pcall_ignore_func_ref(callback, || 0);
            }
            Err(e) => {
                l.pcall_ignore_func_ref(callback, || {
                    handle_error(&l, &e.into()); // this will push the error to the stack
                    0
                });
            }
        };
    });
}
