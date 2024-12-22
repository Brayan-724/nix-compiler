use std::cell::RefCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use rnix::ast;

use crate::{
    NixBacktrace, NixError, NixLabel, NixLabelKind, NixLabelMessage, NixResult, NixSpan,
    NixValueWrapped, NixVar, Scope,
};

use super::{NixAttrSet, NixLambda, NixValue};

#[derive(Clone)]
pub enum LazyNixValue {
    Concrete(NixValueWrapped),
    Pending(NixBacktrace, Rc<Scope>, ast::Expr),
    Eval(
        NixBacktrace,
        Rc<RefCell<Option<Box<dyn FnOnce(&NixBacktrace) -> NixResult>>>>,
    ),
    /// Partial resolve for update operator (`<expr> // <expr>`)
    UpdateResolve {
        lhs: NixValueWrapped,
        rhs: ast::Expr,

        backtrace: NixBacktrace,
        scope: Rc<Scope>,
    },
    Resolving(NixBacktrace),
}

impl fmt::Debug for LazyNixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyNixValue::Concrete(value) => fmt::Debug::fmt(value.borrow().deref(), f),
            LazyNixValue::Pending(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Eval(..) => f.write_str("<not-resolved>"),
            LazyNixValue::UpdateResolve { lhs, .. } => fmt::Debug::fmt(lhs.borrow().deref(), f),
            LazyNixValue::Resolving(..) => f.write_str("<resolving>"),
        }
    }
}

impl fmt::Display for LazyNixValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyNixValue::Concrete(value) => fmt::Display::fmt(value.borrow().deref(), f),
            LazyNixValue::Pending(..) => f.write_str("<not-resolved>"),
            LazyNixValue::Eval(..) => f.write_str("<not-resolved>"),
            LazyNixValue::UpdateResolve { lhs, .. } => fmt::Display::fmt(lhs.borrow().deref(), f),
            LazyNixValue::Resolving(..) => f.write_str("<resolving>"),
        }
    }
}

impl LazyNixValue {
    pub fn try_eq(
        lhs: &Rc<RefCell<Self>>,
        rhs: &Rc<RefCell<Self>>,
        backtrace: &NixBacktrace,
    ) -> NixResult<bool> {
        let lhs = LazyNixValue::resolve(lhs, &backtrace)?;
        let rhs = LazyNixValue::resolve(rhs, &backtrace)?;

        let lhs = lhs.borrow();
        let rhs = rhs.borrow();

        Ok(lhs.try_eq(&*rhs, backtrace)?)
    }
}

impl LazyNixValue {
    pub fn new_eval(
        backtrace: NixBacktrace,
        fun: Box<dyn FnOnce(&NixBacktrace) -> NixResult>,
    ) -> Self {
        LazyNixValue::Eval(backtrace, Rc::new(RefCell::new(Option::Some(fun))))
    }

    pub fn new_callback_eval(backtrace: &NixBacktrace, callback: NixLambda, value: NixVar) -> Self {
        match callback {
            NixLambda::Apply(scope, param, expr) => {
                let span = Rc::new(NixSpan::from_ast_node(&scope.file, &expr));

                LazyNixValue::new_eval(
                    NixBacktrace::new_none(span.clone(), Some(backtrace.clone())),
                    Box::new(move |backtrace| {
                        let scope = scope.new_child();

                        match param {
                            crate::NixLambdaParam::Ident(ident) => {
                                scope.set_variable(ident, value);
                            }
                            crate::NixLambdaParam::Pattern(_) => {
                                return Err(crate::NixError::todo(
                                    span,
                                    "Pattern lambda param",
                                    Some((&*backtrace).clone()),
                                ))
                            }
                        };

                        scope.visit_expr(backtrace, expr)?.resolve(backtrace)
                    }),
                )
            }
            NixLambda::Builtin(builtin) => LazyNixValue::new_eval(
                backtrace.clone(),
                Box::new(move |backtrace| builtin.run(backtrace, value)),
            ),
        }
    }

    pub fn wrap_var(self) -> NixVar {
        NixVar(Rc::new(RefCell::new(self)))
    }

    pub fn as_concrete(&self) -> Option<NixValueWrapped> {
        if let LazyNixValue::Concrete(value) = self {
            Some(value.clone())
        } else if let LazyNixValue::UpdateResolve { lhs, .. } = self {
            Some(lhs.clone())
        } else {
            None
        }
    }

    pub fn resolve(this: &Rc<RefCell<Self>>, backtrace: &NixBacktrace) -> NixResult {
        if let LazyNixValue::Concrete(value) = &*this.borrow() {
            return Ok(value.clone());
        }

        let backtrace = &match *this.borrow() {
            LazyNixValue::Concrete(_) => unreachable!(),
            LazyNixValue::Pending(ref backtrace, ..) => backtrace.clone(),
            LazyNixValue::Eval(ref backtrace, ..) => backtrace.clone(),
            LazyNixValue::UpdateResolve { ref backtrace, .. } => backtrace.clone(),
            LazyNixValue::Resolving(ref def_backtrace) => {
                let label = NixLabelMessage::Empty;
                let kind = NixLabelKind::Error;

                let NixBacktrace(span, def_backtrace, ..) = def_backtrace;

                let label = NixLabel::new(span.clone(), label, kind);
                let called_label = NixLabel::new(
                    backtrace.0.clone(),
                    NixLabelMessage::Custom("Called from here".to_string()),
                    NixLabelKind::Help,
                );

                return Err(NixError {
                    message: "Infinite recursion detected. Tried to get a value that is resolving"
                        .to_owned(),
                    labels: vec![label, called_label],
                    backtrace: def_backtrace.clone(),
                });
            }
        };

        let old = this.replace(LazyNixValue::Resolving(backtrace.clone()));

        match old {
            LazyNixValue::Concrete(..) | LazyNixValue::Resolving(..) => unreachable!(),
            LazyNixValue::UpdateResolve {
                lhs,
                rhs,
                backtrace,
                scope,
            } => {
                this.replace(LazyNixValue::Concrete(lhs.clone()));

                scope.visit_expr(&backtrace, rhs).and_then(|rhs| {
                    if matches!(&*rhs.0.borrow(), LazyNixValue::UpdateResolve { .. }) {
                        let LazyNixValue::UpdateResolve {
                            lhs: resolved_rhs,
                            rhs,
                            backtrace,
                            scope,
                        } = &&*rhs.0.borrow()
                        else {
                            unreachable!()
                        };

                        let resolved_lhs = resolved_rhs
                            .borrow()
                            .as_attr_set()
                            .ok_or_else(|| todo!("Error handling"))
                            .map(|rhs| {
                                let lhs_set = lhs.borrow().as_attr_set().cloned().unwrap();
                                let mut lhs = NixAttrSet::new();

                                lhs.extend(lhs_set);
                                lhs.extend(rhs.clone());

                                NixValue::AttrSet(lhs).wrap()
                            })?;

                        *this.borrow_mut().deref_mut() = LazyNixValue::UpdateResolve {
                            lhs: resolved_lhs.clone(),
                            rhs: rhs.clone(),
                            backtrace: backtrace.clone(),
                            scope: scope.clone(),
                        };

                        Ok(resolved_lhs)
                    } else {
                        rhs.resolve(&backtrace).and_then(|rhs| {
                            rhs.borrow()
                                .as_attr_set()
                                .ok_or_else(|| todo!("Error handling"))
                                .map(|rhs| {
                                    let lhs_set = lhs.borrow().as_attr_set().cloned().unwrap();
                                    let mut lhs = NixAttrSet::new();

                                    lhs.extend(lhs_set);
                                    lhs.extend(rhs.clone());

                                    let value = NixValue::AttrSet(lhs).wrap();

                                    *this.borrow_mut().deref_mut() =
                                        LazyNixValue::Concrete(value.clone());

                                    value
                                })
                        })
                    }
                })
            }
            LazyNixValue::Pending(_, scope, expr) => {
                let value = scope.visit_expr(backtrace, expr)?;

                let value = if matches!(&*value.0.borrow(), LazyNixValue::UpdateResolve { .. }) {
                    this.replace(value.0.borrow().clone());

                    LazyNixValue::resolve(this, backtrace)?
                } else {
                    let value = value.resolve(backtrace)?;
                    this.replace(LazyNixValue::Concrete(value.clone()));

                    value
                };

                Ok(value)
            }
            LazyNixValue::Eval(_, eval) => {
                let value = eval
                    .borrow_mut()
                    .take()
                    .expect("Eval cannot be called twice")(backtrace)?;

                *this.borrow_mut().deref_mut() = LazyNixValue::Concrete(value.clone());

                Ok(value)
            }
        }
    }

    pub fn resolve_set(
        this: &Rc<RefCell<Self>>,
        recursive: bool,
        backtrace: &NixBacktrace,
    ) -> NixResult {
        let value = Self::resolve(this, backtrace)?;

        if value.borrow().is_attr_set() {
            let values = if let Some(set) = value.borrow().as_attr_set() {
                set.values().cloned().collect::<Vec<_>>()
            } else {
                unreachable!()
            };

            for var in values {
                if recursive {
                    var.resolve_set(true, backtrace)?;
                } else {
                    var.resolve(backtrace)?;
                }
            }
        } else if let Some(list) = value.borrow().as_list() {
            list.0
                .iter()
                .map(|var| {
                    if recursive {
                        var.resolve_set(true, backtrace)?;
                    } else {
                        var.resolve(backtrace)?;
                    }

                    Ok(())
                })
                .collect::<Result<(), _>>()?;
        }

        Ok(value)
    }
}
