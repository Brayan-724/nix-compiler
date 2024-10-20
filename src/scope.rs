use std::collections::HashMap;
use std::rc::Rc;

use rnix::ast;

use crate::value::{AsAttrSet, NixValue, NixValueWrapped};

#[derive(Debug)]
pub struct Scope {
    pub variables: NixValueWrapped,
    pub parent: Option<Rc<Scope>>,
}

impl Scope {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            variables: NixValue::AttrSet(HashMap::new()).wrap(),
            parent: None,
        })
    }

    pub fn new_child(self: Rc<Self>) -> Rc<Scope> {
        Rc::new(Scope {
            variables: NixValue::AttrSet(HashMap::new()).wrap(),
            parent: Some(self),
        })
    }

    pub fn set_variable(
        self: &Rc<Self>,
        varname: String,
        value: NixValueWrapped,
    ) -> Option<NixValueWrapped> {
        self.variables
            .borrow_mut()
            .as_attr_set_mut()
            .unwrap()
            .insert(varname, value)
    }

    pub fn get_variable(self: &Rc<Self>, varname: String) -> Option<NixValueWrapped> {
        self.variables
            .borrow()
            .as_attr_set()
            .unwrap()
            .get(&varname)
            .cloned()
            .or_else(|| {
                self.parent
                    .as_ref()
                    .and_then(|parent| parent.get_variable(varname))
            })
    }

    pub fn resolve_attr_path<'a>(
        self: &Rc<Self>,
        value: NixValueWrapped,
        attr_path: impl Iterator<Item = ast::Attr>,
    ) -> Option<NixValueWrapped> {
        let mut attr_path: Vec<_> = attr_path.collect();
        let last_attr = attr_path.pop().unwrap();

        let attr_set = self.resolve_attr_set_path(value, attr_path.into_iter());

        let last_attr = self.resolve_attr(last_attr);

        let attr_set = attr_set.borrow();

        attr_set.get(&last_attr).unwrap()
    }

    pub fn resolve_attr_set_path<'a>(
        self: &Rc<Self>,
        value: NixValueWrapped,
        mut attr_path: impl Iterator<Item = ast::Attr>,
    ) -> NixValueWrapped {
        if let Some(attr) = attr_path.next() {
            let attr = self.resolve_attr(attr);

            let set_value = value.borrow().get(&attr).unwrap();

            let Some(set_value) = set_value else {
                let (last, _) = value
                    .borrow_mut()
                    .insert(attr, NixValue::AttrSet(HashMap::new()).wrap())
                    .unwrap();

                return self.resolve_attr_set_path(last, attr_path);
            };

            if !set_value.borrow().is_attr_set() {
                let set_value = set_value.borrow();
                todo!("Error handling for {set_value:#}")
            };

            self.resolve_attr_set_path(set_value, attr_path)
        } else {
            value
        }
    }

    pub fn resolve_attr(self: &Rc<Self>, attr: ast::Attr) -> String {
        match attr {
            ast::Attr::Ident(ident) => ident.ident_token().unwrap().text().to_owned(),
            ast::Attr::Dynamic(_) => todo!(),
            ast::Attr::Str(_) => todo!(),
        }
    }
}
