use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use nix_macros::{builtin, gen_builtins};
use rnix::ast;

use crate::{AsAttrSet, AsString, LazyNixValue, NixValue, NixValueWrapped, Scope};

#[builtin]
pub fn abort(message: String) -> ! {
    panic!("Aborting: {message}")
}

#[builtin]
pub fn compare_versions(first_arg: String, second_arg: String) -> NixResult {
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

#[builtin()]
pub fn get_env(env: String) -> NixResult {
    let value = std::env::var(env).unwrap_or_default();

    Ok(NixValue::String(value).wrap())
}

#[builtin]
pub fn import(argument: NixValueWrapped) {
    let argument = argument.borrow();

    let path = match *argument {
        NixValue::AttrSet(ref set) => {
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

            let NixValue::Path(ref path) = *out_path else {
                todo!("Error handling");
            };

            path.join("default.nix")
        }
        NixValue::Path(ref path) => path.clone(),
        NixValue::String(ref path) => path.into(),
        _ => todo!("Error handling"),
    };

    Scope::import_path(path)
}

#[builtin()]
pub fn is_list(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().as_list().is_some()).wrap())
}

#[builtin()]
pub fn path_exists(path: PathBuf) -> NixResult {
    let exists = path.try_exists().is_ok_and(|x| x);

    Ok(NixValue::Bool(exists).wrap())
}

#[builtin()]
pub fn remove_attrs(attrset: NixValueWrapped, attrs: NixValueWrapped) -> NixResult {
    if !attrset.borrow().is_attr_set() {
        todo!("Error handling")
    }

    let mut attrset = attrset.borrow().as_attr_set().unwrap().clone();

    let attrs = attrs.borrow();
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
        attrset.remove(&attr);
    }

    Ok(NixValue::AttrSet(attrset).wrap())
}

#[builtin()]
pub fn to_string(argument: String) -> NixResult {
    Ok(NixValue::String(argument).wrap())
}

#[builtin()]
pub fn try_eval(argument: (Rc<Scope>, ast::Expr)) -> NixResult {
    let (scope, node) = argument;

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

gen_builtins!{}
