# Goobie MySQL

Goobie MySQL is a Rust library for Garry's Mod that provides a simple interface to MySQL databases using [sqlx](https://docs.rs/sqlx). It supports both synchronous and asynchronous queries, transactions, and handles connection management efficiently.

# Table of Contents

- [Introduction](#goobie-mysql)
- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [API Reference](#api-reference)
  - [Error Table](#error_table)
  - [Query Options](#query_options)
- [Graceful Shutdown](#graceful-shutdown)
- [Future Plans](#future-plans)

## Features

- Simple API for interacting with MySQL databases.
- Supports both synchronous and asynchronous queries.
- Transactions implemented using coroutines for easy use.
- Queries are prepared and cached by default for security. (Thanks to sqlx)
- **Supports raw queries for executing multiple statements.**
- Graceful shutdown, waiting for pending queries. (Default timeout: 15 seconds)

## Installation

To install Goobie MySQL, download the desired version from the [GitHub releases](https://github.com/your-repo/releases). Extract the module to your Garry's Mod `lua/bin` directory.

**Note:** To avoid conflicts when multiple addons use different versions of the library, you must require the specific version you need:

```lua
require("goobie_mysql_0_1_0")
local goobie_mysql = goobie_mysql_0_1_0
```

When installing the library, ensure you select the version you intend to use.

## Usage

### Connection

You can create a new connection using `goobie_mysql.NewConn`, which accepts either a URI string or a configuration table.

#### Examples

```lua
-- Using URI string
local conn = goobie_mysql.NewConn("mysql://user:password@localhost/database", {
    on_connected = function()
        print("Connected to the database!")
    end,
    on_disconnected = function(err)
        if err then
            print("Error during disconnect:", err.message)
        end
    end,
    on_error = function(err)
        print("Error during connect:", err.message)
    end,
})
```

```lua
-- Using configuration table with URI
local conn = goobie_mysql.NewConn({
    uri = "mysql://user:password@localhost/database",
    on_connected = function()
        print("Connected to the database!")
    end,
    on_disconnected = function(err)
        if err then
            print("Error during disconnect:", err.message)
        end
    end,
    on_error = function(err)
        print("Error during connect:", err.message)
    end,
})
```

```lua
-- Using configuration table with parameters
local conn = goobie_mysql.NewConn({
    host = "localhost",
    db = "database",
    user = "user",
    password = "password",
    port = 3306,
    on_connected = function()
        print("Connected to the database!")
    end,
    on_disconnected = function(err)
        if err then
            print("Error during disconnect:", err.message)
        end
    end,
    on_error = function(err)
        print("Error during connect:", err.message)
    end,
})
```

#### Starting the Connection

```lua
conn:Start()
```

Starts the connection asynchronously. It will call `on_connected` if successful, or `on_error` if it fails.

```lua
conn:StartSync()
```

Starts the connection synchronously. If it fails to connect, it will throw an error.

### Executing Queries

#### Execute

Executes a query without returning any data (e.g., INSERT, UPDATE). Supports both prepared statements and raw queries.

```lua
-- Asynchronous execution with prepared statements
conn:Execute("INSERT INTO users (name, age) VALUES (?, ?)", {
    params = {"John Doe", 30},
    on_done = function(result)
        print("Affected Rows:", result.affected_rows)
        print("Insert ID:", result.insert_id)
    end,
    on_error = function(err)
        print("Error:", err.message)
    end,
})
```

```lua
-- Synchronous execution
local err, result = conn:Execute("UPDATE users SET age = age + 1 WHERE id = 1; SELECT age FROM users WHERE id = 1;", {
    sync = true,
})
if err then
    print("Error:", err.message)
    -- Handle error
    return
end

print("Updated Age:", result.affected_rows)
```

**Note:** When using `raw = true`, the query is executed as-is without parameterization. This is useful for executing multiple statements in a single query. Be cautious when using raw queries to avoid SQL injection vulnerabilities.

#### Fetch

Fetches multiple rows from a SELECT query.

```lua
-- Asynchronous fetch
conn:Fetch("SELECT * FROM users WHERE age > ?", {
    params = {20},
    on_done = function(rows)
        for _, row in ipairs(rows) do
            print("User:", row.name, "Age:", row.age)
        end
    end,
    on_error = function(err)
        print("Error:", err.message)
    end,
})
```

```lua
-- Synchronous fetch
local err, rows = conn:Fetch("SELECT * FROM users WHERE age > ?", {
    sync = true,
    params = {20},
})
if err then
    print("Error:", err.message)
    -- Handle error
    return
end

for _, row in ipairs(rows) do
    print("User:", row.name, "Age:", row.age)
end
```

#### FetchOne

Fetches a single row from a SELECT query.

```lua
-- Asynchronous fetch one
conn:FetchOne("SELECT * FROM users WHERE id = ?", {
    params = {1},
    on_done = function(row)
        if row then
            print("User:", row.name, "Age:", row.age)
        else
            print("No user found.")
        end
    end,
    on_error = function(err)
        print("Error:", err.message)
    end,
})
```

```lua
-- Synchronous fetch one
local err, row = conn:FetchOne("SELECT * FROM users WHERE id = ?", {
    sync = true,
    params = {1},
})
if err then
    print("Error:", err.message)
    -- Handle error
    return
end

if row then
    print("User:", row.name, "Age:", row.age)
else
    print("No user found.")
end
```

### Transactions

Transactions allow you to execute multiple queries atomically.

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
        -- You don't need to rollback; it's done automatically
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

#### Notes on Transactions

- Transactions are implemented using coroutines, so they run like synchronous code.
- You **must** check for errors after each query inside a transaction.
- If an error occurs, the transaction is automatically rolled back.
- After a transaction is rolled back, you can no longer use it.
- Transactions lock the connection, so make sure to commit or rollback as soon as possible.

### Graceful Shutdown

The library supports graceful shutdown. It will wait for any pending queries to finish before shutting down. However, it won't run callbacks for those queries after shutdown.

## API Reference

### Error_Table

All errors are tables with the following keys:

```lua
{
    message = string,        -- The error message.
    code = number | nil,     -- MySQL error code (nil if not a MySQL error).
    sqlstate = string | nil, -- SQL state (nil if not a MySQL error or no SQL state).
}
```

### Query_Options

The following options can be used with `Execute`, `Fetch`, and `FetchOne` methods:

| Option     | Type       | Description                                                                                                                                         |
| ---------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sync`     | `boolean`  | If `true`, runs the query synchronously. Defaults to `false`.                                                                                       |
| `raw`      | `boolean`  | If `true`, executes the query as a raw SQL string without using prepared statements. Defaults to `false`. Useful for executing multiple statements. |
| `params`   | `table`    | Parameters for parameterized queries. Ignored if `raw = true`.                                                                                      |
| `on_done`  | `function` | Callback function called upon successful completion of the query.                                                                                   |
| `on_error` | `function` | Callback function called when an error occurs during the query execution.                                                                           |

#### Notes on Query Options

- When using `raw = true`, you can execute multiple statements in a single query.
- Be cautious when using raw queries to avoid SQL injection attacks. ONLY use raw queries when necessary.
- Refer to [Error_Table](#error_table) for the structure of error objects passed to `on_error`.

### `goobie_mysql.VERSION`

A string representing the version of the library. It's automatically incremented by GitHub during builds.

Example:

```lua
print(goobie_mysql.VERSION) --> "0.1.0"
```

### `goobie_mysql.STATES`

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

Can be used to check the connection state:

```lua
if conn.State() == goobie_mysql.STATES.CONNECTED then
    print("Connected!")
end
```

### `goobie_mysql.NewConn`

Creates a new connection to the database.

```lua
goobie_mysql.NewConn(config: string | table, options: table | nil) --> Connection
```

#### Parameters

- **config**: Either a URI string or a configuration table.

##### URI String Format

```
mysql://[user[:password]@][host][:port]/[database][?properties]
```

##### Configuration Table

You can provide a table with either:

- A `uri` key containing the URI string.
- Separate keys for connection parameters:

```lua
{
    host = "localhost",
    db = "database",
    user = "user",
    password = "password",
    port = 3306,
    -- Additional options cannot be set here; use the URI for that.
}
```

#### Options

You can supply event callbacks in the options table:

- **on_connected**: `function() end` — Called when the connection is successfully established.
- **on_error**: `function(err: Error_Table) end` — Called when an error occurs during connection.
- **on_disconnected**: `function(err: Error_Table | nil) end` — Called when the connection is disconnected. If an error occurs during disconnect, it's passed as an argument.

#### Notes

- If both `uri` and other parameters are supplied, `uri` will be used, and other parameters will be ignored.
- Properties in the URI can be found in the [sqlx MySQL ConnectOptions documentation](https://docs.rs/sqlx/0.8.2/sqlx/mysql/struct.MySqlConnectOptions.html#properties).

### Connection Methods

#### `Start`

```lua
conn:Start()
```

Starts the connection asynchronously. Calls `on_connected` or `on_error` as appropriate.

#### `StartSync`

```lua
conn:StartSync()
```

Starts the connection synchronously. Throws an error if it fails to connect.

#### `Disconnect`

```lua
conn:Disconnect()
```

Disconnects the connection asynchronously. Calls `on_disconnected` with an error if one occurs.

#### `DisconnectSync`

```lua
local err = conn:DisconnectSync()
if err then
    print("Error during disconnect:", err.message)
end
```

Disconnects the connection synchronously. Returns an error if one occurs.

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

##### Options

See [Query_Options](#query_options).

##### Result

The `result` table contains:

```lua
{
    affected_rows = number, -- Number of rows affected.
    insert_id = number,     -- ID of the last inserted row.
    rows = table | nil,     -- Only available if the query returns rows (e.g., SELECT in raw queries).
}
```

#### `Fetch`

Fetches multiple rows from a SELECT query.

```lua
-- Asynchronous fetch
conn:Fetch(query: string, options: table | nil)

-- Synchronous fetch
local err, rows = conn:Fetch(query: string, {
    sync = true,
    -- Additional options here
})
```

##### Options

See [Query_Options](#query_options).

##### Result

An array of rows, where each row is a table.

#### `FetchOne`

Fetches a single row from a SELECT query.

```lua
-- Asynchronous fetch one
conn:FetchOne(query: string, options: table | nil)

-- Synchronous fetch one
local err, row = conn:FetchOne(query: string, {
    sync = true,
    -- Additional options here
})
```

##### Options

See [Query_Options](#query_options).

##### Result

A table representing a single row.

#### `Begin`

Starts a transaction asynchronously.

```lua
conn:Begin(function(err: Error_Table, txn: Transaction)
    -- Transaction code here
end)
```

#### `BeginSync`

Starts a transaction synchronously. It works exactly like `Begin`, but queries are executed synchronously.

```lua
conn:BeginSync(function(err: Error_Table, txn: Transaction)
    -- Transaction code here
end)
```

### Transaction Methods

Within a transaction, you can execute queries and fetch data.

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

### Notes on Transactions

- Transaction queries do not accept callbacks; they return results directly.
- You **must** check for errors after each query.
- Transactions automatically rollback if a Query or Lua error occurs or if `Commit` | `Rollback` is not called.

## Graceful Shutdown

The library supports graceful shutdown by waiting for pending queries, and no callbacks will be called once they have been executed.
It times out after 10 seconds if queries are still pending.

---

# Future Plans

- Add support for inner transactions.
- Implement connection pooling.
- Add support for running queries inside Coroutines. It will be implemented in Lua instead of Rust because Lua offers greater flexibility.

# Note

This library is newly released and may contain bugs. Please report any issues you encounter!

Also, be aware that breaking changes could happen in future updates. Always check the changelog before updating to a new version.
