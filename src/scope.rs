mod file;

use std::collections::HashMap;
use std::rc::Rc;

use rnix::ast;
use rowan::ast::AstNode;

pub use file::FileScope;

use crate::result::{NixLabel, NixLabelKind, NixLabelMessage};
use crate::{
    builtins, AsAttrSet, AsString, NixError, NixResult, NixValue, NixValueBuiltin, NixValueWrapped,
    NixVar,
};

#[derive(Debug, PartialEq, Eq)]
pub struct Scope {
    pub file: Rc<FileScope>,
    pub variables: NixValueWrapped,
    pub parent: Option<Rc<Scope>>,
}

impl Scope {
    pub fn new_with_builtins(file_scope: Rc<FileScope>) -> Rc<Self> {
        macro_rules! insert {
            ($ident:ident; $key:ident = $value:expr) => {
                $ident.insert(stringify!($key).to_owned(), $value.wrap_var())
            };
        }

        let mut globals = HashMap::new();
        let builtins = builtins::get_builtins();

        insert!(globals; abort = NixValue::Builtin(NixValueBuiltin::Abort));
        insert!(globals; import = NixValue::Builtin(NixValueBuiltin::Import));
        insert!(globals; toString = NixValue::Builtin(NixValueBuiltin::ToString));
        insert!(globals; builtins = builtins);

        let parent = Rc::new(Scope {
            file: file_scope.clone(),
            variables: NixValue::AttrSet(globals).wrap(),
            parent: None,
        });

        Rc::new(Self {
            file: file_scope,
            variables: NixValue::AttrSet(HashMap::new()).wrap(),
            parent: Some(parent),
        })
    }

    pub fn new_child(self: Rc<Self>) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables: NixValue::AttrSet(HashMap::new()).wrap(),
            parent: Some(self),
        })
    }

    pub fn new_child_from(self: Rc<Self>, variables: NixValueWrapped) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables,
            parent: Some(self),
        })
    }

    pub fn set_variable(self: &Rc<Self>, varname: String, value: NixVar) -> Option<NixVar> {
        self.variables
            .borrow_mut()
            .as_attr_set_mut()
            .unwrap()
            .insert(varname, value)
    }

    pub fn get_variable(self: &Rc<Self>, varname: String) -> Option<NixVar> {
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
        attr_path: ast::Attrpath,
    ) -> NixResult<NixVar> {
        let mut attrs: Vec<_> = attr_path.attrs().collect();
        let last_attr = attrs.pop().unwrap();

        let attr_set = self.resolve_attr_set_path(value, attrs.into_iter())?;

        let attr = self.resolve_attr(&last_attr)?;

        let attr_set = attr_set.borrow();

        attr_set.get(&attr).unwrap().ok_or_else(|| {
            let label = NixLabel::from_syntax_node(
                &self.file,
                last_attr.syntax(),
                NixLabelMessage::VariableNotFound,
                NixLabelKind::Error,
            );
            NixError::from_message(
                label,
                format!("Attribute '\x1b[1;95m{attr}\x1b[0m' missing"),
            )
        })
    }

    pub fn resolve_attr_set_path<'a>(
        self: &Rc<Self>,
        value: NixValueWrapped,
        mut attr_path: impl Iterator<Item = ast::Attr>,
    ) -> NixResult {
        if let Some(attr) = attr_path.next() {
            let attr = self.resolve_attr(&attr)?;

            let set_value = value.borrow().get(&attr).unwrap();

            let Some(set_value) = set_value else {
                let (last, _) = value
                    .borrow_mut()
                    .insert(attr, NixValue::AttrSet(HashMap::new()).wrap_var())
                    .unwrap();

                return self.resolve_attr_set_path(last.resolve()?, attr_path);
            };

            let set_value = set_value.resolve()?;

            if !set_value.borrow().is_attr_set() {
                todo!("Error handling for {:#}", set_value.borrow());
            };

            self.resolve_attr_set_path(set_value, attr_path)
        } else {
            Ok(value)
        }
    }

    pub fn resolve_attr(self: &Rc<Self>, attr: &ast::Attr) -> NixResult<String> {
        match attr {
            ast::Attr::Ident(ident) => Ok(ident.ident_token().unwrap().text().to_owned()),
            ast::Attr::Dynamic(dynamic) => Ok(self
                .visit_expr(dynamic.expr().unwrap())?
                .borrow()
                .as_string()
                .expect("Cannot cast as string")),
            ast::Attr::Str(_) => todo!(),
        }
    }
}
