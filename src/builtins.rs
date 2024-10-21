use std::ops::Deref;

use crate::scope::FileScope;
use crate::value::{NixValue, NixValueWrapped};

pub fn import(argument: NixValueWrapped) -> NixValueWrapped {
    let argument = argument.borrow();

    let NixValue::Path(path) = argument.deref() else {
        todo!("Error handling");
    };

    FileScope::from_path(path).evaluate().unwrap()
}
