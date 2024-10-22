use std::ffi::OsStr;
use std::ops::Deref;
use std::path::Path;

use crate::flake::resolve_flake;
use crate::scope::FileScope;
use crate::value::{AsString, NixValue, NixVar};

#[derive(Clone)]
pub enum NixValueBuiltin {
    Abort,
    Import,
}

pub fn abort(argument: NixVar) -> ! {
    let argument = argument.resolve();
    let argument = argument.borrow();

    let Some(message) = argument.as_string() else {
        todo!("Error handling: {argument:#?}")
    };

    panic!("Aborting: {message}")
}

pub fn import(argument: NixVar) -> NixVar {
    let argument = argument.resolve();
    let argument = argument.borrow();

    let path = match argument.deref() {
        NixValue::Path(path) => path.clone(),
        NixValue::AttrSet(set) => {
            let is_flake = set
                .get("_type")
                .is_some_and(|ty| ty.resolve_map(|val| val.as_string() == Some("flake".to_owned())));

            if !is_flake {
                todo!("Cannot import attr set");
            }

            let out_path = set.get("outPath").expect("Flake should have outPath");
            let out_path = out_path.resolve();
            let out_path = out_path.borrow();

            let NixValue::Path(path) = out_path.deref() else {
                todo!("Error handling");
            };

            path.join("default.nix")
        }
        _ => todo!("Error handling"),
    };

    import_path(path)
}

pub fn import_path(path: impl AsRef<Path>) -> NixVar {
    let path = path.as_ref();

    println!("Importing {path:#?}");

    let result = FileScope::from_path(path).evaluate().unwrap();

    if path.file_name() == Some(OsStr::new("flake.nix")) {
        resolve_flake(result)
    } else {
        result
    }
}
