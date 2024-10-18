use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

use rnix::ast::{self, AstToken, HasEntry};

use crate::scope::Scope;
use crate::value::{AsAttrSet, AsString, NixValue, NixValueWrapped};

#[allow(unused_variables, reason = "todo")]
impl Scope {
    fn insert_to_attrset(
        self: &Rc<Self>,
        out: NixValueWrapped,
        attrpath: ast::Attrpath,
        attr_value: ast::Expr,
    ) {
        let mut attr_path: Vec<ast::Attr> = attrpath.attrs().collect();
        let last_attr_path = attr_path
            .pop()
            .expect("Attrpath requires at least one attribute");

        let target = self.resolve_attr_set_path(out.clone(), attr_path.into_iter());

        if !target.borrow().is_attr_set() {
            todo!("Error handling")
        };

        let attr = self.resolve_attr(last_attr_path);
        let child = self.clone().new_child().visit_expr(attr_value);

        let mut target = target.borrow_mut();
        let set = target.as_attr_set_mut().unwrap();

        set.insert(attr, child);
    }

    fn insert_entry_to_attrset(self: &Rc<Self>, out: NixValueWrapped, entry: ast::Entry) {
        match entry {
            ast::Entry::Inherit(_) => todo!(),
            ast::Entry::AttrpathValue(entry) => {
                self.insert_to_attrset(out, entry.attrpath().unwrap(), entry.value().unwrap())
            }
        }
    }

    pub fn visit_expr(self: &Rc<Self>, node: ast::Expr) -> NixValueWrapped {
        match node {
            ast::Expr::Apply(node) => self.visit_apply(node),
            ast::Expr::Assert(node) => self.visit_assert(node),
            ast::Expr::AttrSet(node) => self.visit_attrset(node),
            ast::Expr::BinOp(node) => self.visit_binop(node),
            ast::Expr::Error(node) => self.visit_error(node),
            ast::Expr::HasAttr(node) => self.visit_hasattr(node),
            ast::Expr::Ident(node) => self.visit_ident(node),
            ast::Expr::IfElse(node) => self.visit_ifelse(node),
            ast::Expr::Lambda(node) => self.visit_lambda(node),
            ast::Expr::LegacyLet(node) => self.visit_legacylet(node),
            ast::Expr::LetIn(node) => self.visit_letin(node),
            ast::Expr::List(node) => self.visit_list(node),
            ast::Expr::Literal(node) => self.visit_literal(node),
            ast::Expr::Paren(node) => self.visit_paren(node),
            ast::Expr::Path(node) => self.visit_path(node),
            ast::Expr::Root(node) => self.visit_root(node),
            ast::Expr::Select(node) => self.visit_select(node),
            ast::Expr::Str(node) => self.visit_str(node),
            ast::Expr::UnaryOp(node) => self.visit_unaryop(node),
            ast::Expr::With(node) => self.visit_with(node),
        }
    }

    fn visit_apply(self: &Rc<Self>, node: ast::Apply) -> NixValueWrapped {
        todo!()
    }

    fn visit_assert(self: &Rc<Self>, node: ast::Assert) -> NixValueWrapped {
        todo!()
    }

    fn visit_attrset(self: &Rc<Self>, node: ast::AttrSet) -> NixValueWrapped {
        let is_recursive = node.rec_token().is_some();

        let out = NixValue::AttrSet(HashMap::new()).wrap();

        for entry in node.entries() {
            self.insert_entry_to_attrset(out.clone(), entry);
        }

        out
    }

    fn visit_binop(self: &Rc<Self>, node: ast::BinOp) -> NixValueWrapped {
        todo!()
    }

    fn visit_error(self: &Rc<Self>, node: ast::Error) -> NixValueWrapped {
        todo!()
    }

    fn visit_hasattr(self: &Rc<Self>, node: ast::HasAttr) -> NixValueWrapped {
        todo!()
    }

    fn visit_ident(self: &Rc<Self>, node: ast::Ident) -> NixValueWrapped {
        let varname = node.ident_token().unwrap().text().to_string();
        self.get_variable(varname).unwrap_or_default()
    }

    fn visit_ifelse(self: &Rc<Self>, node: ast::IfElse) -> NixValueWrapped {
        todo!()
    }

    fn visit_lambda(self: &Rc<Self>, node: ast::Lambda) -> NixValueWrapped {
        todo!()
    }

    fn visit_legacylet(self: &Rc<Self>, node: ast::LegacyLet) -> NixValueWrapped {
        todo!()
    }

    fn visit_letin(self: &Rc<Self>, node: ast::LetIn) -> NixValueWrapped {
        for entry in node.entries() {
            self.insert_entry_to_attrset(self.variables.clone(), entry);
        }

        let body = node.body().unwrap();

        self.visit_expr(body)
    }

    fn visit_list(self: &Rc<Self>, node: ast::List) -> NixValueWrapped {
        todo!()
    }

    fn visit_literal(self: &Rc<Self>, node: ast::Literal) -> NixValueWrapped {
        todo!()
    }

    fn visit_paren(self: &Rc<Self>, node: ast::Paren) -> NixValueWrapped {
        todo!()
    }

    fn visit_path(self: &Rc<Self>, node: ast::Path) -> NixValueWrapped {
        todo!()
    }

    fn visit_root(self: &Rc<Self>, node: ast::Root) -> NixValueWrapped {
        todo!()
    }

    fn visit_select(self: &Rc<Self>, node: ast::Select) -> NixValueWrapped {
        let var = self.visit_expr(node.expr().unwrap());
        self.resolve_attr_path(var, node.attrpath().unwrap().attrs()).unwrap_or_default()
    }

    fn visit_str(self: &Rc<Self>, node: ast::Str) -> NixValueWrapped {
        let mut content = String::new();

        for part in node.parts() {
            match part {
                ast::InterpolPart::Literal(str) => {
                    content += str.syntax().text();
                }
                ast::InterpolPart::Interpolation(interpol) => {
                    content += &self
                        .visit_expr(interpol.expr().unwrap())
                        .borrow()
                        .as_string()
                        .unwrap();
                }
            }
        }

        Rc::new(RefCell::new(NixValue::String(content)))
    }

    fn visit_unaryop(self: &Rc<Self>, node: ast::UnaryOp) -> NixValueWrapped {
        todo!()
    }

    fn visit_with(self: &Rc<Self>, node: ast::With) -> NixValueWrapped {
        todo!()
    }
}
