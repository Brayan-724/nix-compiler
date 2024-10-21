use crate::value::{NixValue, NixValueWrapped};

pub fn import(argument: NixValueWrapped) -> NixValueWrapped {
    NixValue::Null.wrap()
}
