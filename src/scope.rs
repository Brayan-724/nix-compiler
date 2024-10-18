use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

use rnix::ast;

use crate::value::{AsAttrSet, NixValue};

#[derive(Debug)]
pub struct Scope {
    pub variables: Rc<RefCell<NixValue>>,
    pub parent: Option<Rc<Scope>>,
}

impl Scope {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            variables: Rc::new(RefCell::new(NixValue::AttrSet(HashMap::new()))),
            parent: None,
        })
    }

    pub fn new_child(self: Rc<Self>) -> Rc<Scope> {
        Rc::new(Scope {
            variables: Rc::new(RefCell::new(NixValue::AttrSet(HashMap::new()))),
            parent: Some(self),
        })
    }

    pub fn get_variable(self: &Rc<Self>, varname: String) -> Option<Rc<RefCell<NixValue>>> {
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
        value: Rc<RefCell<NixValue>>,
        mut attr_path: impl Iterator<Item = ast::Attr>,
    ) -> Rc<RefCell<NixValue>> {
        if let Some(attr) = attr_path.next() {
            let attr = self.resolve_attr(attr);

            println!("{value:#?}");

            let a = {
                let mut set = value.borrow_mut();
                let Some(set) = set.as_attr_set_mut() else {
                    todo!("Errors handling")
                };

                if let Some(set_value) = set.get(&attr) {
                    Ok(set_value.clone())
                } else {
                    let last = Rc::new(RefCell::new(NixValue::AttrSet(HashMap::new())));

                    set.insert(attr.clone(), last.clone());

                    Err(last)
                }
            };

            let set_value = match a {
                Ok(s) => s,
                Err(last) => {
                    return self.resolve_attr_path(last, attr_path);
                }
            };

            let set_value_ref = set_value.borrow();
            let set_value = set_value.clone();

            let NixValue::AttrSet(_) = set_value_ref.deref() else {
                todo!("Error handling")
            };

            self.resolve_attr_path(set_value, attr_path)
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
