# Goobie MySQL

A simple and easy-to-use MySQL client for Garry's Mod.

## Version Information

The module exposes version constants that you can use to verify or log the library version:

- **`goobie_mysql.VERSION`**: Contains the complete version string.
- **`goobie_mysql.MAJOR_VERSION`**: Contains the major version number.

## LuaLS

You can now use [LuaLS](https://github.com/LuaLS/lua-language-server) to get autocomplete and syntax highlighting for the library!

You can find the meta file in `luals/goobie_mysql.lua` (Don't ask me how to install because I don't know).

## Installation

1. Download the latest version from the [GitHub releases](https://github.com/Srlion/goobie-mysql/releases/latest).
2. Extract the module to your Garry's Mod `lua/bin` directory.

**Note:** To avoid conflicts when multiple addons use different versions of the library, require the specific version you need:

```lua
require("goobie_mysql_1")

---@type goobie_mysql
local goobie_mysql = goobie_mysql_1
```

When installing the library, ensure you select the version you intend to use.

Previously, it was using SemVer, but I've changed it to use Float-Versioning.

## Establishing a Connection

Create a connection object using `goobie_mysql.NewConn`. You can specify connection options in two ways:

### Using a URI

#### URI Format

The URI format is `mysql://[user[:password]@][host][:port]/[database][?properties]`.

```lua
local conn = goobie_mysql.NewConn({
    uri = "mysql://username:password@host:port/database"
})
```

### Using Individual Credentials

```lua
local conn = goobie_mysql.NewConn({
    host = "localhost",
    username = "user",
    password = "pass",
    database = "mydb"
})
```

Additional (optional) settings include:

- `charset` (e.g., `"utf8mb4"`)
- `collation` (e.g., `"utf8mb4_0900_ai_ci"`)
- `timezone` (e.g., `"UTC"`)
- `statement_cache_capacity` (e.g., `100`)
- `socket` (e.g., `"/path/to/socket"`)

### Start the connection using either:

#### Asynchronous Connection

```lua
conn:Start(function(err)
    if err then
        print("Connection failed: " .. err.message)
    else
        print("Connected successfully")
    end
end)
```

#### Synchronous Connection

```lua
conn:StartSync()
```

> **Note:** `StartSync` throws an error on failure, so consider using `pcall` to handle errors.

## Connection States

After starting a connection, you can check its state using the following methods:

- **Numeric State:** `conn:State()`
  Returns a number corresponding to one of the state constants defined in `goobie_mysql.STATES`.

- **State Name:** `conn:StateName()`
  Returns a string representation of the current state (e.g., `"CONNECTED"`).

Available states (available via `goobie_mysql.STATES`):

- `CONNECTED`
- `CONNECTING`
- `NOT_CONNECTED`
- `DISCONNECTED`

Additional helper methods:

- `conn:IsConnected()`
- `conn:IsConnecting()`
- `conn:IsNotConnected()`
- `conn:IsDisconnected()`

## Polling for Connection Events

For asynchronous operations that may not use callbacks directly, you can use the **`conn:Poll()`** method to process connection events.

## Disconnecting

When you need to close the connection, use one of the following methods:

#### Asynchronous Disconnection

```lua
conn:Disconnect(function(err)
    if err then
        print("Disconnection failed: " .. err.message)
    else
        print("Disconnected successfully")
    end
end)
```

#### Synchronous Disconnection

```lua
local err = conn:DisconnectSync()
if err then
    print("Disconnection failed: " .. err.message)
else
    print("Disconnected successfully")
end
```

## Error Handling

Most methods return an `Error` object on failure. This object contains:

- **`message`**: A description of the error.
- **`code`**: A numeric error code.
- **`sqlstate`** (optional): The SQL state string provided by MySQL.

Always check for errors after each operation to handle failures gracefully.

## Querying the Database

The module offers methods for running queries, with both asynchronous and synchronous versions.

### Run

For queries like INSERT, UPDATE, or DELETE that don’t return data.

#### Asynchronous

```lua
conn:Run("INSERT INTO mytable (column) VALUES (?)", {params = {"value"}}, function(err)
    if err then
        print("Query failed: " .. err.message)
    else
        print("Query executed successfully")
    end
end)
```

#### Synchronous

```lua
local err = conn:RunSync("INSERT INTO mytable (column) VALUES (?)", {params = {"value"}})
if err then
    print("Query failed: " .. err.message)
else
    print("Query executed successfully")
end
```

### Execute

Like `Run`, but returns `rows_affected` and `last_insert_id`.

#### Asynchronous

```lua
conn:Execute("INSERT INTO mytable (column) VALUES (?)", {params = {"value"}}, function(err, result)
    if err then
        print("Query failed: " .. err.message)
    else
        print("Rows affected: " .. result.rows_affected)
        print("Last insert ID: " .. result.last_insert_id)
    end
end)
```

#### Synchronous

```lua
local err, result = conn:ExecuteSync("INSERT INTO mytable (column) VALUES (?)", {params = {"value"}})
if err then
    print("Query failed: " .. err.message)
else
    print("Rows affected: " .. result.rows_affected)
    print("Last insert ID: " .. result.last_insert_id)
end
```

### Fetch

Retrieves multiple rows from a SELECT query.

#### Asynchronous

```lua
conn:Fetch("SELECT * FROM mytable WHERE column = ?", {params = {"value"}}, function(err, rows)
    if err then
        print("Query failed: " .. err.message)
    else
        for _, row in ipairs(rows) do
            print(row.column)
        end
    end
end)
```

#### Synchronous

```lua
local err, rows = conn:FetchSync("SELECT * FROM mytable WHERE column = ?", {params = {"value"}})
if err then
    print("Query failed: " .. err.message)
else
    for _, row in ipairs(rows) do
        print(row.column)
    end
end
```

### FetchOne

Retrieves a single row from a SELECT query.

#### Asynchronous

```lua
conn:FetchOne("SELECT * FROM mytable WHERE id = ?", {params = {1}}, function(err, row)
    if err then
        print("Query failed: " .. err.message)
    elseif row then
        print(row.column)
    else
        print("No row found")
    end
end)
```

#### Synchronous

```lua
local err, row = conn:FetchOneSync("SELECT * FROM mytable WHERE id = ?", {params = {1}})
if err then
    print("Query failed: " .. err.message)
elseif row then
    print(row.column)
else
    print("No row found")
end
```

### Query Parameters

To prevent SQL injection, always use the `params` field:

```lua
{params = {"value1", "value2"}}
```

For raw queries (such as multi-statement queries), set `raw = true`. Use this option with caution and avoid with untrusted input.

## Transactions

Group queries into a transaction that can be committed or rolled back.

### Asynchronous Transaction

```lua
conn:Begin(function(err, txn)
    if err then
        print("Failed to start transaction: " .. err.message)
        return
    end
    local err, rows = txn:Fetch("SELECT * FROM mytable")
    if err then
        print("Query failed: " .. err.message)
        txn:Rollback()
        return
    end
    local err = txn:Commit()
    if err then
        print("Commit failed: " .. err.message)
    end
end)
```

### Synchronous Transaction

```lua
conn:BeginSync(function(err, txn)
    if err then
        print("Failed to start transaction: " .. err.message)
        return
    end
    local err = txn:Run("INSERT INTO mytable (column) VALUES ('value')")
    if err then
        txn:Rollback()
        return
    end
    txn:Commit()
end)
```

Inside a transaction:

- All query methods (`Run`, `Execute`, `Fetch`, `FetchOne`) are synchronous and do not take a callback.
- Check transaction status with `txn:IsOpen()`.
- Finalize the transaction using `txn:Commit()` or `txn:Rollback()`.

## Ping

Test the connection with a ping (note: it’s not a reliable method to check if the connection is alive).

### Asynchronous

```lua
conn:Ping(function(err, latency)
    if err then
        print("Ping failed: " .. err.message)
    else
        print("Latency: " .. latency .. "ms")
    end
end)
```

### Synchronous

```lua
local err, latency = conn:PingSync()
if err then
    print("Ping failed: " .. err.message)
else
    print("Latency: " .. latency .. "ms")
end
```

## Examples

### Simple Query

```lua
local goobie_mysql = require("goobie_mysql")

local conn = goobie_mysql.NewConn({
    host = "localhost",
    username = "user",
    password = "pass",
    database = "mydb"
})

conn:StartSync()

local err, rows = conn:FetchSync("SELECT * FROM mytable")
if err then
    print("Query failed: " .. err.message)
else
    for _, row in ipairs(rows) do
        print(row.column)
    end
end
```

### Transaction Example

```lua
local goobie_mysql = require("goobie_mysql")

local conn = goobie_mysql.NewConn({
    host = "localhost",
    username = "user",
    password = "pass",
    database = "mydb"
})

conn:StartSync()

conn:BeginSync(function(err, txn)
    if err then
        print("Failed to start transaction: " .. err.message)
        return
    end
    local err = txn:Run("INSERT INTO mytable (column) VALUES ('value1')")
    if err then
        print("Insert failed: " .. err.message)
        txn:Rollback()
        return
    end
    local err = txn:Run("INSERT INTO mytable (column) VALUES ('value2')")
    if err then
        print("Insert failed: " .. err.message)
        txn:Rollback()
        return
    end
    local err = txn:Commit()
    if err then
        print("Commit failed: " .. err.message)
    end
end)
```
