use std::ffi::OsStr;
use std::ops::Deref;
use std::path::Path;

use crate::flake::resolve_flake;
use crate::{AsString, FileScope, NixResult, NixValue, NixValueWrapped};

#[derive(Clone, PartialEq, Eq)]
pub enum NixValueBuiltin {
    Abort,
    CompareVersions(Option<String>),
    Import,
    ToString,
}

pub fn abort(argument: NixValueWrapped) -> ! {
    let argument = argument.borrow();

    let Some(message) = argument.as_string() else {
        todo!("Error handling: {argument:#?}")
    };

    panic!("Aborting: {message}")
}

pub fn compare_versions(argument: NixValueWrapped, first_arg: Option<String>) -> NixResult {
    let Some(first_arg) = first_arg else {
        let argument = argument.borrow();

        let Some(first_arg) = argument.as_string() else {
            todo!("Error handling: {argument:#?}")
        };

        return Ok(NixValue::Builtin(NixValueBuiltin::CompareVersions(Some(first_arg))).wrap());
    };

    let argument = argument.borrow();

    let Some(second_arg) = argument.as_string() else {
        todo!("Error handling: {argument:#?}")
    };

    let first_arg = first_arg.split(".");
    let second_arg = second_arg.split(".");

    for (first, second) in first_arg.zip(second_arg) {
        let first = first.parse::<u8>().unwrap();
        let second = second.parse::<u8>().unwrap();

        match first.cmp(&second) {
            std::cmp::Ordering::Less => return Ok(NixValue::Int(-1).wrap()),
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => return Ok(NixValue::Int(-1).wrap()),
        }
    }

    Ok(NixValue::Int(0).wrap())
}

pub fn import(argument: NixValueWrapped) -> NixResult {
    let argument = argument.borrow();

    let path = match argument.deref() {
        NixValue::Path(path) => path.clone(),
        NixValue::AttrSet(set) => {
            let is_flake = if let Some(ty) = set.get("_type") {
                ty.resolve_map(|val| val.as_string() == Some("flake".to_owned()))?
            } else {
                false
            };

            if !is_flake {
                todo!("Cannot import attr set");
            }

            let out_path = set.get("outPath").expect("Flake should have outPath");
            let out_path = out_path.resolve()?;
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

pub fn import_path(path: impl AsRef<Path>) -> NixResult {
    let path = path.as_ref();

    println!("Importing {path:#?}");

    let result = FileScope::from_path(path).evaluate()?;

    if path.file_name() == Some(OsStr::new("flake.nix")) {
        resolve_flake(result)
    } else {
        Ok(result)
    }
}

pub fn to_string(argument: NixValueWrapped) -> NixResult {
    let argument = argument.borrow();

    let Some(message) = argument.as_string() else {
        todo!("Error handling: {argument:#?}")
    };

    Ok(NixValue::String(message).wrap())
}
