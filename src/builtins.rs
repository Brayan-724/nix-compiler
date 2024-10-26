use core::fmt;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;

use rnix::ast;

use crate::flake::resolve_flake;
use crate::{
    AsAttrSet, AsString, FileScope, LazyNixValue, NixResult, NixValue, NixValueWrapped, Scope,
};

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
    insert!(removeAttrs = NixValue::Builtin(NixValueBuiltin::RemoveAttrs(None)));
    insert!(toString = NixValue::Builtin(NixValueBuiltin::ToString));
    insert!(tryEval = NixValue::Builtin(NixValueBuiltin::TryEval));

    NixValue::AttrSet(builtins)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NixValueBuiltin {
    Abort,
    CompareVersions(Option<String>),
    GetEnv,
    Import,
    PathExists,
    RemoveAttrs(Option<(Rc<Scope>, ast::Expr)>),
    ToString,
    TryEval,
}

impl fmt::Display for NixValueBuiltin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NixValueBuiltin::Abort => "<abort>",
            NixValueBuiltin::CompareVersions(..) => "<compareVersions>",
            NixValueBuiltin::GetEnv => "<getEnv>",
            NixValueBuiltin::Import => "<import>",
            NixValueBuiltin::PathExists => "<pathExists>",
            NixValueBuiltin::RemoveAttrs(..) => "<removeAttrs>",
            NixValueBuiltin::ToString => "<toString>",
            NixValueBuiltin::TryEval => "<tryEval>",
        })
    }
}

impl NixValueBuiltin {
    pub fn run(&self, scope: Rc<Scope>, argument: ast::Expr) -> NixResult {
        use NixValueBuiltin::*;

        match self {
            Abort => abort(scope.visit_expr(argument)?),
            CompareVersions(first_arg) => {
                compare_versions(scope.visit_expr(argument)?, first_arg.clone())
            }
            GetEnv => get_env(scope.visit_expr(argument)?),
            Import => import(scope.visit_expr(argument)?),
            PathExists => path_exists(scope.visit_expr(argument)?),
            RemoveAttrs(Some((scope, expr))) => {
                let first_arg = scope.visit_expr(expr.clone())?;
                remove_attrs(scope.visit_expr(argument)?, first_arg)
            }
            RemoveAttrs(None) => {
                Ok(NixValue::Builtin(NixValueBuiltin::RemoveAttrs(Some((scope, argument)))).wrap())
            }
            ToString => to_string(scope.visit_expr(argument)?),
            TryEval => try_eval(scope, argument),
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

pub fn remove_attrs(argument: NixValueWrapped, first_arg: NixValueWrapped) -> NixResult {
    if !first_arg.borrow().is_attr_set() {
        todo!("Error handling")
    }

    let mut first_arg = first_arg.borrow().as_attr_set().unwrap().clone();

    let attrs = argument.borrow();
    let Some(attrs) = attrs.as_list() else {
        todo!("Error handling")
    };

    let attrs = attrs
        .into_iter()
        .map(|attr| {
            attr.resolve()
                .map(|attr| attr.borrow().as_string().unwrap())
        })
        .collect::<Result<Vec<_>, _>>()?;

    for attr in attrs {
        first_arg.remove(&attr);
    }

    Ok(NixValue::AttrSet(first_arg).wrap())
}

pub fn to_string(argument: NixValueWrapped) -> NixResult {
    let argument = argument.borrow();

    let Some(message) = argument.as_string() else {
        todo!("Error handling: {argument:#?}")
    };

    Ok(NixValue::String(message).wrap())
}

pub fn try_eval(scope: Rc<Scope>, node: ast::Expr) -> NixResult {
    let Ok(argument) = scope.visit_expr(node) else {
        let mut result = HashMap::new();
        result.insert("success".to_string(), NixValue::Bool(false).wrap_var());
        return Ok(NixValue::AttrSet(result).wrap());
    };

    let mut result = HashMap::new();
    result.insert("success".to_string(), NixValue::Bool(true).wrap_var());
    result.insert(
        "value".to_string(),
        LazyNixValue::Concrete(argument).wrap_var(),
    );

    return Ok(NixValue::AttrSet(result).wrap());
}
