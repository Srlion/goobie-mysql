# Goobie MySQL

Goobie MySQL is a Rust library for Garry's Mod that provides a simple interface to MySQL databases using [sqlx](https://docs.rs/sqlx). It supports both synchronous and asynchronous queries, transactions, and efficient connection management.

## Features

- Simple API for interacting with MySQL databases.
- Supports both synchronous and asynchronous queries.
- Transactions implemented using coroutines for ease of use.
- Prepared and cached queries by default for security (thanks to sqlx).
- Supports raw queries for executing multiple statements.
- Graceful shutdown, waiting for pending queries (default timeout: 15 seconds).

## Table of Contents

- [Installation](#installation)
- [Getting Started](#getting-started)
  - [Connecting to the Database](#connecting-to-the-database)
  - [Executing Queries](#executing-queries)
  - [Transactions](#transactions)
- [API Reference](#api-reference)
  - [Globals](#globals)
  - [Error Table](#error-table)
  - [Query Options](#query-options)
  - [Connection Methods](#connection-methods)
  - [Transaction Methods](#transaction-methods)
- [Graceful Shutdown](#graceful-shutdown)
- [Future Plans](#future-plans)
- [ConVars](#convars)

## Installation

To install Goobie MySQL:

1. Download the latest version from the [GitHub releases](https://github.com/Srlion/goobie-mysql/releases/latest).
2. Extract the module to your Garry's Mod `lua/bin` directory.

**Note:** To avoid conflicts when multiple addons use different versions of the library, require the specific version you need:

```lua
require("goobie_mysql_0_1_0")
local goobie_mysql = goobie_mysql_0_1_0
```

When installing the library, ensure you select the version you intend to use.

## Getting Started

### Connecting to the Database

You can create a new connection using `goobie_mysql.NewConn`, which accepts either a URI string or a configuration table.

#### Examples

**Using a URI string:**

```lua
local conn = goobie_mysql.NewConn("mysql://user:password@localhost/database", {
    on_connected = function(conn)
        print("Connected to the database!")
    end,
    on_disconnected = function(conn, err)
        if err then
            print("Error during disconnect:", err.message)
        end
    end,
    on_error = function(conn, err)
        print("Connection error:", err.message)
    end,
})
```

**Using a configuration table:**

```lua
local conn = goobie_mysql.NewConn({
    host = "localhost",
    db = "database",
    user = "user",
    password = "password",
    port = 3306,
    on_connected = function(conn)
        print("Connected to the database!")
    end,
    on_disconnected = function(conn, err)
        if err then
            print("Error during disconnect:", err.message)
        end
    end,
    on_error = function(conn, err)
        print("Connection error:", err.message)
    end,
})
```

**Starting the Connection:**

- **Asynchronous start:**

  ```lua
  conn:Start()
  ```

  Calls `on_connected` if successful, or `on_error` if it fails.

- **Synchronous start:**

  ```lua
  conn:StartSync()
  ```

  Throws an error if it fails to connect.

### Executing Queries

#### `Execute` Method

Executes a query without returning any data (e.g., `INSERT`, `UPDATE`).

**Asynchronous execution:**

```lua
conn:Execute("INSERT INTO users (name, age) VALUES (?, ?)", {
    params = {"John Doe", 30},
    callback = function(err, res)
        if err then
            print("Error:", err.message)
            return
        end

        print("Affected Rows:", res.affected_rows)
        print("Insert ID:", res.insert_id)
    end,
})
```

**Synchronous execution:**

```lua
local err, res = conn:Execute("UPDATE users SET age = age + 1 WHERE id = ?", {
    params = {1},
    sync = true,
})
if err then
    print("Error:", err.message)
    -- Handle error
else
    print("Rows affected:", res.affected_rows)
    print("Insert ID:", res.insert_id)
end
```

**Note:** When using `raw = true`, the query is executed as-is without parameterization, allowing execution of multiple statements. Use cautiously to avoid SQL injection vulnerabilities.

#### `Fetch` Method

Fetches multiple rows from a `SELECT` query.

**Asynchronous fetch:**

```lua
conn:Fetch("SELECT * FROM users WHERE age > ?", {
    params = {20},
    callback = function(err, rows)
        if err then
            print("Error:", err.message)
            return
        end

        for _, row in ipairs(rows) do
            print("User:", row.name, "Age:", row.age)
        end
    end
})
```

**Synchronous fetch:**

```lua
local err, rows = conn:Fetch("SELECT * FROM users WHERE age > ?", {
    params = {20},
    sync = true,
})
if err then
    print("Error:", err.message)
else
    for _, row in ipairs(rows) do
        print("User:", row.name, "Age:", row.age)
    end
end
```

#### `FetchOne` Method

Fetches a single row from a `SELECT` query.

**Asynchronous fetch one:**

```lua
conn:FetchOne("SELECT * FROM users WHERE id = ?", {
    params = {1},
    callback = function(err, row)
        if err then
            print("Error:", err.message)
            return
        end

        if row then
            print("User:", row.name, "Age:", row.age)
        else
            print("No user found.")
        end
    end
})
```

**Synchronous fetch one:**

```lua
local err, row = conn:FetchOne("SELECT * FROM users WHERE id = ?", {
    params = {1},
    sync = true,
})
if err then
    print("Error:", err.message)
elseif row then
    print("User:", row.name, "Age:", row.age)
else
    print("No user found.")
end
```

### Transactions

Transactions allow you to execute multiple queries atomically.

**Starting a Transaction:**

```lua
conn:Begin(function(err, txn)
    if err then
        print("Error starting transaction:", err.message)
        return
    end

    -- Perform queries within the transaction
    local err, result = txn:Execute("INSERT INTO users (name) VALUES (?)", {
        params = {"Alice"},
    })
    if err then
        print("Error during insert:", err.message)
        return
    end

    print("Inserted Alice with ID:", result.insert_id)

    -- Commit the transaction
    local commit_err = txn:Commit()
    if commit_err then
        print("Error committing transaction:", commit_err.message)
    else
        print("Transaction committed successfully.")
    end
end)
```

[Notes on Transactions](#notes-on-transactions)

## API Reference

### Globals

#### `goobie_mysql.VERSION`

A string representing the version of the library.

Example:

```lua
print(goobie_mysql.VERSION) --> "0.1.0"
```

#### `goobie_mysql.STATES`

A table containing the connection states:

```lua
{
    CONNECTED = number,
    CONNECTING = number,
    NOT_CONNECTED = number,
    DISCONNECTED = number,
    ERROR = number,
}
```

#### `Poll`

Polls all pending tasks. You can use this function to wait for asynchronous tasks to complete.

You usually don't need to call this function directly, as you can already use `sync = true` with queries to make them synchronous.
I use it to test async queries in a synchronous environment, to verify that they work as expected.

```lua
local is_done = false
conn:Execute("SELECT 1", {
    callback = function()
        is_done = true
    },
})

while not is_done do
    goobie_mysql.Poll()
end

print("Query is done!")
```

### Error Table

All errors return a table containing the following fields:

| Key        | Type              | Description                                           |
| ---------- | ----------------- | ----------------------------------------------------- |
| `message`  | `string`          | The error message.                                    |
| `code`     | `number` or `nil` | MySQL error code (nil if not a MySQL error).          |
| `sqlstate` | `string` or `nil` | SQL state (nil if not a MySQL error or no SQL state). |

### Query Options

The following options can be used with `Execute`, `Fetch`, and `FetchOne` methods:

| Option     | Type       | Description                                                                                                                                         |
| ---------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sync`     | `boolean`  | If `true`, runs the query synchronously. Defaults to `false`.                                                                                       |
| `raw`      | `boolean`  | If `true`, executes the query as a raw SQL string without using prepared statements. Defaults to `false`. Useful for executing multiple statements. |
| `params`   | `table`    | Parameters for parameterized queries. Ignored if `raw = true`.                                                                                      |
| `callback` | `function` | Callback function invoked when the process is complete.                                                                                             |

**Notes:**

- When using `raw = true`, you can execute multiple statements in a single query.
- Be cautious with raw queries to avoid SQL injection attacks. Only use raw queries when necessary.
- Refer to the [Error Table](#error-table) for the structure of error objects passed to `callback`.

### Connection Methods

#### `goobie_mysql.NewConn`

Creates a new connection to the database.

**Signature:**

```lua
goobie_mysql.NewConn(config: string | table, options: table | nil) --> Connection
```

**Parameters:**

- **config**: Either a URI string or a configuration table.

  - **URI String Format:**

    ```
    mysql://[user[:password]@][host][:port]/[database][?properties]
    ```

  - **Configuration Table:**
    You can easily add the uri inside the table if you want to keep things simple.

    ```lua
    {
        ---------------------------------------
        uri = "mysql://user:password@localhost/database",

        -- OR

        host = "localhost",
        db = "database",
        user = "user",
        password = "password",
        port = 3306,
        ---------------------------------------

        charset = "utf8mb4", -- Default charset
        collation = "utf8mb4_0900_ai_ci", -- If you don't provide one then MySQL server will select the default one
        timezone = "UTC", -- Default timezone
        statement_cache_capacity = 100, -- Default statement cache capacity. Caching is handled using LRU, meaning when the amount of queries hits the defined limit, the oldest statement will get dropped.

        -- Event callbacks can be included here (see below)
    }
    ```

**Options (Event Callbacks):**

- **on_connected**: `function(conn: Connection) end` — Called when the connection is successfully established.
- **on_error**: `function(conn: Connection, err: Error_Table) end` — Called when an error occurs during connection.
- **on_disconnected**: `function(conn: Connection, err: Error_Table | nil) end` — Called when the connection is disconnected. If an error occurs during disconnect, it's passed as an argument.

**Notes:**

- If both `uri` and other parameters are supplied, `uri` will be used, and other parameters will be ignored.
- Properties in the URI can be found in the [sqlx MySQL ConnectOptions documentation](https://docs.rs/sqlx/latest/sqlx/mysql/struct.MySqlConnectOptions.html#properties).

#### `Start`

Starts the connection asynchronously.

```lua
conn:Start()
```

Calls `on_connected` or `on_error` based on the outcome.

#### `StartSync`

Starts the connection synchronously.

```lua
conn:StartSync()
```

Throws an error if it fails to connect.

#### `Disconnect`

Disconnects the connection asynchronously.

```lua
conn:Disconnect()
```

Calls `on_disconnected` with an error if one occurs.

#### `DisconnectSync`

Disconnects the connection synchronously.

```lua
local err = conn:DisconnectSync()
if err then
    print("Error during disconnect:", err.message)
end
```

Also calls `on_disconnected` with an error if one occurs.

#### `State`

Returns the current connection state.

> **Note:** Do **NOT** use numbers directly to check the state of the connection. Always use the provided constants. The numeric values of these constants may change in future versions, and using them directly could break your code.

```lua
local state = conn:State() -- compare it to goobie_mysql.STATES.*
```

#### `Ping`

Pings the database to check the connection status.

> **Note:** It's generally not recommended to use this method to check if a connection is alive, as it may not be reliable. For more information, refer to [this article](https://www.percona.com/blog/checking-for-a-live-database-connection-considered-harmful/).

```lua
local success, err = conn:Ping()
if not success then
    print("Error during ping:", err.message)
end
```

#### `Execute`

Executes a query without fetching data.

```lua
-- Asynchronous execution
conn:Execute(query: string, options: table | nil)

-- Synchronous execution
local err, result = conn:Execute(query: string, {
    sync = true,
    -- Additional options here
})
```

**Result:**

The `result` table contains:

```lua
{
    affected_rows = number, -- Number of rows affected.
    insert_id = number,     -- ID of the last inserted row.
}
```

#### `Fetch`

Fetches multiple rows from a `SELECT` query.

```lua
-- Asynchronous fetch
conn:Fetch(query: string, options: table | nil)

-- Synchronous fetch
local err, rows = conn:Fetch(query: string, {
    sync = true,
    -- Additional options here
})
```

**Result:**

An array of rows, where each row is a table.

#### `FetchOne`

Fetches a single row from a `SELECT` query.

```lua
-- Asynchronous fetch one
conn:FetchOne(query: string, options: table | nil)

-- Synchronous fetch one
local err, row = conn:FetchOne(query: string, {
    sync = true,
    -- Additional options here
})
```

**Result:**

A table representing a single row.

#### `Begin`

Starts a transaction asynchronously.

```lua
conn:Begin(function(err: Error_Table, txn: Transaction)
    -- Transaction code here
end)
```

#### `BeginSync`

Starts a transaction synchronously.

```lua
conn:BeginSync(function(err: Error_Table, txn: Transaction)
    -- Transaction code here
end)
```

### Transaction Methods

Within a transaction, you can execute queries and fetch data.

#### `IsOpen`

Returns whether the transaction is still open or not.

```lua
if txn:IsOpen() then
    print("Transaction is still open!")
end
```

#### `Ping`

Pings the database to check the connection status.

> **Note:** It's generally not recommended to use this method to check if a connection is alive, as it may not be reliable. For more information, refer to [this article](https://www.percona.com/blog/checking-for-a-live-database-connection-considered-harmful/).

```lua
local success, err = txn:Ping()
if not success then
    print("Error during ping:", err.message)
end
```

#### `Execute`

```lua
local err, result = txn:Execute(query: string, options: table | nil)
```

#### `Fetch`

```lua
local err, rows = txn:Fetch(query: string, options: table | nil)
```

#### `FetchOne`

```lua
local err, row = txn:FetchOne(query: string, options: table | nil)
```

#### `Commit`

Commits the transaction.

```lua
local err = txn:Commit()
if err then
    print("Error committing transaction:", err.message)
end
```

#### `Rollback`

Rolls back the transaction.

```lua
local err = txn:Rollback()
if err then
    print("Error rolling back transaction:", err.message)
end
```

#### Notes on Transactions

- Implemented using coroutines; transactions run like synchronous code.
- Always check for errors after each query inside a transaction. Transactions automatically roll back if Lua error occurs, or if `Commit`/`Rollback` is not called.
- After a rollback, the transaction cannot be used further.
- Transactions take a mutex lock on the connection. Commit or rollback as soon as possible to release the lock.
- **Do NOT** keep transactions open for a long time.
- **Do NOT** keep transactions open for a long time.
- Transaction queries do **not** accept callbacks; they return results directly.

## Graceful Shutdown

The library supports graceful shutdown by waiting for pending queries before shutting down. However, callbacks for those queries will **not** be called after shutdown. The default timeout is 10 seconds if queries are still pending.

## ConVars

- GOOBIE_MYSQL_WORKER_THREADS: Number of worker threads to use for async queries. Default is 2. You need to restart the server for changes to take effect.

## Future Plans

- Add support for nested transactions.
- Implement connection pooling.
- ~~Add support for running queries inside coroutines in Lua for greater flexibility.~~
  Will not be implemented. Working with coroutines in GLua is not the best thing to do, one mistake of forgetting that you are in a coroutine working with async code, can lead to a lot of issues.

## Note

This library is newly released and may contain bugs. Please report any issues you encounter!

Be aware that breaking changes may occur in future updates. Always check the changelog before updating to a new version.

---

```

```
