---@meta

---@class goobie_mysql
local goobie_mysql = {}

---@type string
goobie_mysql.VERSION = nil

---@type string
goobie_mysql.MAJOR_VERSION = nil

goobie_mysql.STATES = {
    ---@type number
    CONNECTED = nil,
    ---@type number
    CONNECTING = nil,
    ---@type number
    NOT_CONNECTED = nil,
    ---@type number
    DISCONNECTED = nil,
}

---@class Error
---@field message string
---@field code number
---@field sqlstate string?

---@class BaseConnOpts
---@field charset string?
---@field collation string?
---@field timezone string?
---@field statement_cache_capacity number?
---@field socket string?

---@class ConnOptsWithUri : BaseConnOpts
---@field uri string

---@class ConnOptsWithCredentials : BaseConnOpts
---@field host string
---@field username string
---@field password string
---@field database string

---@alias ConnOptions ConnOptsWithUri | ConnOptsWithCredentials

---@param opts ConnOptions
---@return Conn
function goobie_mysql.NewConn(opts) end

---@class Conn
local Conn = {}

---
--- Polls the connection for events.
---
--- Can be used to wait for a connection task to complete.
---
function Conn:Poll() end

---
--- Initiates a connection to the database asynchronously.
---
---@param callback fun(err: Error|nil)
function Conn:Start(callback) end

---
--- Initiates a connection to the database synchronously.
---
--- Throws an error if the connection fails.
---
function Conn:StartSync() end

---
--- Disconnects from the database asynchronously.
---
---@param callback fun(err: Error|nil)
function Conn:Disconnect(callback) end

---
--- Disconnects from the database synchronously.
---
--- Unlike Conn:StartSync, this function returns an error if the connection fails.
---
---@return Error|nil err
function Conn:DisconnectSync() end

---
--- Returns the current state of the connection.
---
---@return number
function Conn:State() end

--- Pings the database to check the connection status.
---
--- **Note:** It's generally not recommended to use this method to check if a connection is alive, as it may not be
--- reliable. For more information, refer to [this article](https://www.percona.com/blog/checking-for-a-live-database-connection-considered-harmful/).
---
--- It's used internally after each query fails, to confirm that the connection is dropped or not.
---
---@param callback fun(err: Error|nil, latency: number)
function Conn:Ping(callback) end

---@return Error err|nil
---@return number latency
function Conn:PingSync() end

---
--- Returns the current state of the connection as a string.
---
---@return string
function Conn:StateName() end

---@return boolean
function Conn:IsConnected() end

---@return boolean
function Conn:IsConnecting() end

---@return boolean
function Conn:IsDisconnected() end

---@return boolean
function Conn:IsNotConnected() end

-- General Query
---@alias QueryParam string | number | boolean
---@alias QueryParams QueryParam[]

---@class BaseQueryOpts
---@field params QueryParams?
---@field raw boolean? -- If true, the params will not be used and you can have multi statement queries.
--

-- Run
---@class RunQueryOpts : BaseQueryOpts
---@field callback fun(err: Error|nil)

---@param query string
---@param opts RunQueryOpts?
function Conn:Run(query, opts) end

---@param query string
---@param opts BaseQueryOpts?
---@return Error|nil err
function Conn:RunSync(query, opts) end
--

-- Execute
---@class ExecuteQueryResult
---@field rows_affected number
---@field last_insert_id number

---@class ExecuteQueryOpts : BaseQueryOpts
---@field callback fun(err: Error|nil, result: ExecuteQueryResult)

---@param query string
---@param opts ExecuteQueryOpts?
function Conn:Execute(query, opts) end

---@param query string
---@param opts BaseQueryOpts?
---@return Error|nil err
---@return ExecuteQueryResult result
function Conn:ExecuteSync(query, opts) end
--

--Fetch
---@class FetchQueryOpts : BaseQueryOpts
---@field callback fun(err: Error|nil, rows: table[])

---@param query string
---@param opts FetchQueryOpts?
function Conn:Fetch(query, opts) end

---@param query string
---@param opts BaseQueryOpts?
---@return Error|nil err
---@return table[] rows
function Conn:FetchSync(query, opts) end
--

--FetchOne
---@class FetchOneQueryOpts : BaseQueryOpts
---@field callback fun(err: Error|nil, row: table|nil)

---@param query string
---@param opts FetchOneQueryOpts?
function Conn:FetchOne(query, opts) end

---@param query string
---@param opts BaseQueryOpts?
---@return Error|nil err
---@return table|nil row
function Conn:FetchOneSync(query, opts) end
---

---@class Txn
local Txn = {}

--- Begins a transaction asynchronously.
---
--- Runs the callback inside a coroutine.
---
--- All queries inside will return values and take no callback.
---
---@param callback fun(err: Error|nil, txn: Txn)
function Conn:Begin(callback) end

--- Begins a transaction synchronously.
---
--- Runs the callback inside a coroutine.
---
--- All queries inside will return values and take no callback.
---
---@param callback fun(err: Error|nil, txn: Txn)
function Conn:BeginSync(callback) end

--- Returns whether the transaction is still open or not.
---@return boolean is_open
function Txn:IsOpen() end

--- Pings the database to check the connection status.
---
--- **Note:** It's generally not recommended to use this method to check if a connection is alive, as it may not be
--- reliable. For more information, refer to [this article](https://www.percona.com/blog/checking-for-a-live-database-connection-considered-harmful/).
---
---@return Error|nil err
---@return number latency
function Txn:Ping(callback) end

---@return Error|nil err
function Txn:Run(query, opts) end

---@return Error|nil err
---@return ExecuteQueryResult result
function Txn:Execute(query, opts) end

---@return Error|nil err
---@return table[] rows
function Txn:Fetch(query, opts) end

---@return Error|nil err
---@return table|nil row
function Txn:FetchOne(query, opts) end

---@return Error|nil err
function Txn:Commit() end

---@return Error|nil err
function Txn:Rollback() end
