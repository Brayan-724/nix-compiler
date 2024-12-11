mod file;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::rc::Rc;

use rnix::ast;

pub use file::FileScope;

use crate::result::{NixLabel, NixLabelKind, NixLabelMessage, NixSpan};
use crate::{
    builtins, flake, AsAttrSet, AsString, NixBacktrace, NixError, NixResult, NixValue,
    NixValueWrapped, NixVar,
};

#[derive(Debug)]
pub struct Scope {
    pub backtrace: Option<Rc<NixBacktrace>>,
    pub file: Rc<FileScope>,
    pub variables: NixValueWrapped,
    pub parent: Option<Rc<Scope>>,
}

impl PartialEq for Scope {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file && self.variables == other.variables && self.parent == other.parent
    }
}

impl Eq for Scope {}

impl Scope {
    pub fn new_with_builtins(file_scope: Rc<FileScope>) -> Rc<Self> {
        macro_rules! insert {
            ($ident:ident; $key:ident = $value:expr) => {
                $ident.insert(stringify!($key).to_owned(), $value.wrap_var())
            };
        }

        let mut globals = HashMap::new();
        let builtins = builtins::get_builtins();

        insert!(globals; abort = builtins::Abort::generate());
        insert!(globals; false = NixValue::Bool(false));
        insert!(globals; import = builtins::Import::generate());
        insert!(globals; map = builtins::Map::generate());
        insert!(globals; null = NixValue::Null);
        insert!(globals; removeAttrs = builtins::RemoveAttrs::generate());
        insert!(globals; toString = builtins::ToString::generate());
        insert!(globals; throw = builtins::Throw::generate());
        insert!(globals; true = NixValue::Bool(true));
        insert!(globals; builtins = builtins);

        let parent = Rc::new(Scope {
            file: file_scope.clone(),
            variables: NixValue::AttrSet(globals).wrap(),
            parent: None,
            backtrace: None,
        });

        Rc::new(Self {
            file: file_scope,
            variables: NixValue::AttrSet(HashMap::new()).wrap(),
            parent: Some(parent),
            backtrace: None,
        })
    }

    pub fn new_child(self: Rc<Self>) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables: NixValue::AttrSet(HashMap::new()).wrap(),
            parent: Some(self),
            backtrace: None,
        })
    }

    pub fn new_child_from(self: Rc<Self>, variables: NixValueWrapped) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables,
            parent: Some(self),
            backtrace: None,
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

    pub fn import_path(backtrace: Rc<NixBacktrace>, path: impl AsRef<Path>) -> NixResult {
        let path = path.as_ref();

        println!("Importing {path:#?}");

        let (scope, backtrace, result) = FileScope::from_path(path).evaluate(Some(backtrace))?;

        if path.file_name() == Some(OsStr::new("flake.nix")) {
            flake::resolve_flake(scope, backtrace, result)
        } else {
            Ok(result)
        }
    }

    /// The first Result is fair, the second is the VariableNotFound error
    pub fn resolve_attr_path<'a>(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        value: NixValueWrapped,
        attr_path: ast::Attrpath,
    ) -> NixResult<NixResult<NixVar>> {
        let mut attrs: Vec<_> = attr_path.attrs().collect();
        let last_attr = attrs.pop().unwrap();

        let attr_set = self.resolve_attr_set_path(backtrace.clone(), value, attrs.into_iter())?;

        let attr = self.resolve_attr(backtrace, &last_attr)?;

        let attr_set = attr_set.borrow();

        Ok(attr_set.get(&attr).unwrap().ok_or_else(|| {
            NixError::from_message(
                NixLabel::new(
                    NixSpan::from_ast_node(&self.file, &last_attr).into(),
                    NixLabelMessage::AttributeMissing,
                    NixLabelKind::Error,
                ),
                format!("Attribute '\x1b[1;95m{attr}\x1b[0m' missing"),
            )
        }))
    }

    pub fn resolve_attr_set_path<'a>(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        value: NixValueWrapped,
        mut attr_path: impl Iterator<Item = ast::Attr>,
    ) -> NixResult {
        if let Some(attr) = attr_path.next() {
            let attr = self.resolve_attr(backtrace.clone(), &attr)?;

            let set_value = value.borrow().get(&attr).unwrap();

            let Some(set_value) = set_value else {
                // If `value` doesn't have `attr`, then create it
                // as empty `AttrSet`
                let (last, _) = value
                    .borrow_mut()
                    .insert(attr, NixValue::AttrSet(HashMap::new()).wrap_var())
                    .unwrap();

                return self.resolve_attr_set_path(
                    backtrace.clone(),
                    last.resolve(backtrace)?,
                    attr_path,
                );
            };

            let set_value = set_value.resolve(backtrace.clone())?;

            if !set_value.borrow().is_attr_set() {
                todo!("Error handling for {:#}", set_value.borrow());
            };

            self.resolve_attr_set_path(backtrace, set_value, attr_path)
        } else {
            Ok(value)
        }
    }

    pub fn resolve_attr(
        self: &Rc<Self>,
        backtrace: Rc<NixBacktrace>,
        attr: &ast::Attr,
    ) -> NixResult<String> {
        match attr {
            ast::Attr::Ident(ident) => Ok(ident.ident_token().unwrap().text().to_owned()),
            ast::Attr::Dynamic(dynamic) => Ok(self
                .visit_expr(backtrace.clone(), dynamic.expr().unwrap())?
                .resolve(backtrace)?
                .borrow()
                .as_string()
                .expect("Cannot cast as string")),
            ast::Attr::Str(str) => self
                .visit_str(backtrace.clone(), str.clone())
                // visit_str always returns a string concrete
                .map(|v| v.as_concrete().unwrap().borrow().as_string().unwrap()),
        }
    }
}
