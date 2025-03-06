use std::ops::Deref;
use std::rc::Rc;

use rnix::ast::{self, AstToken, HasEntry};
use rowan::ast::AstNode;

use crate::result::{NixBacktrace, NixSpan};
use crate::value::{NixLambda, NixList};
use crate::{
    LazyNixValue, NixAttrSet, NixBacktraceKind, NixError, NixLabel, NixLabelKind, NixLabelMessage,
    NixLambdaParam, NixResult, NixValue, NixValueWrapped, NixVar, Scope,
};

impl Scope {
    fn insert_to_attrset(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        out: NixValueWrapped,
        attrpath: ast::Attrpath,
        attr_value: ast::Expr,
    ) -> NixResult {
        let mut attr_path: Vec<ast::Attr> = attrpath.attrs().collect();
        let last_attr_path = attr_path
            .pop()
            .expect("Attrpath requires at least one attribute");

        let target =
            self.resolve_attr_set_path(backtrace, out.clone(), attr_path.into_iter())??;

        if !target.borrow().is_attr_set() {
            todo!("Error handling")
        };

        let attr = self.resolve_attr(backtrace, &last_attr_path)?;

        // non-storeable attributes are ignored as empty strings
        if !attr.is_empty() {
            let child = LazyNixValue::Pending(
                self.new_backtrace(backtrace, &attr_value),
                self.clone().new_child(),
                attr_value,
            )
            .wrap_var();

            let mut target = target.borrow_mut();
            let set = target.as_attr_set_mut().unwrap();

            set.insert(attr, child);
        }

        Ok(out)
    }

    fn insert_entry_to_attrset(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        out: NixValueWrapped,
        entry: ast::Entry,
    ) -> NixResult {
        match entry {
            ast::Entry::Inherit(entry) => {
                let from = entry.from().map(|from| {
                    (
                        LazyNixValue::Pending(
                            self.new_backtrace(backtrace, &from),
                            self.clone(),
                            from.expr().unwrap(),
                        )
                        .wrap_var(),
                        from,
                    )
                });

                for attr_node in entry.attrs() {
                    let attr = self.resolve_attr(backtrace, &attr_node)?;

                    let attr_node = attr_node.clone();
                    let file = self.file.clone();

                    let value = if let Some((from, from_expr)) = &from {
                        let attr = attr.clone();
                        let from = from.clone();
                        let from_expr = from_expr.clone();

                        LazyNixValue::new_eval(
                            self.new_backtrace(backtrace, &from_expr),
                            Box::new(move |backtrace| {
                                from.resolve(&backtrace)?
                                    .borrow()
                                    .as_attr_set()
                                    .unwrap()
                                    .get(&attr)
                                    .cloned()
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
                        let scope = self.clone();
                        let attr = attr.clone();

                        LazyNixValue::new_eval(
                            self.new_backtrace(backtrace, &attr_node),
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

                    out.borrow_mut()
                        .as_attr_set_mut()
                        .unwrap()
                        .insert(attr, value.wrap_var());
                }

                Ok(out)
            }
            ast::Entry::AttrpathValue(entry) => self.insert_to_attrset(
                backtrace,
                out,
                entry.attrpath().unwrap(),
                entry.value().unwrap(),
            ),
        }
    }

    fn new_backtrace(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: &impl AstNode,
    ) -> NixBacktrace {
        NixBacktrace(
            Rc::new(NixSpan::from_ast_node(&self.file, node)),
            Some(backtrace.clone()).into(),
            NixBacktraceKind::None,
        )
    }
}

impl Scope {
    pub fn visit_expr(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Expr,
    ) -> NixResult<NixVar> {
        let backtrace = &backtrace.visit(&self.file, &node);

        match node {
            ast::Expr::Apply(node) => self.visit_apply(backtrace, node),
            ast::Expr::Assert(node) => self.visit_assert(backtrace, node),
            ast::Expr::AttrSet(node) => self.visit_attrset(backtrace, node),
            ast::Expr::BinOp(node) => self.visit_binop(backtrace, node),
            ast::Expr::Error(node) => self.visit_error(backtrace, node),
            ast::Expr::HasAttr(node) => self.visit_hasattr(backtrace, node),
            ast::Expr::Ident(node) => self.visit_ident(backtrace, node),
            ast::Expr::IfElse(node) => self.visit_ifelse(backtrace, node),
            ast::Expr::Lambda(node) => self.visit_lambda(backtrace, node),
            ast::Expr::LegacyLet(node) => self.visit_legacylet(backtrace, node),
            ast::Expr::LetIn(node) => self.visit_letin(backtrace, node),
            ast::Expr::List(node) => self.visit_list(backtrace, node),
            ast::Expr::Literal(node) => self.visit_literal(backtrace, node),
            ast::Expr::Paren(node) => self.visit_paren(backtrace, node),
            ast::Expr::Path(node) => self.visit_path(backtrace, node),
            ast::Expr::Root(node) => self.visit_root(backtrace, node),
            ast::Expr::Select(node) => self.visit_select(backtrace, node),
            ast::Expr::Str(node) => self.visit_str(backtrace, node),
            ast::Expr::UnaryOp(node) => self.visit_unaryop(backtrace, node),
            ast::Expr::With(node) => self.visit_with(backtrace, node),
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_apply(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Apply,
    ) -> NixResult<NixVar> {
        let lambda_backtrace = backtrace.change_span((&self.file, &node.lambda().unwrap()));

        self.visit_expr(&lambda_backtrace, node.lambda().unwrap())?
            .resolve(&lambda_backtrace)?
            .borrow()
            .as_lambda()
            .ok_or_else(|| todo!("Error handling: Lambda cast"))
            .and_then(|l| {
                let backtrace = &backtrace.change_span((&self.file, &node.argument().unwrap()));

                let argument = self.visit_expr(backtrace, node.argument().unwrap())?;
                l.call(backtrace, argument)
            })
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_assert(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Assert,
    ) -> NixResult<NixVar> {
        let condition = self
            .visit_expr(backtrace, node.condition().unwrap())?
            .resolve(backtrace)?;

        let Some(condition) = condition.borrow().as_bool() else {
            todo!("Error handling")
        };

        if condition {
            node.body().map_or_else(
                || Ok(NixValue::Null.wrap_var()),
                |expr| self.visit_expr(backtrace, expr),
            )
        } else {
            Err(backtrace.to_labeled_error(
                vec![NixLabel::new(
                    NixSpan::from_ast_node(&self.file, &node.condition().unwrap()).into(),
                    NixLabelMessage::AssertionFailed,
                    NixLabelKind::Error,
                )],
                "assert failed",
            ))
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_attrset(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::AttrSet,
    ) -> NixResult<NixVar> {
        let is_recursive = node.rec_token().is_some();

        if is_recursive {
            let scope = self.clone().new_child();

            for entry in node.entries() {
                scope.insert_entry_to_attrset(backtrace, scope.variables.clone(), entry)?;
            }

            Ok(LazyNixValue::Concrete(scope.variables.clone()).wrap_var())
        } else {
            let out = NixValue::AttrSet(NixAttrSet::new()).wrap();

            for entry in node.entries() {
                self.insert_entry_to_attrset(backtrace, out.clone(), entry)?;
            }

            Ok(LazyNixValue::Concrete(out).wrap_var())
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_binop(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::BinOp,
    ) -> NixResult<NixVar> {
        let lhs = self
            .visit_expr(backtrace, node.lhs().unwrap())?
            .resolve(backtrace)?;

        match node.operator().unwrap() {
            ast::BinOpKind::Concat => lhs
                .borrow()
                .as_list()
                .ok_or_else(|| todo!("Error handling"))
                .and_then(|ref lhs| {
                    let rhs = self
                        .visit_expr(backtrace, node.rhs().unwrap())
                        .and_then(|rhs| rhs.resolve(backtrace))
                        .and_then(|rhs| {
                            rhs.borrow()
                                .as_list()
                                .ok_or_else(|| todo!("Error handling"))
                        })?;

                    let mut out = Vec::with_capacity(lhs.0.len() + rhs.0.len());

                    out.extend(lhs.0.iter().cloned());
                    out.extend(rhs.0.iter().cloned());

                    Ok(NixValue::List(NixList(Rc::new(out))).wrap_var())
                }),

            ast::BinOpKind::Update => {
                if let None = lhs.borrow().as_attr_set() {
                    todo!("Error handling");
                }

                Ok(LazyNixValue::UpdateResolve {
                    lhs,
                    rhs: node.rhs().unwrap(),
                    backtrace: backtrace.clone(),
                    scope: self.clone(),
                }
                .wrap_var())
            }
            ast::BinOpKind::Add => match lhs.borrow().deref() {
                NixValue::String(lhs) => self
                    .visit_expr(backtrace, node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .cast_to_string()
                    .ok_or_else(|| todo!("Error handling"))
                    .map(|rhs| NixValue::String(format!("{lhs}{rhs}")).wrap_var()),
                NixValue::Int(lhs) => self
                    .visit_expr(backtrace, node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .as_int()
                    .ok_or_else(|| todo!("Error handling: Int cast"))
                    .map(|rhs| *lhs + rhs)
                    .map(NixValue::Int)
                    .map(NixValue::wrap_var),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Cannot add",
                    None,
                )),
            },
            ast::BinOpKind::Sub => match lhs.borrow().deref() {
                NixValue::Int(lhs) => self
                    .visit_expr(backtrace, node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .as_int()
                    .ok_or_else(|| todo!("Error handling: Int cast"))
                    .map(|rhs| *lhs - rhs)
                    .map(NixValue::Int)
                    .map(NixValue::wrap_var),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Cannot sub",
                    None,
                )),
            },

            ast::BinOpKind::Mul => match lhs.borrow().deref() {
                NixValue::Int(lhs) => self
                    .visit_expr(backtrace, node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .as_int()
                    .ok_or_else(|| todo!("Error handling: Int cast"))
                    .map(|rhs| *lhs * rhs)
                    .map(NixValue::Int)
                    .map(NixValue::wrap_var),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Cannot mul",
                    None,
                )),
            },
            ast::BinOpKind::Div => match lhs.borrow().deref() {
                NixValue::Int(lhs) => self
                    .visit_expr(backtrace, node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .as_int()
                    .ok_or_else(|| todo!("Error handling: Int cast"))
                    .map(|rhs| *lhs / rhs)
                    .map(NixValue::Int)
                    .map(NixValue::wrap_var),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Cannot div",
                    None,
                )),
            },
            ast::BinOpKind::And => lhs
                .borrow()
                .as_bool()
                .ok_or_else(|| todo!("Error handling"))
                .and_then(|lhs| {
                    lhs.then(|| self.visit_expr(backtrace, node.rhs().unwrap()))
                        .unwrap_or_else(|| Ok(NixValue::Bool(false).wrap_var()))
                }),
            ast::BinOpKind::Equal => self
                .visit_expr(backtrace, node.rhs().unwrap())
                .and_then(|rhs| rhs.resolve(backtrace))
                .and_then(|rhs| rhs.borrow().deref().try_eq(&*lhs.borrow(), backtrace))
                .map(NixValue::Bool)
                .map(NixValue::wrap_var),
            ast::BinOpKind::Implication => lhs
                .borrow()
                .as_bool()
                .ok_or_else(|| todo!("Error handling"))
                .and_then(|lhs| {
                    lhs.then(|| self.visit_expr(backtrace, node.rhs().unwrap()))
                        .unwrap_or_else(|| Ok(NixValue::Bool(true).wrap_var()))
                }),
            ast::BinOpKind::Less => match lhs.borrow().deref() {
                NixValue::Int(lhs) => self
                    .visit_expr(backtrace, node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .as_int()
                    .ok_or_else(|| todo!("Error handling"))
                    .map(|rhs| NixValue::Bool(*lhs < rhs).wrap_var()),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Cannot less",
                    None,
                )),
            },
            ast::BinOpKind::LessOrEq => match lhs.borrow().deref() {
                NixValue::Int(lhs) => self
                    .visit_expr(backtrace, node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .as_int()
                    .ok_or_else(|| todo!("Error handling"))
                    .map(|rhs| NixValue::Bool(*lhs <= rhs).wrap_var()),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Cannot LessOrEq",
                    None,
                )),
            },
            ast::BinOpKind::More => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "More op",
                None,
            )),
            ast::BinOpKind::MoreOrEq => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "MoreOrEq op",
                None,
            )),
            ast::BinOpKind::NotEqual => self
                .visit_expr(backtrace, node.rhs().unwrap())
                .and_then(|rhs| rhs.resolve(backtrace))
                .and_then(|rhs| rhs.borrow().deref().try_eq(&*lhs.borrow(), backtrace))
                .map(std::ops::Not::not)
                .map(NixValue::Bool)
                .map(NixValue::wrap_var),
            ast::BinOpKind::Or => lhs
                .borrow()
                .as_bool()
                .ok_or_else(|| todo!("Error handling"))
                .and_then(|lhs| {
                    (!lhs)
                        .then(|| self.visit_expr(backtrace, node.rhs().unwrap()))
                        .unwrap_or_else(|| Ok(NixValue::Bool(true).wrap_var()))
                }),
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_error(
        self: &Rc<Self>,
        _backtrace: &NixBacktrace,
        node: ast::Error,
    ) -> NixResult<NixVar> {
        Err(NixError::todo(
            NixSpan::from_ast_node(&self.file, &node).into(),
            "Error Expr",
            None,
        ))
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_hasattr(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::HasAttr,
    ) -> NixResult<NixVar> {
        let value = self.visit_expr(backtrace, node.expr().unwrap())?;

        let has_attr = self
            .resolve_attr_path(backtrace, value, node.attrpath().unwrap().attrs())?
            .is_ok();

        Ok(NixValue::Bool(has_attr).wrap_var())
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_ident(
        self: &Rc<Self>,
        _backtrace: &NixBacktrace,
        node: ast::Ident,
    ) -> NixResult<NixVar> {
        let ident = node.ident_token().unwrap();
        let varname = ident.text().to_string();

        self.get_variable(varname.clone()).ok_or_else(|| {
            NixError::from_message(
                NixLabel::new(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    NixLabelMessage::VariableNotFound,
                    NixLabelKind::Error,
                ),
                format!("Variable '\x1b[1;95m{varname}\x1b[0m' not found"),
            )
        })
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_ifelse(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::IfElse,
    ) -> NixResult<NixVar> {
        let condition = self
            .visit_expr(backtrace, node.condition().unwrap())?
            .resolve(backtrace)?;

        let Some(condition) = condition.borrow().as_bool() else {
            todo!("Error handling")
        };

        if condition {
            self.visit_expr(backtrace, node.body().unwrap())
        } else {
            self.visit_expr(backtrace, node.else_body().unwrap())
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_lambda(
        self: &Rc<Self>,
        _backtrace: &NixBacktrace,
        node: ast::Lambda,
    ) -> NixResult<NixVar> {
        let param = match node.param().unwrap() {
            ast::Param::Pattern(pattern) => NixLambdaParam::Pattern(pattern),
            ast::Param::IdentParam(ident) => NixLambdaParam::Ident(
                ident
                    .ident()
                    .unwrap()
                    .ident_token()
                    .unwrap()
                    .text()
                    .to_owned(),
            ),
        };

        Ok(
            NixValue::Lambda(NixLambda::Apply(self.clone(), param, node.body().unwrap()))
                .wrap_var(),
        )
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_legacylet(
        self: &Rc<Self>,
        _backtrace: &NixBacktrace,
        _node: ast::LegacyLet,
    ) -> NixResult<NixVar> {
        unimplemented!("This is legacy")
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_letin(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::LetIn,
    ) -> NixResult<NixVar> {
        for entry in node.entries() {
            self.insert_entry_to_attrset(
                &self.new_backtrace(backtrace, &entry),
                self.variables.clone(),
                entry,
            )?;
        }

        let body = node.body().unwrap();

        Ok(LazyNixValue::Pending(
            backtrace.child_none(&self.file, &body),
            self.clone().new_child(),
            body,
        )
        .wrap_var())
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_list(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::List,
    ) -> NixResult<NixVar> {
        Ok(NixValue::List(NixList(Rc::new(
            node.items()
                .map(|expr| {
                    LazyNixValue::Pending(self.new_backtrace(backtrace, &expr), self.clone(), expr)
                        .wrap_var()
                })
                .collect(),
        )))
        .wrap_var())
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_literal(
        self: &Rc<Self>,
        _backtrace: &NixBacktrace,
        node: ast::Literal,
    ) -> NixResult<NixVar> {
        match node.kind() {
            ast::LiteralKind::Float(value) => {
                Ok(NixValue::Float(value.value().unwrap()).wrap_var())
            }
            ast::LiteralKind::Integer(value) => {
                Ok(NixValue::Int(value.value().unwrap()).wrap_var())
            }
            ast::LiteralKind::Uri(_) => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "Uri literal",
                None,
            )),
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_paren(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Paren,
    ) -> NixResult<NixVar> {
        self.visit_expr(backtrace, node.expr().unwrap())
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_path(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Path,
    ) -> NixResult<NixVar> {
        let mut path = String::new();

        for (idx, part) in node.parts().enumerate() {
            match part {
                ast::InterpolPart::Literal(str) => {
                    let str = str.syntax().text();

                    if idx == 0 {
                        if &str[0..1] == "/" {
                            path += str;
                        } else {
                            let dirname = self.file.path.parent().expect("Cannot get parent");

                            if str.get(1..2) == Some(".") {
                                let Some(parent) = dirname.parent() else {
                                    return Err(NixError::todo(
                                        NixSpan::from_ast_node(&self.file, &node).into(),
                                        "Error handling: path doesn't have parent",
                                        None,
                                    ));
                                };
                                path += &parent.display().to_string();
                                path += &str[2..];
                            } else {
                                path += &dirname.display().to_string();
                                path += &str[1..];
                            }
                        }
                    } else {
                        if path.chars().rev().next() != Some('/') {
                            path += "/";
                        }

                        path += str;
                    }
                }
                ast::InterpolPart::Interpolation(interpol) => {
                    let str = self
                        .visit_expr(backtrace, interpol.expr().unwrap())?
                        .resolve(backtrace)?
                        .borrow()
                        .cast_to_string()
                        .unwrap();

                    if idx == 1 && path.get(0..1) == Some("/") && str.get(0..1) == Some("/") {
                        path.pop();
                    }

                    path += &str;
                }
            }
        }

        Ok(NixValue::Path(path.try_into().expect("TODO: Error handling")).wrap_var())
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    #[nix_macros::profile]
    pub fn visit_root(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Root,
    ) -> NixResult<NixVar> {
        self.visit_expr(backtrace, node.expr().unwrap())
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_select(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Select,
    ) -> NixResult<NixVar> {
        let var = self.visit_expr(backtrace, node.expr().unwrap())?;

        let var = self.resolve_attr_path(backtrace, var, node.attrpath().unwrap().attrs())?;

        if var.is_err() && node.default_expr().is_some() {
            self.visit_expr(backtrace, node.default_expr().unwrap())
        } else {
            var
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_str(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::Str,
    ) -> NixResult<NixVar> {
        let mut content = String::new();

        for part in node.parts() {
            match part {
                ast::InterpolPart::Literal(str) => {
                    content += str.syntax().text();
                }
                ast::InterpolPart::Interpolation(interpol) => {
                    content += &self
                        .visit_expr(backtrace, interpol.expr().unwrap())?
                        .resolve(backtrace)?
                        .borrow()
                        .cast_to_string()
                        .unwrap();
                }
            }
        }

        Ok(NixValue::String(content).wrap_var())
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_unaryop(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::UnaryOp,
    ) -> NixResult<NixVar> {
        let value = self
            .visit_expr(backtrace, node.expr().unwrap())?
            .resolve(backtrace)?;
        let value = value.borrow();

        match node.operator().unwrap() {
            ast::UnaryOpKind::Invert => {
                let Some(value) = value.as_bool() else {
                    todo!("Error handling");
                };

                Ok(NixValue::Bool(!value).wrap_var())
            }
            ast::UnaryOpKind::Negate => match value.deref() {
                NixValue::Int(v) => Ok(NixValue::Int(-v).wrap_var()),
                NixValue::Float(v) => Ok(NixValue::Float(-v).wrap_var()),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Error Handling: should be a number",
                    None,
                )),
            },
        }
    }

    #[cfg_attr(any(feature = "debug", not(debug_assertions)), inline(always))]
    pub fn visit_with(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        node: ast::With,
    ) -> NixResult<NixVar> {
        let namespace = self
            .visit_expr(backtrace, node.namespace().unwrap())?
            .resolve(backtrace)?;

        if !namespace.borrow().is_attr_set() {
            todo!("Error handling")
        }

        let scope = self.clone().new_child_from(namespace);

        scope.visit_expr(backtrace, node.body().unwrap())
    }
}
