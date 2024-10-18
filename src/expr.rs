use std::cell::RefCell;
use std::collections::HashMap;
use std::iter;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use rnix::ast::{self, HasEntry};

use crate::scope::Scope;
use crate::value::{AsAttrSet, NixValue};

#[allow(unused_variables, reason = "todo")]
impl Scope {
    fn insert_to_attrset(
        self: &Rc<Self>,
        out: Rc<RefCell<NixValue>>,
        attrpath: ast::Attrpath,
        attr_value: ast::Expr,
    ) {
        let mut attr_path: Vec<ast::Attr> = attrpath.attrs().collect();
        let last_attr_path = attr_path
            .pop()
            .expect("Attrpath requires at least one attribute");

        let target = self.resolve_attr_path(out.clone(), attr_path.into_iter());

        {
            let target_ref = target.borrow();
            let NixValue::AttrSet(_) = target_ref.deref() else {
                todo!("Error handling")
            };
        }

        let attr = self.resolve_attr(last_attr_path);
        let child = self.clone().new_child().visit_expr(attr_value);


        let mut target = target.borrow_mut();
        let set = target.as_attr_set_mut().unwrap();

        set.insert(attr, child);
    }

    fn insert_entry_to_attrset(self: &Rc<Self>, out: Rc<RefCell<NixValue>>, entry: ast::Entry) {
        match entry {
            ast::Entry::Inherit(_) => todo!(),
            ast::Entry::AttrpathValue(entry) => {
                self.insert_to_attrset(out, entry.attrpath().unwrap(), entry.value().unwrap())
            }
        }
    }

    pub fn visit_expr(self: &Rc<Self>, node: ast::Expr) -> Rc<RefCell<NixValue>> {
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

    fn visit_apply(self: &Rc<Self>, node: ast::Apply) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_assert(self: &Rc<Self>, node: ast::Assert) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_attrset(self: &Rc<Self>, node: ast::AttrSet) -> Rc<RefCell<NixValue>> {
        let out = Rc::new(RefCell::new(NixValue::AttrSet(HashMap::new())));

        for entry in node.entries() {
            self.insert_entry_to_attrset(out.clone(), entry);
        }

        out
    }

    fn visit_binop(self: &Rc<Self>, node: ast::BinOp) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_error(self: &Rc<Self>, node: ast::Error) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_hasattr(self: &Rc<Self>, node: ast::HasAttr) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_ident(self: &Rc<Self>, node: ast::Ident) -> Rc<RefCell<NixValue>> {
        let varname = node.ident_token().unwrap().text().to_string();
        self.get_variable(varname).unwrap_or_default()
    }

    fn visit_ifelse(self: &Rc<Self>, node: ast::IfElse) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_lambda(self: &Rc<Self>, node: ast::Lambda) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_legacylet(self: &Rc<Self>, node: ast::LegacyLet) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_letin(self: &Rc<Self>, node: ast::LetIn) -> Rc<RefCell<NixValue>> {
        for entry in node.entries() {
            self.insert_entry_to_attrset(self.variables.clone(), entry);
        }

        let body = node.body().unwrap();

        self.visit_expr(body)
    }

    fn visit_list(self: &Rc<Self>, node: ast::List) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_literal(self: &Rc<Self>, node: ast::Literal) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_paren(self: &Rc<Self>, node: ast::Paren) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_path(self: &Rc<Self>, node: ast::Path) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_root(self: &Rc<Self>, node: ast::Root) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_select(self: &Rc<Self>, node: ast::Select) -> Rc<RefCell<NixValue>> {
        let var = self.visit_expr(node.expr().unwrap());
        self.resolve_attr_path(var, node.attrpath().unwrap().attrs())
    }

    fn visit_str(self: &Rc<Self>, node: ast::Str) -> Rc<RefCell<NixValue>> {
        Rc::new(RefCell::new(NixValue::String(node.to_string())))
    }

    fn visit_unaryop(self: &Rc<Self>, node: ast::UnaryOp) -> Rc<RefCell<NixValue>> {
        todo!()
    }

    fn visit_with(self: &Rc<Self>, node: ast::With) -> Rc<RefCell<NixValue>> {
        todo!()
    }
}
