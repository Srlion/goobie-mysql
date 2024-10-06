use anyhow::{bail, Result};
use gmod::{lua::*, *};
use sqlx::mysql::MySqlConnectOptions;

#[derive(Debug, Clone)]
pub struct Options {
    pub inner: MySqlConnectOptions,
    pub on_connected: i32,
    pub on_error: i32,
    pub on_disconnected: i32,
}

impl Options {
    pub fn new() -> Self {
        Options {
            inner: MySqlConnectOptions::new(),
            on_connected: LUA_NOREF,
            on_error: LUA_NOREF,
            on_disconnected: LUA_NOREF,
        }
    }

    pub fn parse(&mut self, l: lua::State, parse_on_fns: bool) -> Result<()> {
        // if first argument is a string then it's a uri and increment arg_number as next argument has to be a table or nil
        let mut arg_n = 1;
        if l.is_string(arg_n) {
            self.parse_uri(l, arg_n)?;
            arg_n = 2;
        }

        if !l.is_none_or_nil(arg_n) {
            l.check_table(arg_n)?;
        } else {
            return Ok(());
        }

        // no uri provided, parse connect options
        if arg_n == 1 {
            self.parse_connect_options(l, arg_n)?;
        }

        if parse_on_fns {
            self.parse_on_fns(l, arg_n)?;
        }

        Ok(())
    }

    fn parse_uri(&mut self, l: lua::State, idx: i32) -> Result<()> {
        let uri = l.get_string_unchecked(idx);
        self.inner = uri.parse()?;
        Ok(())
    }

    fn parse_on_fns(&mut self, l: lua::State, arg_n: i32) -> Result<()> {
        if l.get_field_type_or_nil(arg_n, c"on_connected", LUA_TFUNCTION)? {
            self.on_connected = l.reference();
        }

        if l.get_field_type_or_nil(arg_n, c"on_error", LUA_TFUNCTION)? {
            self.on_error = l.reference();
        }

        if l.get_field_type_or_nil(arg_n, c"on_disconnected", LUA_TFUNCTION)? {
            self.on_disconnected = l.reference();
        }

        Ok(())
    }

    fn parse_connect_options(&mut self, l: lua::State, arg_n: i32) -> Result<()> {
        if l.get_field_type_or_nil(arg_n, c"uri", LUA_TSTRING)? {
            self.parse_uri(l, -1)?;
        } else {
            if l.get_field_type_or_nil(arg_n, c"host", LUA_TSTRING)?
                || l.get_field_type_or_nil(arg_n, c"hostname", LUA_TSTRING)?
            {
                let hot = &l.get_string_unchecked(-1); // 😲
                self.inner = self.inner.clone().host(hot);
                l.pop();
            }

            if l.get_field_type_or_nil(arg_n, c"port", LUA_TNUMBER)? {
                let port = l.to_number(-1) as u16;
                self.inner = self.inner.clone().port(port);
                l.pop();
            }

            if l.get_field_type_or_nil(arg_n, c"username", LUA_TSTRING)?
                || l.get_field_type_or_nil(arg_n, c"user", LUA_TSTRING)?
            {
                let user = &l.get_string_unchecked(-1);
                self.inner = self.inner.clone().username(user);
                l.pop();
            }

            if l.get_field_type_or_nil(arg_n, c"password", LUA_TSTRING)? {
                let pass = &l.get_string_unchecked(-1);
                self.inner = self.inner.clone().password(pass);
                l.pop();
            } else {
                bail!("Password is required!");
            }

            if l.get_field_type_or_nil(arg_n, c"database", LUA_TSTRING)?
                || l.get_field_type_or_nil(arg_n, c"db", LUA_TSTRING)?
            {
                let db = &l.get_string_unchecked(-1);
                self.inner = self.inner.clone().database(db);
                l.pop();
            } else {
                bail!("Database name is required!");
            }

            // self.uri = connect_options.build()?;
        }

        Ok(())
    }
}
