use core::fmt;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ops::Deref;
use std::path::Path;

use crate::flake::resolve_flake;
use crate::{AsString, FileScope, NixResult, NixValue, NixValueWrapped};

pub fn get_builtins() -> NixValue {
    let mut builtins = HashMap::new();

    macro_rules! insert {
        ($key:ident = $value:expr) => {
            builtins.insert(stringify!($key).to_owned(), $value.wrap_var())
        };
    }

    insert!(abort = NixValue::Builtin(NixValueBuiltin::Abort));
    insert!(compareVersions = NixValue::Builtin(NixValueBuiltin::CompareVersions(None)));
    insert!(getEnv = NixValue::Builtin(NixValueBuiltin::GetEnv));
    insert!(import = NixValue::Builtin(NixValueBuiltin::Import));
    insert!(nixVersion = NixValue::String(String::from("2.24.9")));
    insert!(pathExists = NixValue::Builtin(NixValueBuiltin::PathExists));
    insert!(toString = NixValue::Builtin(NixValueBuiltin::ToString));

    NixValue::AttrSet(builtins)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NixValueBuiltin {
    Abort,
    CompareVersions(Option<String>),
    GetEnv,
    Import,
    PathExists,
    ToString,
}

impl fmt::Display for NixValueBuiltin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NixValueBuiltin::Abort => "<abort>",
            NixValueBuiltin::CompareVersions(_) => "<compare_versions>",
            NixValueBuiltin::GetEnv => "<get_env>",
            NixValueBuiltin::Import => "<import>",
            NixValueBuiltin::PathExists => "<path_exists>",
            NixValueBuiltin::ToString => "<to_string>",
        })
    }
}

impl NixValueBuiltin {
    pub fn run(&self, argument: NixValueWrapped) -> NixResult {
        use NixValueBuiltin::*;

        match self {
            Abort => abort(argument),
            CompareVersions(first_arg) => compare_versions(argument, first_arg.clone()),
            GetEnv => get_env(argument),
            Import => import(argument),
            PathExists => path_exists(argument),
            ToString => to_string(argument),
        }
    }
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
            std::cmp::Ordering::Greater => return Ok(NixValue::Int(1).wrap()),
        }
    }

    Ok(NixValue::Int(0).wrap())
}

pub fn get_env(argument: NixValueWrapped) -> NixResult {
    let Some(env) = argument.borrow().as_string() else {
        todo!("Error handling")
    };

    let value = std::env::var(env).unwrap_or_default();

    Ok(NixValue::String(value).wrap())
}

pub fn import(argument: NixValueWrapped) -> NixResult {
    let argument = argument.borrow();

    let path = match argument.deref() {
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
        NixValue::Path(path) => path.clone(),
        NixValue::String(path) => path.into(),
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

pub fn path_exists(argument: NixValueWrapped) -> NixResult {
    let argument = argument.borrow();

    let Some(path) = argument.as_path() else {
        todo!("Error handling");
    };

    let exists = path.try_exists().is_ok_and(|x| x);

    Ok(NixValue::Bool(exists).wrap())
}

pub fn to_string(argument: NixValueWrapped) -> NixResult {
    let argument = argument.borrow();

    let Some(message) = argument.as_string() else {
        todo!("Error handling: {argument:#?}")
    };

    Ok(NixValue::String(message).wrap())
}
