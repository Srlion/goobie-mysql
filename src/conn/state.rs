use gmod::lua;

use crate::GLOBAL_TABLE_NAME_C;

#[derive(PartialEq)]
#[atomic_enum::atomic_enum]
pub enum State {
    Connected,
    Connecting,
    NotConnected,
    Disconnected,
}

impl State {
    pub const fn to_usize(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            State::Connected => write!(f, "Connected"),
            State::Connecting => write!(f, "Connecting"),
            State::NotConnected => write!(f, "Not Connected"),
            State::Disconnected => write!(f, "Disconnected"),
        }
    }
}

pub fn setup(l: lua::State) {
    l.get_global(GLOBAL_TABLE_NAME_C);
    {
        l.new_table();
        {
            l.push_number(AtomicState::to_usize(State::Connected));
            l.set_field(-2, c"CONNECTED");

            l.push_number(AtomicState::to_usize(State::Connecting));
            l.set_field(-2, c"CONNECTING");

            l.push_number(AtomicState::to_usize(State::NotConnected));
            l.set_field(-2, c"NOT_CONNECTED");

            l.push_number(AtomicState::to_usize(State::Disconnected));
            l.set_field(-2, c"DISCONNECTED");
        }
        l.set_field(-2, c"STATES");
    }
    l.pop();
}
