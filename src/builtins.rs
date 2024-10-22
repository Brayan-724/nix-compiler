use std::ops::Deref;

use crate::scope::FileScope;
use crate::value::{NixValue, NixVar};

pub fn import(argument: NixVar) -> NixVar {
    let argument = argument.resolve();
    let argument = argument.borrow();

    let NixValue::Path(path) = argument.deref() else {
        todo!("Error handling");
    };

    FileScope::from_path(path).evaluate().unwrap()
}
