local debug = debug
local coroutine = coroutine

local tostring = tostring
local istable = istable
local isstring = isstring
local setmetatable = setmetatable

local MAJOR_VERSION = "MAJOR_VERSION_PLACEHOLDER"

local goobie_mysql = _G["goobie_mysql_" .. MAJOR_VERSION]

local GOOBIE_MYSQL_PREFIX = "(Goobie MySQL v" .. goobie_mysql.VERSION .. ") "
local CONN_META = goobie_mysql.CONN_META

function goobie_mysql.Error(msg, ...)
    return ErrorNoHalt(string.format(msg, ...), "\n")
end

local STATES = goobie_mysql.STATES
local STATES_NAMES = {}; do
    for k, v in pairs(STATES) do
        STATES_NAMES[v] = k
    end
end

local ERROR_META = {
    __tostring = function(s)
        return string.format("(%s) %s", s.code or "?", s.message or "unknown error")
    end
}
goobie_mysql.ERROR_META = ERROR_META

local function new_error(msg)
    return setmetatable({
        message = msg,
    }, ERROR_META)
end

local RealNewConn = goobie_mysql.NewConn
function goobie_mysql.NewConn(opts)
    if not istable(opts) then
        return error("opts must be a table")
    end
    local conn = RealNewConn(opts)
    conn.locked = false
    conn.queue = {}
    return conn
end

-- **Connection State Methods**
function CONN_META:IsConnected() return self:State() == STATES.CONNECTED end
function CONN_META:IsConnecting() return self:State() == STATES.CONNECTING end
function CONN_META:IsDisconnected() return self:State() == STATES.DISCONNECTED end
function CONN_META:IsNotConnected() return self:State() == STATES.NOT_CONNECTED end
function CONN_META:StateName() return STATES_NAMES[self:State()] end

function CONN_META:Error(msg, ...)
    return goobie_mysql.Error(GOOBIE_MYSQL_PREFIX .. "|" .. self:Host() .. "| " .. msg, ...)
end

local QUERIES = {
    Run = CONN_META.Run,
    Execute = CONN_META.Execute,
    FetchOne = CONN_META.FetchOne,
    Fetch = CONN_META.Fetch,
}

local function check_query(query, opts)
    if not isstring(query) then
        return error("query must be a string", 2)
    end
    if opts then
        if not istable(opts) then
            return error("opts must be a table", 2)
        end
    else
        opts = {}
    end
    return opts
end

local function ConnQueueTask(self, func, p1, p2, p3)
    if self.locked then
        if self.txn and self.txn.open and coroutine.running() == self.txn.co then
            return error("you can't run queries on a `connection` inside an open transaction's coroutine")
        end

        local queue = self.queue
        queue[#queue + 1] = {func, p1, p2, p3}
    else
        func(self, p1, p2, p3)
    end
end

local function ConnProcessQueue(self)
    if self.locked then return end -- we can't process if connection is locked

    local queue = self.queue
    local queue_len = #queue
    if queue_len == 0 then return end

    self.queue = {} -- make sure to clear the queue to avoid conflicts

    for i = 1, queue_len do
        local task = queue[i]
        -- we call QueueTask again because a task can be a Transaction Begin
        local func = task[1]
        ConnQueueTask(self, func, task[2], task[3], task[4])
    end
end

local function ConnSyncOP(self, op)
    local done, err, res
    op(function(e, r)
        done = true
        err, res = e, r
    end)
    while not done do
        self:Poll()
    end
    return err, res
end

local function create_query_method(query_type)
    local query_func = QUERIES[query_type]

    CONN_META[query_type] = function(self, query, opts)
        opts = check_query(query, opts)
        return ConnQueueTask(self, query_func, query, opts)
    end

    CONN_META[query_type .. "Sync"] = function(self, query, opts)
        opts = check_query(query, opts)
        return ConnSyncOP(self, function(cb)
            opts.callback = cb
            ConnQueueTask(self, query_func, query, opts)
        end)
    end
end

create_query_method("Run")
create_query_method("Execute")
create_query_method("FetchOne")
create_query_method("Fetch")

-- Pings don't need to be queued, so we can just call the method directly
function CONN_META:PingSync()
    local done, err, res
    self:Ping(function(e, r)
        done = true
        err, res = e, r
    end)
    while not done do
        self:Poll()
    end
    return err, res
end

function CONN_META:StartSync()
    local err = ConnSyncOP(self, function(cb)
        self:Start(cb)
    end)
    if err then
        return error(tostring(err))
    end
end


function CONN_META:DisconnectSync()
    local err = ConnSyncOP(self, function(cb)
        self:Disconnect(cb)
    end)
    return err
end

--------------------------------------------------------------------------------

local TxnResume, TxnQuery, TxnFinalize

local TRANSACTION_META = {}
TRANSACTION_META.__index = TRANSACTION_META

local function NewTransaction(conn, co, traceback)
    return setmetatable({
        conn = conn,
        co = co,
        traceback = traceback,
        open = true,
    }, TRANSACTION_META)
end

function TxnResume(self, ...)
    local co = self.co
    local err

    local co_status = coroutine.status(co)
    if co_status == "dead" then
        if self.open then
            err = "transaction was left open!" .. self.traceback
        end
    else
        local success, co_err = coroutine.resume(co, ...)
        if success then
            if coroutine.status(co) == "dead" and self.open then
                err = "transaction was left open!" .. self.traceback
            end
        else
            err = co_err .. "\n" .. debug.traceback(co)
        end
    end

    if err then
        ErrorNoHalt(err, "\n")
        TxnFinalize(self, "rollback", true)
    end
end

function TxnQuery(self, query_type, query, opts)
    opts = check_query(query, opts)

    if not self.open then
        return error("transaction is closed")
    end

    local conn = self.conn
    -- we need to set locked to false to make sure queries are not queued
    -- it's not an issue if it errors or not because TxnResume will handle it anyway

    opts.callback = function(err, res)
        TxnResume(self, err, res)
    end

    conn.locked = false
    conn[query_type](conn, query, opts)
    conn.locked = true

    return coroutine.yield()
end

function TxnFinalize(self, action, failed)
    if not self.open then
        return
    end

    local conn = self.conn
    conn.locked = false

    local err
    -- if the connection dropped/lost/reconnected, we don't want to send a query
    -- because we are not in a transaction anymore
    if conn:IsConnected() and self.conn_id == conn:ID() then
        if failed then
            conn:Run("ROLLBACK;") -- we don't care about the result
        else
            if action == "commit" then
                err = TxnQuery(self, "Run", "COMMIT;")
            elseif action == "rollback" then
                err = TxnQuery(self, "Run", "ROLLBACK;")
            end
        end

        conn.locked = false -- TxnQuery will set it back to true
        conn:Run("SET autocommit = 1;") -- we don't care about the result
    end

    self.open = false
    conn.txn = nil
    conn.locked = false
    ConnProcessQueue(conn)

    return err
end

function TRANSACTION_META:IsOpen()
    return self.open
end

function TRANSACTION_META:Ping()
    if not self.open then
        return error("transaction is closed")
    end
    return self.conn:Ping(function(err, latency)
        TxnResume(self, err, latency)
    end)
end

function TRANSACTION_META:Run(query, opts)
    return TxnQuery(self, "Run", query, opts)
end

function TRANSACTION_META:Execute(query, opts)
    return TxnQuery(self, "Execute", query, opts)
end

function TRANSACTION_META:FetchOne(query, opts)
    return TxnQuery(self, "FetchOne", query, opts)
end

function TRANSACTION_META:Fetch(query, opts)
    return TxnQuery(self, "Fetch", query, opts)
end

function TRANSACTION_META:Commit()
    return TxnFinalize(self, "commit")
end

function TRANSACTION_META:Rollback()
    return TxnFinalize(self, "rollback")
end

local function TxnBegin(self, callback, sync)
    if not isfunction(callback) then
        return error("callback must be a function")
    end

    local traceback = debug.traceback("", 2)

    local is_locked = false
    self:Run("SET autocommit = 0;", {
        callback = function(err)
            is_locked = true

            if not self:IsConnected() then
                ProtectedCall(callback, new_error("connection is not open"))
                return
            end

            self.locked = true

            local co = coroutine.create(callback)
            local txn = NewTransaction(self, co, traceback)
            txn.conn_id = self:ID()
            self.txn = txn
            if err then
                txn.open = false
                TxnResume(txn, err)
            else
                TxnResume(txn, nil, txn)
            end

            -- this is a nice way to make it easier to use sync transactions
            if sync then
                while txn.open do
                    self:Poll()
                end
            end
        end,
    })

    if sync then
        while not is_locked do
            self:Poll()
        end
    end
end

function CONN_META:Begin(callback)
    return TxnBegin(self, callback, false)
end

function CONN_META:BeginSync(callback)
    return TxnBegin(self, callback, true)
end
