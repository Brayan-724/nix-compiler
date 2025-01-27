use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;

use nix_macros::{builtin, gen_builtins};

use crate::value::{NixAttrSet, NixLambda, NixList};
use crate::{
    LazyNixValue, NixBacktrace, NixLabelKind, NixLabelMessage, NixLambdaParam, NixResult, NixValue,
    NixValueWrapped, NixVar, Scope,
};

use super::hash;

#[builtin]
pub fn abort(message: String) {
    panic!("Aborting: {message}")
}

#[builtin]
pub fn all(backtrace: &NixBacktrace, callback: NixLambda, list: NixList) {
    for item in list.0.iter() {
        let callback = callback.call(backtrace, item.clone())?;
        let callback = callback
            .resolve(backtrace)?
            .borrow()
            .as_bool()
            .ok_or_else(|| todo!("Error handling"))?;

        if !callback {
            return Ok(NixValue::Bool(false).wrap());
        }
    }

    Ok(NixValue::Bool(true).wrap())
}

#[builtin]
pub fn any(backtrace: &NixBacktrace, callback: NixLambda, list: NixList) {
    for item in list.0.iter() {
        let callback = callback.call(backtrace, item.clone())?;
        let callback = callback
            .resolve(backtrace)?
            .borrow()
            .as_bool()
            .ok_or_else(|| todo!("Error handling"))?;

        if callback {
            return Ok(NixValue::Bool(true).wrap());
        }
    }

    Ok(NixValue::Bool(false).wrap())
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
pub fn base_name_of(s: NixValueWrapped) {
    let s = s.borrow();

    let s = if let Some(s) = s.as_string() {
        if s.ends_with("/") {
            PathBuf::from(&s[..s.len() - 1])
        } else {
            PathBuf::from(s)
        }
    } else {
        let Some(s) = s.as_path() else {
            todo!("Error Handling: baseNameOf cannot convert into path");
        };

        s
    };

    let Some(s) = s.file_name() else {
        todo!("Error Handling: baseNameOf get file_name/baseNameOf");
    };
    let Some(s) = s.to_str() else {
        todo!("Error Handling: baseNameOf cannot get str from path");
    };

    Ok(NixValue::String(s.to_owned()).wrap())
}

#[builtin]
pub fn attr_values(set: NixValueWrapped) {
    let set = set.borrow();
    let Some(set) = set.as_attr_set() else {
        todo!("Error handling");
    };

    let values = set.values().cloned().collect::<Vec<NixVar>>();

    Ok(NixValue::List(NixList(Rc::new(values))).wrap())
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
pub fn concat_map(backtrace: &NixBacktrace, callback: NixLambda, list: NixList) {
    let mut out = vec![];

    for item in list.0.iter() {
        let item = callback.call(backtrace, item.clone())?.resolve(backtrace)?;

        let Some(item) = item.borrow().as_list() else {
            todo!("Error handling");
        };

        out.extend_from_slice(&item.0)
    }

    Ok(NixValue::List(NixList(Rc::new(out))).wrap())
}

#[builtin]
pub fn concat_string_sep(backtrace: &NixBacktrace, sep: String, list: NixList) {
    let list = list
        .0
        .iter()
        .map(|i| i.resolve(backtrace))
        .collect::<NixResult<Vec<_>>>()?
        .iter()
        .map(|i| {
            i.borrow()
                .cast_to_string()
                .ok_or_else(|| todo!("Error Handling"))
        })
        .collect::<NixResult<Vec<_>>>()?;
    Ok(NixValue::String(list.join(&sep)).wrap())
}

#[builtin]
pub fn dir_of(s: NixValueWrapped) {
    let s = s.borrow();
    let Some(s) = s.as_path() else {
        todo!("Error Handling: dirOf cannot convert into path");
    };
    let Some(s) = s.parent() else {
        todo!("Error Handling: dirOf get parent/dirname");
    };
    let Some(s) = s.to_str() else {
        todo!("Error Handling: dirOf cannot get str from path");
    };

    Ok(NixValue::String(s.to_owned()).wrap())
}

#[builtin]
pub fn elem(backtrace: &NixBacktrace, x: NixValueWrapped, xs: NixList) {
    for item in xs.0.iter() {
        let item = item.resolve(backtrace)?;

        if x.borrow().try_eq(&*item.borrow(), backtrace)? {
            return Ok(NixValue::Bool(true).wrap());
        }
    }

    Ok(NixValue::Bool(false).wrap())
}

#[builtin]
pub fn elemAt(backtrace: &NixBacktrace, xs: NixList, x: usize) {
    xs.0.get(x)
        .ok_or_else(|| todo!("Error handling: Out of bounds"))?
        .resolve(backtrace)
}

#[builtin]
pub fn filter(backtrace: &NixBacktrace, callback: NixLambda, list: NixList) {
    let mut out = Vec::with_capacity(list.0.len());

    for value in list.0.iter() {
        let item = callback
            .call(backtrace, value.clone())?
            .resolve(backtrace)?;

        let Some(item) = item.borrow().as_bool() else {
            todo!("Error handling");
        };

        if item {
            out.push(value.clone());
        }
    }

    Ok(NixValue::List(NixList(Rc::new(out))).wrap())
}

#[builtin]
pub fn function_args(callback: NixLambda) {
    match callback {
        NixLambda::Apply(_, NixLambdaParam::Pattern(param), _) => Ok(NixValue::AttrSet(
            NixAttrSet::from_iter(param.pat_entries().map(|p| {
                let name = p.ident().unwrap().ident_token().unwrap().to_string();
                let value = p.default().is_some();

                (name, NixValue::Bool(value).wrap_var())
            })),
        )
        .wrap()),
        _ => Ok(NixValue::AttrSet(NixAttrSet::new()).wrap()),
    }
}

#[builtin]
pub fn gen_list(backtrace: &NixBacktrace, callback: NixLambda, size: i64) {
    let out = (0..size)
        .map(|i| {
            LazyNixValue::new_callback_eval(
                backtrace,
                callback.clone(),
                NixValue::Int(i).wrap_var(),
            )
            .wrap_var()
        })
        .collect::<Vec<_>>();

    Ok(NixValue::List(NixList(Rc::new(out))).wrap())
}

fn hash_var(backtrace: &NixBacktrace, var: &NixVar, hasher: &mut impl Hasher) -> NixResult<u64> {
    match &*var.resolve(backtrace)?.borrow() {
        NixValue::AttrSet(_) => todo!("Error handling: Cannot hash AttrSet"),
        NixValue::Lambda(_) => todo!("Error handling: Cannot hash Lambda"),
        NixValue::Path(_) => todo!("Error handling: Cannot hash Path"),
        NixValue::String(_) => todo!("Error handling: Cannot hash String"),

        NixValue::Bool(e) => e.hash(hasher),
        NixValue::Float(e) => (*e as u64).hash(hasher),
        NixValue::Int(e) => e.hash(hasher),
        NixValue::List(e) => {
            e.0.iter()
                .map(|i| hash_var(backtrace, i, hasher).map(|_| {}))
                .collect::<NixResult<()>>()?
        }
        NixValue::Null => hasher.write(&[0]),
    };

    Ok(hasher.finish())
}

#[builtin]
pub fn generic_closure(backtrace: &NixBacktrace, argument: NixValueWrapped) {
    let argument = argument.borrow();
    let argument = argument
        .as_attr_set()
        .ok_or_else(|| todo!("Error handling"))?;

    let start_set = argument
        .get("startSet")
        .ok_or_else(|| todo!("Error handling: Getting startSet"))?
        .resolve(backtrace)?
        .borrow()
        .as_list()
        .ok_or_else(|| todo!("Error handling"))?
        .0;

    if start_set.is_empty() {
        return Ok(NixValue::List(NixList(start_set)).wrap());
    }

    let mut work_set = VecDeque::new();
    work_set.extend(start_set.iter().cloned());

    let op = argument
        .get("operator")
        .ok_or_else(|| todo!("Error handling: Getting startSet"))?
        .resolve(backtrace)?;
    let op = op.borrow();
    let op = op.as_lambda().ok_or_else(|| todo!("Error handling"))?;

    /* Construct the closure by applying the operator to elements of
    `workSet', adding the result to `workSet', continuing until
    no new elements are found. */
    let mut res = Vec::new();

    // `doneKeys' doesn't need to be a GC root, because its values are
    // reachable from res.

    let mut done_keys = HashSet::new();
    while let Some(item) = work_set.pop_front() {
        let e = item.resolve(backtrace)?;
        let e = e.borrow();
        let e = e.as_attr_set().ok_or_else(|| todo!("Error handling"))?;

        let key = e
            .get("key")
            .ok_or_else(|| todo!("Error handling: Getting key"))?;

        let mut hasher = std::hash::DefaultHasher::new();
        let key = hash_var(backtrace, key, &mut hasher)?;

        if !done_keys.insert(key) {
            continue;
        }

        res.push(item.clone());

        /* Call the `operator' function with `e' as argument. */
        let list = op
            .call(backtrace, item.clone())?
            .resolve(backtrace)?
            .borrow()
            .as_list()
            .ok_or_else(|| todo!("Error handling: Cast as list"))?;

        work_set.extend(list.0.iter().cloned());
    }

    Ok(NixValue::List(NixList(res.into())).wrap())
}

#[builtin()]
pub fn get_env(env: String) {
    let value = std::env::var(env).unwrap_or_default();

    Ok(NixValue::String(value).wrap())
}

fn intern_hash(ty: &str, bytes: &[u8]) -> String {
    let algorithm = match ty {
        "md5" => hash::Algorithm::MD5,
        "sha1" => hash::Algorithm::SHA1,
        "sha256" => hash::Algorithm::SHA256,
        "sha512" => hash::Algorithm::SHA512,
        _ => todo!("Error Handling: hashFile incompatible hash type"),
    };

    hash::hex_digest(algorithm, bytes)
}

#[builtin()]
pub fn hash_file(t: String, p: NixValueWrapped) {
    let Some(path) = p.borrow().as_path() else {
        todo!("Error Handling: hashFile cannot convert into path");
    };
    let Ok(content) = std::fs::read(path) else {
        todo!("Error Handling: hashFile cannot read file");
    };

    let value = intern_hash(&t, &content);
    Ok(NixValue::String(value).wrap())
}

#[builtin]
pub fn import(backtrace: &NixBacktrace, argument: NixValueWrapped) {
    let argument = argument.borrow();

    let path = match *argument {
        NixValue::AttrSet(ref set) => {
            let is_flake = if let Some(ty) = set.get("_type") {
                ty.resolve(backtrace)?
                    .borrow()
                    .cast_to_string()
                    .eq(&Some("flake".to_owned()))
            } else {
                false
            };

            if !is_flake {
                todo!("Cannot import attr set");
            }

            let out_path = set.get("outPath").expect("Flake should have outPath");
            let out_path = out_path.resolve(backtrace)?;
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
/// Log a variable and return it
pub fn inspect(backtrace: &NixBacktrace, recursive: bool, argument: NixVar) {
    let argument = argument.resolve_set(recursive, backtrace)?;
    println!("{argument:#?}");
    Ok(argument)
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
pub fn is_float(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_float()).wrap())
}

#[builtin]
pub fn is_int(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_int()).wrap())
}

#[builtin()]
pub fn is_list(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_list()).wrap())
}

#[builtin()]
pub fn is_null(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_null()).wrap())
}

#[builtin()]
pub fn is_path(argument: NixValueWrapped) {
    Ok(NixValue::Bool(argument.borrow().is_path()).wrap())
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
pub fn list_to_attrs(backtrace: &NixBacktrace, list: NixList) {
    let out = list
        .0
        .iter()
        .map(|item| {
            let (name, value) = {
                let item = item.resolve(backtrace)?;
                let item = item.borrow();

                let Some(set) = item.as_attr_set() else {
                    todo!("Error handling!");
                };

                (set.get("name").cloned(), set.get("value").cloned())
            };

            let Some(name) = name else {
                todo!("Error handling!");
            };

            let name = name.resolve(backtrace)?;

            let name = match &*name.borrow() {
                NixValue::String(ref s) => s.clone(),
                _ => todo!("Error handling!"),
            };

            let Some(value) = value else {
                todo!("Error handling!");
            };

            Ok((name, value))
        })
        .collect::<NixResult<NixAttrSet>>()?;

    Ok(NixValue::AttrSet(out).wrap())
}

#[builtin]
pub fn map(backtrace: &NixBacktrace, callback: NixLambda, list: NixList) {
    let mut out = Vec::with_capacity(list.0.len());

    for value in list.0.iter() {
        let value = callback.call(backtrace, value.clone())?;

        out.push(value);
    }

    Ok(NixValue::List(NixList(Rc::new(out))).wrap())
}

#[builtin]
pub fn map_attrs(backtrace: &NixBacktrace, callback: NixLambda, set: NixValueWrapped) {
    let set = set.borrow();
    let Some(set) = set.as_attr_set() else {
        todo!("Error handling");
    };

    let mut out = NixAttrSet::new();

    for (key, value) in set.iter() {
        let callback = callback
            .call(backtrace, NixValue::String(key.clone()).wrap_var())?
            .resolve(backtrace)?;
        let callback = callback.borrow();
        let Some(callback) = callback.as_lambda() else {
            todo!("Error handling")
        };

        let value = callback.call(backtrace, value.clone())?;

        out.insert(key.clone(), value);
    }

    Ok(NixValue::AttrSet(out).wrap())
}

#[builtin]
pub fn r#match(regex: String, content: String) {
    // TODO: Should do a regex caching, specially for loop optimisation
    let regex = regex::Regex::new(&regex).unwrap();

    Ok(regex
        .captures(content.as_str())
        .map(|c| {
            NixValue::List(NixList(Rc::new(
                c.iter()
                    .skip(1)
                    .map(|c| {
                        c.map(|c| c.as_str())
                            .map(String::from)
                            .map(NixValue::String)
                            .unwrap_or_default()
                            .wrap_var()
                    })
                    .collect::<Vec<_>>(),
            )))
        })
        .unwrap_or_default()
        .wrap())
}

#[builtin()]
pub fn path_exists(path: PathBuf) {
    let exists = path.try_exists().is_ok_and(|x| x);

    Ok(NixValue::Bool(exists).wrap())
}

#[builtin]
pub fn read_file(path: NixValueWrapped) {
    let path = path.borrow();
    let Some(path) = path.as_path() else {
        todo!("Error Handling");
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        todo!("Error Handling");
    };

    Ok(NixValue::String(content).wrap())
}

#[builtin]
pub fn read_file_type(path: NixValueWrapped) {
    let path = path.borrow();
    let Some(path) = path.as_path() else {
        todo!("Error Handling");
    };
    let Ok(metadata) = std::fs::metadata(path) else {
        todo!("Error Handling");
    };
    let res = if metadata.is_dir() {
        "directory"
    } else if metadata.is_symlink() {
        "symlink"
    } else if metadata.is_file() {
        "regular"
    } else {
        "unknown"
    };
    Ok(NixValue::String(res.to_owned()).wrap())
}

#[builtin]
pub fn replace_strings(
    backtrace: &NixBacktrace,
    from: NixList,
    to: NixList,
    s: String,
) -> Result<NixValueWrapped, NixError> {
    if from.0.len() != to.0.len() {
        todo!(
            "`from` and `to` arguments have different lengths: {} vs {}",
            from.0.len(),
            to.0.len()
        );
    }

    let mut from_vec = Vec::new();
    let mut to_cache = HashMap::new();

    for item in from.0.iter() {
        let resolved = item.resolve(backtrace)?;
        let Some(search) = resolved.borrow().cast_to_string() else {
            todo!("Expected string in `from`");
        };
        from_vec.push(search.clone());
    }

    let mut res = String::new();
    let s_chars: Vec<_> = s.chars().collect();
    let mut p = 0;

    while p <= s_chars.len() {
        let mut found = false;

        for (i, search) in from_vec.iter().enumerate() {
            if s_chars[p..].iter().collect::<String>().starts_with(search) {
                let replace = to.0.get(i).unwrap();
                let resolved_replace = replace.resolve(backtrace)?;
                let Some(replace_str) = resolved_replace.borrow().cast_to_string() else {
                    todo!("Expected string in `to`");
                };

                let cached_replace = to_cache.entry(i).or_insert_with(|| replace_str.clone());

                res.push_str(cached_replace);

                if search.is_empty() {
                    if p < s_chars.len() {
                        res.push(s_chars[p]);
                    }
                    p += 1;
                } else {
                    p += search.len();
                }
                found = true;
                break;
            }
        }

        if !found {
            if p < s_chars.len() {
                res.push(s_chars[p]);
            }
            p += 1;
        }
    }

    Ok(NixValue::String(res).wrap())
}

#[builtin()]
pub fn remove_attrs(backtrace: &NixBacktrace, attrset: NixValueWrapped, attrs: NixList) {
    if !attrset.borrow().is_attr_set() {
        todo!("Error handling")
    }

    let mut attrset = attrset.borrow().as_attr_set().unwrap().clone();

    let attrs = attrs
        .0
        .iter()
        .map(|attr| {
            attr.resolve(backtrace)
                .map(|attr| attr.borrow().cast_to_string().unwrap())
        })
        .collect::<Result<Vec<_>, _>>()?;

    for attr in attrs {
        attrset.remove(&attr);
    }

    Ok(NixValue::AttrSet(attrset).wrap())
}

#[builtin]
pub fn seq(_: NixValueWrapped, argument: NixValueWrapped) {
    Ok(argument)
}

#[builtin]
pub fn substring(start: usize, len: isize, s: String) {
    if len < 0 || start + len as usize > s.len() {
        Ok(NixValue::String(s[start..].to_owned()).wrap())
    } else if len == 0 || start > s.len() {
        Ok(NixValue::String(String::new()).wrap())
    } else {
        Ok(NixValue::String(s[start..start + len as usize].to_owned()).wrap())
    }
}

#[builtin]
pub fn split(regex: String, content: String) {
    // TODO: Should do a regex caching, specially for loop optimisation
    let regex = regex::Regex::new(&regex).unwrap();

    let mut out = vec![];

    let last_idx = regex.find_iter(&content).fold(0, |last_idx, matches| {
        out.push(NixValue::String(String::from(&content[last_idx..matches.start()])).wrap_var());

        out.push(
            NixValue::List(NixList(Rc::new(
                regex
                    .captures(matches.as_str())
                    .expect("Capture a string that already match")
                    .iter()
                    .skip(1)
                    .map(|c| {
                        c.map(|c| c.as_str())
                            .map(String::from)
                            .map(NixValue::String)
                            .unwrap_or_default()
                            .wrap_var()
                    })
                    .collect::<Vec<_>>(),
            )))
            .wrap_var(),
        );

        matches.end()
    });

    out.push(NixValue::String(String::from(&content[last_idx..])).wrap_var());

    Ok(NixValue::List(NixList(Rc::new(out))).wrap())
}

#[builtin]
pub fn string_length(argument: NixValueWrapped) {
    Ok(NixValue::Int(argument.borrow().cast_to_string().unwrap().len() as i64).wrap())
}

#[builtin()]
pub fn to_string(argument: String) {
    Ok(NixValue::String(argument).wrap())
}

#[builtin]
pub fn throw(backtrace: &NixBacktrace, message: String) {
    // TODO: in `nix-env -qa` and other commands that try
    // to evaluate a derivation that throws an error is
    // silently skipped (which is not the case for abort).

    let error = backtrace.to_error(
        NixLabelKind::Error,
        NixLabelMessage::Empty,
        format!("Throwing: {message}"),
    );

    print!("{error}");

    std::process::exit(1)
}

#[builtin]
pub fn trace(backtrace: &NixBacktrace, message: NixValueWrapped, argument: NixVar) {
    let message = message.borrow();

    if message.is_string() || message.is_path() {
        let message = message.cast_to_string().unwrap();
        println!("trace: {message}");
    } else {
        println!("trace: {message:?}");
    }

    argument.resolve(backtrace)
}

#[builtin()]
pub fn try_eval(backtrace: &NixBacktrace, argument: NixVar) {
    if let Err(_) = argument.resolve(backtrace) {
        let mut result = NixAttrSet::new();
        result.insert("success".to_string(), NixValue::Bool(false).wrap_var());
        // `value = false;` is unfortunate but removing it is a breaking change.
        // https://github.com/NixOS/nix/blob/master/src/libexpr/primops.cc#L942
        result.insert("value".to_string(), NixValue::Bool(false).wrap_var());

        return Ok(NixValue::AttrSet(result).wrap());
    };

    let mut result = NixAttrSet::new();
    result.insert("success".to_string(), NixValue::Bool(true).wrap_var());
    result.insert("value".to_string(), argument);

    return Ok(NixValue::AttrSet(result).wrap());
}

#[builtin]
pub fn type_of(argument: NixValueWrapped) {
    Ok(NixValue::String(argument.borrow().as_type().to_owned()).wrap())
}

// TODO: Add message to backtrace
#[builtin]
pub fn add_error_context(_: NixValueWrapped, argument: NixValueWrapped) {
    Ok(argument)
}

gen_builtins! {
    currentSystem = NixValue::String("x86_64-linux".to_owned());
    false = NixValue::Bool(false);
    nixVersion = NixValue::String("2.24.9".to_owned());
    null = NixValue::Null;
    true = NixValue::Bool(true);
}
