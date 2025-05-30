mod file;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::path::Path;
use std::rc::Rc;

use rnix::ast;

pub use file::FileScope;

use crate::result::{NixLabel, NixLabelKind, NixLabelMessage, NixSpan};
use crate::value::attrset::AttrsetBuilder;
use crate::{builtins, flake, NixAttrSetDynamic, NixBacktrace, NixResult, NixValue, NixVar};

#[derive(Debug)]
pub struct Scope {
    pub backtrace: Option<NixBacktrace>,
    pub file: Rc<FileScope>,
    pub variables: Rc<RefCell<AttrsetBuilder>>,
    pub parent: Option<Rc<Scope>>,
}

impl Scope {
    #[nix_macros::profile]
    pub fn new_with_builtins(file_scope: Rc<FileScope>) -> Rc<Self> {
        macro_rules! insert {
            ($ident:ident; $key:ident = $value:expr) => {
                $ident.insert(stringify!($key).to_owned(), $value.wrap_var())
            };
        }

        let mut globals = NixAttrSetDynamic::new();
        let builtins = builtins::get_builtins();

        insert!(globals; builtins = builtins);
        insert!(globals; abort = builtins::Abort::generate());
        insert!(globals; baseNameOf = builtins::BaseNameOf::generate());
        insert!(globals; derivation = builtins::DerivationImpl::generate());
        insert!(globals; false = NixValue::Bool(false));
        insert!(globals; import = builtins::Import::generate());
        insert!(globals; map = builtins::Map::generate());
        insert!(globals; null = NixValue::Null);
        insert!(globals; removeAttrs = builtins::RemoveAttrs::generate());
        insert!(globals; toString = builtins::ToString::generate());
        insert!(globals; throw = builtins::Throw::generate());
        insert!(globals; true = NixValue::Bool(true));

        let parent = Rc::new(Scope {
            file: file_scope.clone(),
            variables: AttrsetBuilder::from(globals).wrap_mut(),
            parent: None,
            backtrace: None,
        });

        Rc::new(Self {
            file: file_scope,
            variables: AttrsetBuilder::new().wrap_mut(),
            parent: Some(parent),
            backtrace: None,
        })
    }

    #[nix_macros::profile]
    pub fn new_child(self: Rc<Self>) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables: AttrsetBuilder::new().wrap_mut(),
            parent: Some(self),
            backtrace: None,
        })
    }

    #[nix_macros::profile]
    pub fn new_child_from(self: Rc<Self>, variables: Rc<RefCell<AttrsetBuilder>>) -> Rc<Scope> {
        Rc::new(Scope {
            file: self.file.clone(),
            variables,
            parent: Some(self),
            backtrace: None,
        })
    }

    #[nix_macros::profile]
    pub fn get_variable(self: &Rc<Self>, varname: String) -> Option<NixVar> {
        self.variables
            .borrow_mut()
            .cached()
            .get(&varname)
            .cloned()
            .or_else(|| {
                self.parent
                    .as_ref()
                    .and_then(|parent| parent.get_variable(varname))
            })
    }

    pub fn import_path(backtrace: &NixBacktrace, path: impl AsRef<Path>) -> NixResult {
        let path = path.as_ref();

        println!("Importing {path:#?}");

        let (backtrace, result) = FileScope::get_file(Some(backtrace.clone()), path)?;

        if path.file_name() == Some(OsStr::new("flake.nix")) {
            flake::resolve_flake(&backtrace, result)
        } else {
            Ok(result)
        }
    }

    /// The first Result is fair, the second is the VariableNotFound error
    #[nix_macros::profile]
    pub fn resolve_attr_path(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        value: NixVar,
        mut attr_path: impl Iterator<Item = ast::Attr>,
    ) -> NixResult<NixResult<NixVar>> {
        if let Some(attr_node) = attr_path.next() {
            let attr = self.resolve_attr(backtrace, &attr_node)?;

            let value = value.resolve(backtrace)?;
            let set_value = match value.borrow().get(backtrace, &attr) {
                Ok(v) => v,
                Err(e) => return Ok(Err(e)),
            };

            let Some(set_value) = set_value else {
                return Ok(Err(backtrace.to_labeled_error(
                    vec![NixLabel::new(
                        NixSpan::from_ast_node(&self.file, &attr_node).into(),
                        NixLabelMessage::AttributeMissing,
                        NixLabelKind::Error,
                    )],
                    format!("Attribute '\x1b[1;95m{attr}\x1b[0m' missing"),
                )));
            };

            self.resolve_attr_path(backtrace, set_value, attr_path)
        } else {
            Ok(Ok(value))
        }
    }

    #[nix_macros::profile]
    pub fn resolve_attr(
        self: &Rc<Self>,
        backtrace: &NixBacktrace,
        attr: &ast::Attr,
    ) -> NixResult<String> {
        match attr {
            ast::Attr::Ident(ident) => Ok(ident.ident_token().unwrap().text().to_owned()),
            ast::Attr::Dynamic(dynamic) => Ok(self
                .visit_expr(backtrace, dynamic.expr().unwrap())?
                .resolve(backtrace)?
                .borrow()
                .cast_to_string()
                .expect("Cannot cast as string")),
            ast::Attr::Str(str) => self
                .visit_str(backtrace, str.clone())
                // visit_str always returns a string concrete
                .map(|v| v.as_concrete().unwrap().borrow().cast_to_string().unwrap()),
        }
    }
}
