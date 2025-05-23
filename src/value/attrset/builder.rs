use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use rnix::ast;

use crate::value::NixAttrSet;
use crate::{
    LazyNixValue, NixAttrSetDynamic, NixBacktrace, NixError, NixLabel, NixLabelKind,
    NixLabelMessage, NixResult, NixSpan, NixValue, NixVar, Scope,
};

pub enum AttrsetBuilderValue {
    Var(NixVar),
    Set(AttrsetBuilder),
}

pub struct AttrsetBuilder {
    building: HashMap<String, AttrsetBuilderValue>,
    cached: Option<Rc<NixAttrSetDynamic>>,
}

impl fmt::Debug for AttrsetBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let alternate = f.alternate();
        let expanded = f.sign_plus();

        if let Some(cached) = &self.cached {
            fmt::Debug::fmt(cached, f)
        } else if expanded {
            let mut s = f.debug_map();

            for (key, entry) in &self.building {
                match entry {
                    AttrsetBuilderValue::Var(v) => {
                        if alternate {
                            s.entry(key, &format_args!("{v:#?}"));
                        } else {
                            s.entry(key, &format_args!("{v:?}"));
                        }
                    }
                    AttrsetBuilderValue::Set(v) => {
                        if alternate {
                            s.entry(key, &format_args!("{v:+#?}"));
                        } else {
                            s.entry(key, &format_args!("{v:+?}"));
                        }
                    }
                }
            }

            s.finish()
        } else {
            let mut s = f.debug_struct("AttrsetBuilder");

            for (key, entry) in &self.building {
                match entry {
                    AttrsetBuilderValue::Var(v) => {
                        if alternate {
                            s.field(key, &format_args!("{v:#?}"));
                        } else {
                            s.field(key, &format_args!("{v:?}"));
                        }
                    }
                    AttrsetBuilderValue::Set(v) => {
                        if alternate {
                            s.field(key, &format_args!("{v:+#?}"));
                        } else {
                            s.field(key, &format_args!("{v:+?}"));
                        }
                    }
                }
            }

            s.finish()
        }
    }
}

impl From<NixAttrSetDynamic> for AttrsetBuilder {
    fn from(value: NixAttrSetDynamic) -> Self {
        let mut out = AttrsetBuilder::new();

        for (k, v) in value {
            out.insert_var(k, v);
        }

        out
    }
}

impl From<&NixAttrSet> for AttrsetBuilder {
    fn from(value: &NixAttrSet) -> Self {
        let mut out = AttrsetBuilder::new();

        for (k, v) in value {
            out.insert_var(k.clone(), v);
        }

        out
    }
}

impl AttrsetBuilder {
    pub fn new() -> Self {
        Self {
            building: HashMap::new(),
            cached: None,
        }
    }

    pub fn wrap_mut(self) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(self))
    }

    pub fn cached(&mut self) -> Rc<NixAttrSetDynamic> {
        self.cached
            .get_or_insert_with(|| {
                let mut out = NixAttrSetDynamic::new();

                for (key, entry) in &mut self.building {
                    let v = match entry {
                        AttrsetBuilderValue::Var(v) => v.clone(),
                        AttrsetBuilderValue::Set(s) => {
                            NixValue::AttrSet(NixAttrSet::Dynamic(s.cached())).wrap_var()
                        }
                    };

                    out.insert(key.clone(), v);
                }

                out.into()
            })
            .clone()
    }

    pub fn finish(self) -> NixAttrSetDynamic {
        let mut out = NixAttrSetDynamic::new();

        for (key, entry) in self.building {
            let v = match entry {
                AttrsetBuilderValue::Var(v) => v,
                AttrsetBuilderValue::Set(mut s) => {
                    NixValue::AttrSet(NixAttrSet::Dynamic(s.cached())).wrap_var()
                }
            };

            out.insert(key, v);
        }

        out
    }

    pub fn insert_var(&mut self, key: String, value: NixVar) {
        self.cached = None;
        self.building.insert(key, AttrsetBuilderValue::Var(value));
    }

    #[nix_macros::profile]
    fn insert_path(
        &mut self,
        scope: &Rc<Scope>,
        backtrace: &NixBacktrace,
        attr: ast::Attr,
        mut attr_path: impl Iterator<Item = ast::Attr>,
        value: NixVar,
    ) -> NixResult<()> {
        let attr = scope.resolve_attr(backtrace, &attr)?;

        // non-storeable attributes are ignored as empty strings
        if attr.is_empty() {
            return Ok(());
        }

        let val = self.building.entry(attr);

        if let Some(next_attr) = attr_path.next() {
            let val = val.or_insert_with(|| AttrsetBuilderValue::Set(AttrsetBuilder::new()));

            let AttrsetBuilderValue::Set(ref mut val) = val else {
                todo!("Error handling: key already setted")
            };

            val.insert_path(scope, backtrace, next_attr, attr_path, value)
        } else {
            match val {
                Entry::Occupied(_) => Err(NixError::todo(
                    backtrace.0.clone(),
                    "OwO",
                    backtrace.1.clone(),
                )),
                Entry::Vacant(v) => {
                    v.insert(AttrsetBuilderValue::Var(value));
                    Ok(())
                }
            }
        }
    }

    pub fn insert(
        &mut self,
        scope: &Rc<Scope>,
        backtrace: &NixBacktrace,
        attr_path: ast::Attrpath,
        attr_value: ast::Expr,
    ) -> NixResult<()> {
        let value = LazyNixValue::Pending(
            scope.new_backtrace(backtrace, &attr_value),
            scope.clone().new_child(),
            attr_value,
        )
        .wrap_var();

        let mut attr_path = attr_path.attrs();

        // Take first attribute or stop here
        let Some(attr) = attr_path.next() else {
            return Ok(());
        };

        self.cached = None;

        self.insert_path(scope, backtrace, attr, attr_path, value)
    }

    pub fn insert_entry(
        &mut self,
        scope: &Rc<Scope>,
        backtrace: &NixBacktrace,
        entry: ast::Entry,
    ) -> NixResult<()> {
        match entry {
            ast::Entry::Inherit(entry) => {
                self.cached = None;

                let from = entry.from().map(|from| {
                    (
                        LazyNixValue::Pending(
                            scope.new_backtrace(backtrace, &from),
                            scope.clone(),
                            from.expr().unwrap(),
                        )
                        .wrap_var(),
                        from,
                    )
                });

                for attr_node in entry.attrs() {
                    let attr = scope.resolve_attr(backtrace, &attr_node)?;

                    let attr_node = attr_node.clone();
                    let file = scope.file.clone();

                    let value = if let Some((from, from_expr)) = &from {
                        let attr = attr.clone();
                        let from = from.clone();
                        let from_expr = from_expr.clone();

                        LazyNixValue::new_eval(
                            scope.new_backtrace(backtrace, &from_expr),
                            Box::new(move |backtrace| {
                                from.resolve(&backtrace)?
                                    .borrow()
                                    .as_attr_set()
                                    .unwrap()
                                    .get(&attr)
                                    .ok_or_else(|| {
                                        backtrace.to_labeled_error(
                                            vec![
                                                NixLabel::new(
                                                    NixSpan::from_ast_node(&file, &attr_node)
                                                        .into(),
                                                    NixLabelMessage::AttributeMissing,
                                                    NixLabelKind::Error,
                                                ),
                                                NixLabel::new(
                                                    NixSpan::from_ast_node(&file, &from_expr)
                                                        .into(),
                                                    NixLabelMessage::Custom(
                                                        "Parent attrset".to_owned(),
                                                    ),
                                                    NixLabelKind::Help,
                                                ),
                                            ],
                                            format!("Attribute '\x1b[1;95m{attr}\x1b[0m' missing"),
                                        )
                                    })?
                                    .resolve(&backtrace)
                            }),
                        )
                    } else {
                        let scope = scope.clone();
                        let attr = attr.clone();

                        LazyNixValue::new_eval(
                            scope.new_backtrace(backtrace, &attr_node),
                            Box::new(move |backtrace| {
                                let Some(value) = scope.get_variable(attr.clone()) else {
                                    return Err(backtrace.to_labeled_error(
                                        vec![NixLabel::new(
                                            NixSpan::from_ast_node(&file, &attr_node).into(),
                                            NixLabelMessage::VariableNotFound,
                                            NixLabelKind::Error,
                                        )],
                                        format!("Variable '{attr} not found"),
                                    ));
                                };

                                value.resolve(&backtrace)
                            }),
                        )
                    };

                    match self.building.entry(attr) {
                        Entry::Occupied(_) => todo!("Error handling: key already setted"),
                        Entry::Vacant(v) => {
                            v.insert(AttrsetBuilderValue::Var(value.wrap_var()));
                        }
                    }
                }

                Ok(())
            }
            ast::Entry::AttrpathValue(entry) => self.insert(
                scope,
                backtrace,
                entry.attrpath().unwrap(),
                entry.value().unwrap(),
            ),
        }
    }
}
