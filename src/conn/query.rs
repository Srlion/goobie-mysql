use gmod::*;
use sqlx::{mysql::MySqlConnection, Connection};
use std::{
    self,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use crate::{error::handle_error, print_goobie_with_host};

use super::{state::State, ConnMeta};

fn should_reconnect(e: &anyhow::Error) -> bool {
    let sqlx_e = match e.downcast_ref::<sqlx::Error>() {
        Some(e) => e,
        None => return false,
    };
    match sqlx_e {
        sqlx::Error::Io(io_err) => {
            let conn_dropped = matches!(
                io_err.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::NotConnected
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::UnexpectedEof
            );
            conn_dropped
        }
        sqlx::Error::Tls(tls_err) => {
            tls_err.to_string().contains("handshake failed")
                || tls_err.to_string().contains("connection closed")
                || tls_err.to_string().contains("unexpected EOF")
        }
        sqlx::Error::Database(db_err) => {
            if let Some(mysql_err) = db_err.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>() {
                let code = mysql_err.number();
                let connection_dropped = matches!(
                    code,
                    2002  // Can't connect to local MySQL server (socket issues)
                        | 2003  // Can't connect to MySQL server on 'hostname' (network issues)
                        | 2006  // MySQL server has gone away
                        | 2013  // Lost connection during query
                        | 2055 // Lost connection with system error
                );
                connection_dropped
            } else {
                false
            }
        }
        _ => false,
    }
}

#[inline(always)]
pub async fn query(
    conn: &mut Option<MySqlConnection>,
    meta: &Arc<ConnMeta>,
    mut query: crate::query::Query,
) {
    let db_conn = match conn {
        Some(conn) => conn,
        None => {
            meta.task_queue.add(move |l| {
                l.pcall_ignore_func_ref(query.callback, || {
                    handle_error(&l, &anyhow::anyhow!("connection is not open"));
                    0
                });
            });
            return;
        }
    };
    query.start(db_conn).await;

    let should_reconnect = {
        if let Err(e) = query.result.as_ref() {
            let should = should_reconnect(e);
            // we need to actually ping the connection, as extra validation that the connection is actually dead to not mess up with any queries
            if should && db_conn.ping().await.is_err() {
                // make sure that it's set before we return back to lua
                // this is a MUST because if we are inside a transaction and reconnect, lua MUST forget about the transaction
                // it can cause issues if we reconnect and lua thinks it's still in a transaction
                // we do it by changing the state AND having a unique id for each inner connection
                // this way a transaction can check the state AND the id to know if it's still in a transaction
                // if it's not, it can forget about it completely
                meta.state
                    .store(State::NotConnected, Ordering::Release);
                print_goobie_with_host!(
                    meta.opts.inner.get_host(),
                    "Database connection is lost, reconnecting..."
                );
            }
            should
        } else {
            false
        }
    };

    // if we should reconnect, we need to let lua know that there is an error so it can handle it
    meta.task_queue
        .add(move |l| query.process_result(l));

    if !should_reconnect {
        return;
    }

    let mut delay = Duration::from_secs(2);
    let mut reconnected = false;
    for _ in 0..7 {
        tokio::time::sleep(delay).await;
        delay += Duration::from_secs(1);
        if super::connect::connect(conn, meta, LUA_NOREF).await {
            print_goobie_with_host!(meta.opts.inner.get_host(), "Reconnected!");
            reconnected = true;
            break;
        } else {
            print_goobie_with_host!(
                meta.opts.inner.get_host(),
                "Failed to reconnect, retrying in {} seconds...",
                delay.as_secs()
            );
        }
    }
    if !reconnected {
        print_goobie_with_host!(
            meta.opts.inner.get_host(),
            "Failed to reconnect, giving up!",
        );
    }
}
