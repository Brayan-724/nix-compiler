use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use nix_macros::{builtin, gen_builtins};
use rnix::ast;

use crate::value::{NixLambda, NixList};
use crate::{
    AsAttrSet, AsString, LazyNixValue, NixBacktrace, NixResult, NixValue, NixValueWrapped, NixVar,
    Scope,
};

#[builtin]
pub fn abort(message: String) {
    panic!("Aborting: {message}")
}

#[builtin]
pub fn attr_names(set: NixValueWrapped) {
    let set = set.borrow();
    let Some(set) = set.as_attr_set() else {
        todo!("Error handling");
    };

    let names = set
        .keys()
        .cloned()
        .map(NixValue::String)
        .map(NixValue::wrap_var)
        .collect::<Vec<NixVar>>();

    Ok(NixValue::List(NixList(Rc::new(names))).wrap())
}

#[builtin]
pub fn compare_versions(first_arg: String, second_arg: String) {
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

#[builtin]
pub fn concat_map(backtrace: Rc<NixBacktrace>, callback: NixLambda, list: NixList) {
    let mut out = vec![];

    for item in list.0.iter() {
        let item = callback.call(backtrace.clone(), item.clone())?;

        let Some(item) = item.borrow().as_list() else {
            todo!("Error handling");
        };

        out.extend_from_slice(&item.0)
    }

    Ok(NixValue::List(NixList(Rc::new(out))).wrap())
}

#[builtin]
pub fn gen_list(backtrace: Rc<NixBacktrace>, callback: NixLambda, size: i64) {
    let out = (0..size)
        .map(|i| {
            LazyNixValue::new_callback_eval(
                backtrace.clone(),
                callback.clone(),
                NixValue::Int(i).wrap_var(),
            )
            .wrap_var()
        })
        .collect::<Vec<_>>();

    Ok(NixValue::List(NixList(Rc::new(out))).wrap())
}

#[builtin()]
pub fn get_env(env: String) {
    let value = std::env::var(env).unwrap_or_default();

    Ok(NixValue::String(value).wrap())
}

#[builtin]
pub fn import(backtrace: Rc<NixBacktrace>, argument: NixValueWrapped) {
    let argument = argument.borrow();

    let path = match *argument {
        NixValue::AttrSet(ref set) => {
            let is_flake = if let Some(ty) = set.get("_type") {
                ty.resolve(backtrace.clone())?
                    .borrow()
                    .as_string()
                    .eq(&Some("flake".to_owned()))
            } else {
                false
            };

            if !is_flake {
                todo!("Cannot import attr set");
            }

            let out_path = set.get("outPath").expect("Flake should have outPath");
            let out_path = out_path.resolve(backtrace.clone())?;
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

    Scope::import_path(backtrace, path)
}

#[builtin]
pub fn is_attrs(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_attr_set()).wrap())
}

#[builtin]
pub fn is_bool(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().as_bool().is_some()).wrap())
}

#[builtin]
pub fn is_function(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_function()).wrap())
}

#[builtin]
pub fn is_int(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_int()).wrap())
}

#[builtin()]
pub fn is_list(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().as_list().is_some()).wrap())
}

#[builtin()]
pub fn is_null(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_null()).wrap())
}

#[builtin]
pub fn is_string(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_string()).wrap())
}

#[builtin()]
pub fn length(list: NixList) {
    Ok(NixValue::Int(list.0.len() as i64).wrap())
}

#[builtin]
pub fn list_to_attrs(backtrace: Rc<NixBacktrace>, list: NixList) {
    let out = list
        .0
        .iter()
        .map(|item| {
            let (name, value) = {
                let item = item.resolve(backtrace.clone())?;
                let item = item.borrow();

                let Some(set) = item.as_attr_set() else {
                    todo!("Error handling!");
                };

                (set.get("name").cloned(), set.get("value").cloned())
            };

            let Some(name) = name else {
                todo!("Error handling!");
            };

            let name = name.resolve(backtrace.clone())?;

            let name = match &*name.borrow() {
                NixValue::String(ref s) => s.clone(),
                _ => todo!("Error handling!"),
            };

            let Some(value) = value else {
                todo!("Error handling!");
            };

            Ok((name, value))
        })
        .collect::<NixResult<HashMap<String, NixVar>>>()?;

    Ok(NixValue::AttrSet(out).wrap())
}

#[builtin()]
pub fn path_exists(path: PathBuf) {
    let exists = path.try_exists().is_ok_and(|x| x);

    Ok(NixValue::Bool(exists).wrap())
}

#[builtin()]
pub fn remove_attrs(backtrace: Rc<NixBacktrace>, attrset: NixValueWrapped, attrs: NixValueWrapped) {
    if !attrset.borrow().is_attr_set() {
        todo!("Error handling")
    }

    let mut attrset = attrset.borrow().as_attr_set().unwrap().clone();

    let attrs = attrs.borrow();
    let Some(attrs) = attrs.as_list() else {
        todo!("Error handling")
    };

    let attrs = attrs
        .0
        .iter()
        .map(|attr| {
            attr.resolve(backtrace.clone())
                .map(|attr| attr.borrow().as_string().unwrap())
        })
        .collect::<Result<Vec<_>, _>>()?;

    for attr in attrs {
        attrset.remove(&attr);
    }

    Ok(NixValue::AttrSet(attrset).wrap())
}

#[builtin()]
pub fn to_string(argument: String) {
    Ok(NixValue::String(argument).wrap())
}

#[builtin()]
pub fn try_eval(backtrace: Rc<NixBacktrace>, argument: (Rc<Scope>, ast::Expr)) {
    let (scope, node) = argument;

    let Ok(argument) = scope.visit_expr(backtrace, node) else {
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

gen_builtins! {
    false = NixValue::Bool(false);
    nixVersion = NixValue::String("2.24.9".to_owned());
    null = NixValue::Null;
    true = NixValue::Bool(true);
}
