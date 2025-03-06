use std::{
    self,
    sync::{atomic::Ordering, Arc},
};

use gmod::lua::*;
use sqlx::{mysql::MySqlConnection, Connection};

use super::{state::State, ConnMeta};
use crate::error::handle_error;

#[inline(always)]
pub async fn connect(
    conn: &mut Option<MySqlConnection>,
    meta: &Arc<ConnMeta>,
    callback: LuaReference,
) -> bool {
    if let Some(old_conn) = conn.take() {
        // let's gracefully close the connection if there is any
        // we don't care if it fails, as we are reconnecting anyway
        let _ = old_conn.close().await;
    }

    meta.set_state(State::Connecting);

    let res = match MySqlConnection::connect_with(&meta.opts.inner).await {
        Ok(new_conn) => {
            *conn = Some(new_conn);
            meta.id.fetch_add(1, Ordering::Release); // increment the id
            meta.set_state(State::Connected);
            Ok(())
        }
        Err(e) => {
            meta.set_state(State::NotConnected);
            Err(e)
        }
    };

    if callback == LUA_NOREF {
        match res {
            Ok(_) => return true,
            Err(_) => return false,
        };
    }

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

    true
}
