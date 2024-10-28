mod r#impl;

use std::collections::HashMap;
use std::fmt::{self, Write};
use std::path::PathBuf;
use std::rc::Rc;

use rnix::ast;
use rowan::ast::AstNode;

use crate::{AsString, NixError, NixResult, NixValue, NixValueWrapped, Scope};

pub use r#impl::{Abort, Import, ToString, get_builtins};

pub trait FromNixExpr: Sized {
    fn from_nix_expr(scope: Rc<Scope>, expr: ast::Expr) -> NixResult<Self>;
}

impl FromNixExpr for NixValueWrapped {
    fn from_nix_expr(scope: Rc<Scope>, expr: ast::Expr) -> NixResult<Self> {
        scope.visit_expr(expr)
    }
}

impl FromNixExpr for (Rc<Scope>, ast::Expr) {
    fn from_nix_expr(scope: Rc<Scope>, expr: ast::Expr) -> NixResult<Self> {
        Ok((scope, expr))
    }
}

impl FromNixExpr for PathBuf {
    fn from_nix_expr(scope: Rc<Scope>, expr: ast::Expr) -> NixResult<Self> {
        let value = scope.visit_expr(expr.clone())?;
        let Some(value) = value.borrow().as_path() else {
            return Err(NixError::todo(
                &scope.file,
                expr.syntax().clone().into(),
                "Error handling: Path cast",
            ));
        };

        Ok(value)
    }
}

impl FromNixExpr for String {
    fn from_nix_expr(scope: Rc<Scope>, expr: ast::Expr) -> NixResult<Self> {
        let value = scope.visit_expr(expr.clone())?;
        let Some(value) = value.borrow().as_string() else {
            return Err(NixError::todo(
                &scope.file,
                expr.syntax().clone().into(),
                "Error handling: String cast",
            ));
        };

        Ok(value)
    }
}

pub trait NixBuiltinInfo {
    const NAME: &str;
}

pub trait NixBuiltin {
    fn get_name(&self) -> &'static str;

    fn run(&self, scope: Rc<Scope>, argument: ast::Expr) -> NixResult;
}

impl fmt::Debug for dyn NixBuiltin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('<')?;
        f.write_str(self.get_name())?;
        f.write_char('>')
    }
}

impl fmt::Display for dyn NixBuiltin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('<')?;
        f.write_str(self.get_name())?;
        f.write_char('>')
    }
}

impl PartialEq for dyn NixBuiltin {
    fn eq(&self, other: &Self) -> bool {
        self.get_name() == other.get_name()
    }
}

impl Eq for dyn NixBuiltin {}

// pub fn get_builtins() -> NixValue {
//     let mut builtins = HashMap::new();
//
//     builtins.insert(
//         "nixVersion".to_owned(),
//         NixValue::String(String::from("2.24.9")).wrap_var(),
//     );
//
//     macro_rules! gen_builtins {
//         ($builtins:ident; $($impl:ident),*) => {
//             $($builtins.insert($impl::NAME.to_owned(), r#impl::$impl::generate().wrap_var()));*
//         };
//     }
//
//     gen_builtins!(builtins; CompareVersions, Import);
//
//     NixValue::AttrSet(builtins)
// }
