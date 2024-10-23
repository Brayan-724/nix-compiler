use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use rnix::ast;

use crate::builtins::NixValueBuiltin;
use crate::value::{AsAttrSet, AsString, NixValue, NixVar};

#[derive(Debug, PartialEq, Eq)]
pub struct FileScope {
    pub path: PathBuf,
}

impl FileScope {
    pub fn from_path(path: impl AsRef<Path>) -> Rc<Self> {
        Rc::new(FileScope {
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn evaluate(self: Rc<Self>) -> Result<NixVar, ()> {
        let content = fs::read_to_string(&self.path).unwrap();

        let parse = rnix::Root::parse(&content);

        for error in parse.errors() {
            println!("\x1b[31merror: {}\x1b[0m", error);
        }

        if !parse.errors().is_empty() {
            return Err(());
        }

        let root = parse.tree();

        let scope = Scope::new_with_builtins(self);

        Ok(scope.visit_root(root))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Scope {
    pub file: Rc<FileScope>,
    pub variables: NixVar,
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
        let mut builtins = HashMap::new();

        insert!(globals; abort = NixValue::Builtin(NixValueBuiltin::Abort));
        insert!(builtins; abort = NixValue::Builtin(NixValueBuiltin::Abort));

        insert!(builtins; compareVersions = NixValue::Builtin(NixValueBuiltin::CompareVersions(None)));

        insert!(globals; import = NixValue::Builtin(NixValueBuiltin::Import));
        insert!(builtins; import = NixValue::Builtin(NixValueBuiltin::Import));

        insert!(builtins; nixVersion = NixValue::String(String::from("2.24.9")));

        insert!(globals; toString = NixValue::Builtin(NixValueBuiltin::ToString));
        insert!(builtins; toString = NixValue::Builtin(NixValueBuiltin::ToString));

        insert!(globals; builtins = NixValue::AttrSet(builtins));

        let parent = Rc::new(Scope {
            file: file_scope.clone(),
            variables: NixValue::AttrSet(globals).wrap_var(),
            parent: None,
        });

        Rc::new(Self {
            file: file_scope,
            variables: NixValue::AttrSet(HashMap::new()).wrap_var(),
            parent: Some(parent),
        })
    }

    pub fn new_child(self: Rc<Self>) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables: NixValue::AttrSet(HashMap::new()).wrap_var(),
            parent: Some(self),
        })
    }

    pub fn new_child_from(self: Rc<Self>, variables: NixVar) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables,
            parent: Some(self),
        })
    }

    pub fn set_variable(self: &Rc<Self>, varname: String, value: NixVar) -> Option<NixVar> {
        self.variables
            .as_concrete()
            .unwrap()
            .borrow_mut()
            .as_attr_set_mut()
            .unwrap()
            .insert(varname, value)
    }

    pub fn get_variable(self: &Rc<Self>, varname: String) -> Option<NixVar> {
        self.variables
            .as_concrete()
            .unwrap()
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
        value: NixVar,
        attr_path: impl Iterator<Item = ast::Attr>,
    ) -> Option<NixVar> {
        let mut attr_path: Vec<_> = attr_path.collect();
        let last_attr = attr_path.pop().unwrap();

        let attr_set = self.resolve_attr_set_path(value, attr_path.into_iter());

        let last_attr = self.resolve_attr(last_attr);

        let attr_set = attr_set.resolve();
        let attr_set = attr_set.borrow();

        attr_set.get(&last_attr).unwrap().or_else(|| {
            println!("Cannot get {last_attr}");
            None
        })
    }

    pub fn resolve_attr_set_path<'a>(
        self: &Rc<Self>,
        value: NixVar,
        mut attr_path: impl Iterator<Item = ast::Attr>,
    ) -> NixVar {
        if let Some(attr) = attr_path.next() {
            let attr = self.resolve_attr(attr);

            let value = value.resolve();
            let set_value = value.borrow().get(&attr).unwrap();

            let Some(set_value) = set_value else {
                let (last, _) = value
                    .borrow_mut()
                    .insert(attr, NixValue::AttrSet(HashMap::new()).wrap_var())
                    .unwrap();

                return self.resolve_attr_set_path(last, attr_path);
            };

            if !set_value.resolve_and(AsAttrSet::is_attr_set) {
                set_value.resolve_map(|set_value| todo!("Error handling for {set_value:#}"));
            };

            self.resolve_attr_set_path(set_value, attr_path)
        } else {
            value
        }
    }

    pub fn resolve_attr(self: &Rc<Self>, attr: ast::Attr) -> String {
        match attr {
            ast::Attr::Ident(ident) => ident.ident_token().unwrap().text().to_owned(),
            ast::Attr::Dynamic(dynamic) => self
                .visit_expr(dynamic.expr().unwrap())
                .resolve_map(AsString::as_string)
                .expect("Cannot cast as string"),
            ast::Attr::Str(_) => todo!(),
        }
    }
}
