use std::ops::Deref;
use std::rc::Rc;

use rnix::ast::{self, AstToken, HasEntry};
use rowan::ast::AstNode;

use crate::result::{NixBacktrace, NixSpan};
use crate::value::{NixLambda, NixList};
use crate::{
    LazyNixValue, NixAttrSet, NixError, NixLabel, NixLabelKind, NixLabelMessage, NixLambdaParam,
    NixResult, NixValue, NixValueWrapped, NixVar, Scope,
};

impl Scope {
    fn insert_to_attrset(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        out: NixValueWrapped,
        attrpath: ast::Attrpath,
        attr_value: ast::Expr,
    ) -> NixResult {
        let mut attr_path: Vec<ast::Attr> = attrpath.attrs().collect();
        let last_attr_path = attr_path
            .pop()
            .expect("Attrpath requires at least one attribute");

        let target =
            self.resolve_attr_set_path(backtrace.clone(), out.clone(), attr_path.into_iter())?;

        if !target.borrow().is_attr_set() {
            todo!("Error handling")
        };

        let attr = self.resolve_attr(backtrace.clone(), &last_attr_path)?;

        let child = LazyNixValue::Pending(
            self.new_backtrace(backtrace.clone(), &attr_value),
            self.clone().new_child(),
            attr_value,
        )
        .wrap_var();

        let mut target = target.borrow_mut();
        let set = target.as_attr_set_mut().unwrap();

        set.insert(attr, child);

        Ok(out)
    }

    fn insert_entry_to_attrset(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        out: NixValueWrapped,
        entry: ast::Entry,
    ) -> NixResult {
        match entry {
            ast::Entry::Inherit(entry) => {
                let from = entry.from().map(|from| {
                    (
                        LazyNixValue::Pending(
                            self.new_backtrace(backtrace.clone(), &from),
                            self.clone(),
                            from.expr().unwrap(),
                        )
                        .wrap_var(),
                        from,
                    )
                });

                for attr_node in entry.attrs() {
                    let attr = self.resolve_attr(backtrace.clone(), &attr_node)?;

                    if let Some((from, from_expr)) = &from {
                        let value = {
                            let from = from.clone();
                            let from_expr = from_expr.clone();
                            let attr = attr.clone();
                            let attr_node = attr_node.clone();
                            let file = self.file.clone();

                            LazyNixValue::new_eval(
                                self.new_backtrace(backtrace.clone(), &from_expr),
                                Box::new(move |backtrace| {
                                    from.resolve(backtrace.clone())?
                                        .borrow()
                                        .as_attr_set()
                                        .unwrap()
                                        .get(&attr)
                                        .cloned()
                                        .ok_or_else(|| NixError {
                                            message: format!(
                                                "Attribute '\x1b[1;95m{attr}\x1b[0m' missing"
                                            ),
                                            labels: vec![
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
                                            backtrace: Some(backtrace.clone()),
                                        })?
                                        .resolve(backtrace)
                                }),
                            )
                        };

                        out.borrow_mut()
                            .as_attr_set_mut()
                            .unwrap()
                            .insert(attr, value.wrap_var());
                    } else {
                        let value = {
                            let scope = self.clone();
                            let attr = attr.clone();
                            let attr_node = attr_node.clone();
                            let file = self.file.clone();

                            LazyNixValue::new_eval(
                                self.new_backtrace(backtrace.clone(), &attr_node),
                                Box::new(move |backtrace| {
                                    let Some(value) = scope.get_variable(attr.clone()) else {
                                        return Err(NixError::from_message(
                                            NixLabel::new(
                                                NixSpan::from_ast_node(&file, &attr_node).into(),
                                                NixLabelMessage::VariableNotFound,
                                                NixLabelKind::Error,
                                            ),
                                            format!("Variable '{attr} not found"),
                                        ));
                                    };

                                    value.resolve(backtrace)
                                }),
                            )
                        };

                        out.borrow_mut()
                            .as_attr_set_mut()
                            .unwrap()
                            .insert(attr, value.wrap_var());
                    }
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
        backtrace: Rc<NixBacktrace>,
        node: &impl AstNode,
    ) -> Rc<NixBacktrace> {
        Rc::new(NixBacktrace(
            Rc::new(NixSpan::from_ast_node(&self.file, node)),
            Some(backtrace),
        ))
    }

    pub fn visit_expr(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::Expr,
    ) -> NixResult<NixVar> {
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

    pub fn visit_apply(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::Apply,
    ) -> NixResult<NixVar> {
        self.visit_expr(
            self.new_backtrace(backtrace.clone(), &node),
            node.lambda().unwrap(),
        )?
        .resolve(backtrace.clone())?
        .borrow()
        .as_lambda()
        .ok_or_else(|| todo!("Error handling: Lambda cast"))
        .and_then(|l| {
            let argument = self.visit_expr(backtrace.clone(), node.argument().unwrap())?;
            l.call(backtrace, argument)
        })
    }

    pub fn visit_assert(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::Assert,
    ) -> NixResult<NixVar> {
        let condition = self
            .visit_expr(backtrace.clone(), node.condition().unwrap())?
            .resolve(backtrace.clone())?;
        let Some(condition) = condition.borrow().as_bool() else {
            todo!("Error handling")
        };

        if condition {
            node.body().map_or_else(
                || Ok(NixValue::Null.wrap_var()),
                |expr| self.visit_expr(backtrace.clone(), expr),
            )
        } else {
            Err(NixError {
                message: "assert failed".to_owned(),
                labels: vec![NixLabel::new(
                    NixSpan::from_ast_node(&self.file, &node.condition().unwrap()).into(),
                    NixLabelMessage::AssertionFailed,
                    NixLabelKind::Error,
                )],
                backtrace: Some(backtrace),
            })
        }
    }

    pub fn visit_attrset(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::AttrSet,
    ) -> NixResult<NixVar> {
        let is_recursive = node.rec_token().is_some();

        if is_recursive {
            let scope = self.clone().new_child();

            for entry in node.entries() {
                scope.insert_entry_to_attrset(backtrace.clone(), scope.variables.clone(), entry)?;
            }

            Ok(LazyNixValue::Concrete(scope.variables.clone()).wrap_var())
        } else {
            let out = NixValue::AttrSet(NixAttrSet::new()).wrap();

            for entry in node.entries() {
                self.insert_entry_to_attrset(backtrace.clone(), out.clone(), entry)?;
            }

            Ok(LazyNixValue::Concrete(out).wrap_var())
        }
    }

    pub fn visit_binop(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::BinOp,
    ) -> NixResult<NixVar> {
        let lhs = self
            .visit_expr(backtrace.clone(), node.lhs().unwrap())?
            .resolve(backtrace.clone())?;

        match node.operator().unwrap() {
            ast::BinOpKind::Concat => lhs
                .borrow()
                .as_list()
                .ok_or_else(|| todo!("Error handling"))
                .and_then(|ref lhs| {
                    let rhs = self
                        .visit_expr(backtrace.clone(), node.rhs().unwrap())
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
                    backtrace,
                    scope: self.clone(),
                }
                .wrap_var())
            }
            ast::BinOpKind::Add => match lhs.borrow().deref() {
                NixValue::String(lhs) => self
                    .visit_expr(backtrace.clone(), node.rhs().unwrap())?
                    .resolve(backtrace)?
                    .borrow()
                    .as_string()
                    .ok_or_else(|| todo!("Error handling"))
                    .map(|rhs| NixValue::String(format!("{lhs}{rhs}")).wrap_var()),
                _ => Err(NixError::todo(
                    NixSpan::from_ast_node(&self.file, &node).into(),
                    "Cannot add",
                    None,
                )),
            },
            ast::BinOpKind::Sub => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "Sub op",
                None,
            )),
            ast::BinOpKind::Mul => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "Mul op",
                None,
            )),
            ast::BinOpKind::Div => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "Div op",
                None,
            )),
            ast::BinOpKind::And => lhs
                .borrow()
                .as_bool()
                .ok_or_else(|| todo!("Error handling"))
                .and_then(|lhs| {
                    lhs.then(|| self.visit_expr(backtrace, node.rhs().unwrap()))
                        .unwrap_or_else(|| Ok(NixValue::Bool(false).wrap_var()))
                }),
            ast::BinOpKind::Equal => self
                .visit_expr(backtrace.clone(), node.rhs().unwrap())
                .and_then(|rhs| rhs.resolve(backtrace))
                .map(|rhs| rhs.borrow().deref().eq(&lhs.borrow()))
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
                    .visit_expr(backtrace.clone(), node.rhs().unwrap())?
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
            ast::BinOpKind::LessOrEq => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "LessOrEq op",
                None,
            )),
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
                .visit_expr(backtrace.clone(), node.rhs().unwrap())
                .and_then(|rhs| rhs.resolve(backtrace))
                .map(|rhs| rhs.borrow().deref().ne(&lhs.borrow()))
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

    pub fn visit_error(
        self: &Rc<Self>,
        _backtrace: Rc<NixBacktrace>,
        node: ast::Error,
    ) -> NixResult<NixVar> {
        Err(NixError::todo(
            NixSpan::from_ast_node(&self.file, &node).into(),
            "Error Expr",
            None,
        ))
    }

    pub fn visit_hasattr(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::HasAttr,
    ) -> NixResult<NixVar> {
        let value = self
            .visit_expr(backtrace.clone(), node.expr().unwrap())?
            .resolve(backtrace.clone())?;

        let has_attr = self
            .resolve_attr_path(backtrace.clone(), value, node.attrpath().unwrap())?
            .and_then(|v| v.resolve(backtrace))
            .is_ok();

        Ok(NixValue::Bool(has_attr).wrap_var())
    }

    pub fn visit_ident(
        self: &Rc<Self>,
        _backtrace: Rc<NixBacktrace>,
        node: ast::Ident,
    ) -> NixResult<NixVar> {
        let node = node.ident_token().unwrap();
        let varname = node.text().to_string();

        self.get_variable(varname.clone()).ok_or_else(|| {
            NixError::from_message(
                NixLabel::new(
                    NixSpan::from_syntax_token(&self.file, &node).into(),
                    NixLabelMessage::VariableNotFound,
                    NixLabelKind::Error,
                ),
                format!("Variable '\x1b[1;95m{varname}\x1b[0m' not found"),
            )
        })
    }

    pub fn visit_ifelse(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::IfElse,
    ) -> NixResult<NixVar> {
        let condition = self
            .visit_expr(backtrace.clone(), node.condition().unwrap())?
            .resolve(backtrace.clone())?;

        let Some(condition) = condition.borrow().as_bool() else {
            todo!("Error handling")
        };

        if condition {
            self.visit_expr(backtrace, node.body().unwrap())
        } else {
            self.visit_expr(backtrace, node.else_body().unwrap())
        }
    }

    pub fn visit_lambda(
        self: &Rc<Self>,
        _backtrace: Rc<NixBacktrace>,
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

    pub fn visit_legacylet(
        self: &Rc<Self>,
        _backtrace: Rc<NixBacktrace>,
        _node: ast::LegacyLet,
    ) -> ! {
        unimplemented!("This is legacy")
    }

    pub fn visit_letin(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::LetIn,
    ) -> NixResult<NixVar> {
        for entry in node.entries() {
            self.insert_entry_to_attrset(
                self.new_backtrace(backtrace.clone(), &entry),
                self.variables.clone(),
                entry,
            )?;
        }

        let body = node.body().unwrap();

        self.clone()
            .new_child()
            .visit_expr(self.new_backtrace(backtrace, &body), body)
    }

    pub fn visit_list(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::List,
    ) -> NixResult<NixVar> {
        Ok(NixValue::List(NixList(Rc::new(
            node.items()
                .map(|expr| {
                    LazyNixValue::Pending(
                        self.new_backtrace(backtrace.clone(), &expr),
                        self.clone(),
                        expr,
                    )
                    .wrap_var()
                })
                .collect(),
        )))
        .wrap_var())
    }

    pub fn visit_literal(
        self: &Rc<Self>,
        _backtrace: Rc<NixBacktrace>,
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

    pub fn visit_paren(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::Paren,
    ) -> NixResult<NixVar> {
        self.visit_expr(backtrace, node.expr().unwrap())
    }

    pub fn visit_path(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
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
                        .visit_expr(backtrace.clone(), interpol.expr().unwrap())?
                        .resolve(backtrace.clone())?
                        .borrow()
                        .as_string()
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

    pub fn visit_root(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::Root,
    ) -> NixResult<NixVar> {
        self.visit_expr(backtrace, node.expr().unwrap())
    }

    pub fn visit_select(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::Select,
    ) -> NixResult<NixVar> {
        let var = self
            .visit_expr(backtrace.clone(), node.expr().unwrap())?
            .resolve(backtrace.clone())?;

        let var = self.resolve_attr_path(backtrace.clone(), var, node.attrpath().unwrap())?;

        if var.is_err() && node.default_expr().is_some() {
            self.visit_expr(backtrace, node.default_expr().unwrap())
        } else {
            var
        }
    }

    pub fn visit_str(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
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
                        .visit_expr(backtrace.clone(), interpol.expr().unwrap())?
                        .resolve(backtrace.clone())?
                        .borrow()
                        .as_string()
                        .unwrap();
                }
            }
        }

        Ok(NixValue::String(content).wrap_var())
    }

    pub fn visit_unaryop(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::UnaryOp,
    ) -> NixResult<NixVar> {
        let value = self
            .visit_expr(backtrace.clone(), node.expr().unwrap())?
            .resolve(backtrace)?;
        let value = value.borrow();

        match node.operator().unwrap() {
            ast::UnaryOpKind::Invert => {
                let Some(value) = value.as_bool() else {
                    todo!("Error handling");
                };

                Ok(NixValue::Bool(!value).wrap_var())
            }
            ast::UnaryOpKind::Negate => Err(NixError::todo(
                NixSpan::from_ast_node(&self.file, &node).into(),
                "Negate op",
                None,
            )),
        }
    }

    pub fn visit_with(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        node: ast::With,
    ) -> NixResult<NixVar> {
        let namespace = self
            .visit_expr(backtrace.clone(), node.namespace().unwrap())?
            .resolve(backtrace.clone())?;

        if !namespace.borrow().is_attr_set() {
            todo!("Error handling")
        }

        let scope = self.clone().new_child_from(namespace);

        scope.visit_expr(backtrace, node.body().unwrap())
    }
}
