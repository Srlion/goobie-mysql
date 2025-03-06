use anyhow::{bail, Result};
use gmod::{lua::*, *};
use sqlx::mysql::MySqlConnectOptions;

#[derive(Debug, Clone)]
pub struct Options {
    pub inner: MySqlConnectOptions,
}

impl Options {
    pub fn new() -> Self {
        Options {
            inner: MySqlConnectOptions::new(),
        }
    }

    pub fn parse(&mut self, l: lua::State) -> Result<()> {
        // if first argument is a string then it's a uri and increment arg_number as next argument has to be a table or nil
        l.check_table(1)?;

        self.parse_uri_options(l, 1)?;
        // self.parse_on_fns(l, 1)?;
        self.parse_connect_options(l, 1)?;

        Ok(())
    }

    fn parse_uri(&mut self, l: lua::State, idx: i32) -> Result<()> {
        let uri = l.get_string_unchecked(idx);
        self.inner = uri.parse()?;
        Ok(())
    }

    //     fn parse_on_fns(&mut self, l: lua::State, arg_n: i32) -> Result<()> {
    //         // if l.get_field_type_or_nil(arg_n, c"on_error", LUA_TFUNCTION)? {
    //         //     self.on_error = l.reference();
    //         // }
    //
    //         Ok(())
    //     }

    fn parse_uri_options(&mut self, l: lua::State, arg_n: i32) -> Result<()> {
        if l.get_field_type_or_nil(arg_n, c"uri", LUA_TSTRING)? {
            self.parse_uri(l, -1)?;
        } else {
            if l.get_field_type_or_nil(arg_n, c"host", LUA_TSTRING)?
                || l.get_field_type_or_nil(arg_n, c"hostname", LUA_TSTRING)?
            {
                let hot = l.get_string_unchecked(-1); // 😲
                self.inner = self.inner.clone().host(&hot);
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
                let user = l.get_string_unchecked(-1);
                self.inner = self.inner.clone().username(&user);
                l.pop();
            }

            if l.get_field_type_or_nil(arg_n, c"password", LUA_TSTRING)? {
                let pass = l.get_string_unchecked(-1);
                self.inner = self.inner.clone().password(&pass);
                l.pop();
            } else {
                bail!("Password is required!");
            }

            if l.get_field_type_or_nil(arg_n, c"database", LUA_TSTRING)?
                || l.get_field_type_or_nil(arg_n, c"db", LUA_TSTRING)?
            {
                let db = l.get_string_unchecked(-1);
                self.inner = self.inner.clone().database(db.as_ref());
                l.pop();
            } else {
                bail!("Database name is required!");
            }

            // self.uri = connect_options.build()?;
        }

        Ok(())
    }

    fn parse_connect_options(&mut self, l: lua::State, arg_n: i32) -> Result<()> {
        if l.get_field_type_or_nil(arg_n, c"charset", LUA_TSTRING)? {
            let charset = l.get_string_unchecked(-1);
            self.inner = self.inner.clone().charset(&charset);
            l.pop();
        }

        if l.get_field_type_or_nil(arg_n, c"collation", LUA_TSTRING)? {
            let collation = l.get_string_unchecked(-1);
            self.inner = self.inner.clone().collation(&collation);
            l.pop();
        }

        if l.get_field_type_or_nil(arg_n, c"timezone", LUA_TSTRING)? {
            let timezone = l.get_string_unchecked(-1);
            self.inner = self.inner.clone().timezone(timezone);
            l.pop();
        }

        if l.get_field_type_or_nil(arg_n, c"statement_cache_capacity", LUA_TNUMBER)? {
            let capacity = l.to_number(-1) as usize;
            self.inner = self
                .inner
                .clone()
                .statement_cache_capacity(capacity);
            l.pop();
        }

        if l.get_field_type_or_nil(arg_n, c"socket", LUA_TSTRING)? {
            let socket = l.get_string_unchecked(-1);
            self.inner = self.inner.clone().socket(socket);
            l.pop();
        }

        Ok(())
    }
}
