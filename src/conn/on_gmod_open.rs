use gmod::*;

use crate::GLOBAL_TABLE_NAME_C;

const CONNECT_METHODS: &[LuaReg] = lua_regs![
    "NewConn" => super::new,
];

pub fn init(l: lua::State) {
    l.register(GLOBAL_TABLE_NAME_C.as_ptr(), CONNECT_METHODS.as_ptr());
    l.pop();

    l.new_metatable(super::META_NAME);
    {
        l.register(std::ptr::null(), super::METHODS.as_ptr());

        l.push_value(-1); // Pushes the metatable to the top of the stack
        l.set_field(-2, c"__index");
    }
    l.pop();

    super::state::setup(l);
    super::transaction::setup(l);
}
