use std::{self, sync::Arc};

use gmod::lua::*;
use sqlx::{mysql::MySqlConnection, Connection};

use super::ConnMeta;
use crate::error::handle_error;

#[inline(always)]
pub async fn ping(
    conn: &mut Option<MySqlConnection>,
    meta: &Arc<ConnMeta>,
    callback: LuaReference,
) {
    let conn = match conn {
        Some(conn) => conn,
        None => {
            meta.task_queue.add(move |l| {
                l.pcall_ignore_func_ref(callback, || {
                    handle_error(&l, &anyhow::anyhow!("connection is not open"));
                    0
                });
            });
            return;
        }
    };
    let start = tokio::time::Instant::now();
    let res = conn.ping().await;
    let latency = start.elapsed().as_micros() as f64;
    meta.task_queue.add(move |l| {
        match res {
            Ok(_) => {
                l.pcall_ignore_func_ref(callback, || {
                    l.push_nil(); // err is nil
                    l.push_number(latency);
                    0
                });
            }
            Err(e) => {
                l.pcall_ignore_func_ref(callback, || {
                    handle_error(&l, &e.into());
                    0
                });
            }
        };
    });
}
