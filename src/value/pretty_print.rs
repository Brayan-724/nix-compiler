use std::fmt::{self, Write};

use super::{NixAttrSet, NixLambda, NixValue};

impl fmt::Debug for NixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NixValue::AttrSet(NixAttrSet::Dynamic(set)) => {
                let mut map = f.debug_map();

                for (key, value) in set.iter() {
                    map.entry(key, value);
                }

                map.finish()
            }
            NixValue::AttrSet(NixAttrSet::Derivation {
                selected_output,
                derivation,
            }) => {
                f.write_str("<derivation ")?;
                f.write_str(
                    &derivation
                        .as_ref()
                        .path(&selected_output)
                        .expect("`selected_output` is part of its outputs"),
                )?;
                f.write_char('>')
            }
            NixValue::Bool(true) => f.write_str("true"),
            NixValue::Bool(false) => f.write_str("false"),
            NixValue::Float(val) => f.write_str(&val.to_string()),
            NixValue::Int(val) => f.write_str(&val.to_string()),
            NixValue::Lambda(NixLambda::Apply(..)) => f.write_str("<lamda>"),
            NixValue::Lambda(NixLambda::Builtin(builtin)) => fmt::Debug::fmt(builtin, f),
            NixValue::List(list) => {
                let mut debug_list = f.debug_list();

                for item in &*list.0 {
                    debug_list.entry(item);
                }

                debug_list.finish()
            }
            NixValue::Null => f.write_str("null"),
            NixValue::Path(path) => fmt::Debug::fmt(path, f),
            NixValue::String(s) => {
                f.write_char('"')?;
                f.write_str(s)?;
                f.write_char('"')
            }
        }
    }
}

impl fmt::Display for NixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NixValue::AttrSet(NixAttrSet::Dynamic(set)) => {
                let width = f.width().unwrap_or_default();
                let outside_pad = " ".repeat(width);

                let width = width + 2;
                let pad = " ".repeat(width);

                f.write_char('{')?;

                if f.alternate() {
                    f.write_char('\n')?;
                }

                for (key, value) in set.iter() {
                    let value = value.as_concrete().unwrap_or_else(|| {
                        eprintln!("Can't display something unresolved, run `.resolve_set()` before display it");
                        std::process::exit(1)
                    });

                    let value = &*value.borrow();

                    if f.alternate() {
                        f.write_str(&pad)?;
                    } else {
                        f.write_char(' ')?;
                    }

                    f.write_str(key)?;
                    f.write_str(" = ")?;

                    if f.alternate() {
                        f.write_fmt(format_args!("{value:#width$}"))?;
                    } else {
                        fmt::Display::fmt(value, f)?;
                    }

                    f.write_char(';')?;

                    if f.alternate() {
                        f.write_char('\n')?;
                    }
                }

                if f.alternate() {
                    f.write_str(&outside_pad)?;
                } else {
                    f.write_char(' ')?;
                }

                f.write_char('}')
            }
            NixValue::AttrSet(NixAttrSet::Derivation {
                selected_output,
                derivation,
            }) => {
                f.write_str("<derivation ")?;
                f.write_str(
                    &derivation
                        .as_ref()
                        .path(&selected_output)
                        .expect("`selected_output` is part of its outputs"),
                )?;
                f.write_char('>')
            }
            NixValue::Bool(true) => f.write_str("true"),
            NixValue::Bool(false) => f.write_str("false"),
            NixValue::Float(val) => f.write_str(&val.to_string()),
            NixValue::Int(val) => f.write_str(&val.to_string()),
            NixValue::Lambda(NixLambda::Apply(..)) => f.write_str("<lamda>"),
            NixValue::Lambda(NixLambda::Builtin(builtin)) => fmt::Display::fmt(builtin, f),
            NixValue::List(list) => {
                let width = f.width().unwrap_or_default();
                let outside_pad = " ".repeat(width);

                let width = width + 2;
                let pad = " ".repeat(width);

                f.write_char('[')?;

                if f.alternate() {
                    f.write_char('\n')?;
                }

                for value in &*list.0 {
                    let value = value.as_concrete().unwrap_or_else(|| {
                        eprintln!("Can't display something unresolved, run `.resolve_set()` before display it");
                        std::process::exit(1)
                    });
                    let value = &*value.borrow();

                    if f.alternate() {
                        f.write_str(&pad)?;
                    } else {
                        f.write_char(' ')?;
                    }

                    if f.alternate() {
                        f.write_fmt(format_args!("{value:#width$}"))?;
                    } else {
                        fmt::Display::fmt(value, f)?;
                    }

                    if f.alternate() {
                        f.write_char('\n')?;
                    }
                }

                if f.alternate() {
                    f.write_str(&outside_pad)?;
                } else {
                    f.write_char(' ')?;
                }

                f.write_char(']')
            }
            NixValue::Null => f.write_str("null"),
            NixValue::Path(path) => f.write_fmt(format_args!("{}", path.display())),
            NixValue::String(s) => {
                f.write_char('"')?;
                f.write_str(s)?;
                f.write_char('"')
            }
        }
    }
}
