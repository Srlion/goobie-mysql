# Changelog

Most changes will be documented here.

## 1.0

Whole library rewritten to improve performance and fix some bugs.

- Switched from SemVer to Float-Versioning.
- Added `conn:Run`. This is just the same as `conn:Execute` but return no result at all. (This is for @morgverd the betch to to calm down about sqlite doing another query in `goobie-sql`)
- Removed `on_connected` & `on_disconnected` & `on_error` callbacks from `goobie_mysql.NewConn`.
- `conn:Start` and `conn:Disconnect` now take callbacks.
- Removed `sync` option from all queries, now you can use `conn:RunSync`, `conn:ExecuteSync`, `conn:FetchSync`, and `conn:FetchOneSync`.
- `conn:Ping` is now asynchronous. Use `conn:PingSync` for synchronous ping.
  - It also returns latency now, in microseconds.
- Removed `goobie_mysql.Poll`.
  - Use `conn:Poll` instead.
- Queries are now guaranteed to processed in order, this is to try to keep behavior consistent with sqlite in garry's mod.
- Added `goobie_mysql.MAJOR_VERSION`.
- Added `conn:ID` to get the connection ID. It's incremental for each inner connection. (You can test it by running `conn:StartSync()` multiple times).
- Added `conn:StateName` to get the current connection state as a string.
- Added **LuaLS** support, which can be found in `luals/goobie_mysql.lua`.
- Database now attempts to reconnect if connection is lost.
  - Transactions will play nicely incase it happens.
- Added `GOOBIE_MYSQL_GRACEFUL_SHUTDOWN_TIMEOUT` convar to control the timeout for graceful shutdown when restarting or closing the server.
- Query results are now processed before being sent to the lua thread, this should give a small boost incase of large result sets.
- Updated sqlx to `0.8.3`
