use gmod::lua;

use crate::GLOBAL_TABLE_NAME_C;

#[derive(PartialEq)]
#[atomic_enum::atomic_enum]
pub enum State {
    Connected,
    Connecting,
    NotConnected,
    Disconnected,
    Error,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            State::Connected => write!(f, "Connected"),
            State::Connecting => write!(f, "Connecting"),
            State::NotConnected => write!(f, "Not Connected"),
            State::Disconnected => write!(f, "Disconnected"),
            State::Error => write!(f, "Error"),
        }
    }
}

pub fn setup(l: lua::State) {
    l.get_global(GLOBAL_TABLE_NAME_C);
    {
        l.new_table();
        {
            l.push_number(State::Connected as i32);
            l.set_field(-2, c"CONNECTED");

            l.push_number(State::Connecting as i32);
            l.set_field(-2, c"CONNECTING");

            l.push_number(State::NotConnected as i32);
            l.set_field(-2, c"NOT_CONNECTED");

            l.push_number(State::Disconnected as i32);
            l.set_field(-2, c"DISCONNECTED");

            l.push_number(State::Error as i32);
            l.set_field(-2, c"ERROR");
        }
        l.set_field(-2, c"STATES");
    }
    l.pop();
}
