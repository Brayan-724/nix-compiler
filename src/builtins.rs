mod r#impl;

use std::fmt::{self, Write};
use std::path::PathBuf;
use std::rc::Rc;

use rnix::ast;

use crate::result::NixSpan;
use crate::value::{NixLambda, NixList};
use crate::{AsString, NixBacktrace, NixError, NixResult, NixValue, NixValueWrapped, Scope};

pub use r#impl::{get_builtins, Abort, Import, Map, RemoveAttrs, ToString};

pub trait FromNixExpr: Sized {
    fn from_nix_expr(
        backtrace: Rc<NixBacktrace>,
        scope: Rc<Scope>,
        expr: ast::Expr,
    ) -> NixResult<Self>;
}

impl FromNixExpr for NixValueWrapped {
    fn from_nix_expr(
        backtrace: Rc<NixBacktrace>,
        scope: Rc<Scope>,
        expr: ast::Expr,
    ) -> NixResult<Self> {
        scope.visit_expr(backtrace, expr)
    }
}

impl FromNixExpr for (Rc<Scope>, ast::Expr) {
    fn from_nix_expr(_: Rc<NixBacktrace>, scope: Rc<Scope>, expr: ast::Expr) -> NixResult<Self> {
        Ok((scope, expr))
    }
}

impl FromNixExpr for (Rc<NixBacktrace>, Rc<Scope>, ast::Expr) {
    fn from_nix_expr(
        backtrace: Rc<NixBacktrace>,
        scope: Rc<Scope>,
        expr: ast::Expr,
    ) -> NixResult<Self> {
        Ok((backtrace, scope, expr))
    }
}

macro_rules! int_from_nix_expr {
    ($($ty:ident),+) => { $(
        #[allow(unused_imports)]
        use std::primitive::$ty;

        impl FromNixExpr for $ty {
            fn from_nix_expr(backtrace: Rc<NixBacktrace>,scope: Rc<Scope>, expr: ast::Expr) -> NixResult<Self> {
                match *scope.visit_expr(backtrace, expr.clone())?.borrow() {
                    NixValue::Int(i) => Ok(i as $ty),
                    _ => Err(NixError::todo(
                        NixSpan::from_ast_node(&scope.file, &expr).into(),
                        concat!("Error handling: ", stringify!($ty)," cast"),
                        None
                    ))
                }
            }
        }
    )+ };
}

int_from_nix_expr! {isize, i64, i32, i16, i8}
int_from_nix_expr! {usize, u64, u32, u16, u8}

impl FromNixExpr for NixLambda {
    fn from_nix_expr(
        backtrace: Rc<NixBacktrace>,
        scope: Rc<Scope>,
        expr: ast::Expr,
    ) -> NixResult<Self> {
        let value = scope.visit_expr(backtrace, expr.clone())?;
        let Some(value) = value.borrow().as_lambda().cloned() else {
            return Err(NixError::todo(
                NixSpan::from_ast_node(&scope.file, &expr).into(),
                "Error handling: Lambda cast",
                None,
            ));
        };

        Ok(value)
    }
}

impl FromNixExpr for NixList {
    fn from_nix_expr(
        backtrace: Rc<NixBacktrace>,
        scope: Rc<Scope>,
        expr: ast::Expr,
    ) -> NixResult<Self> {
        let value = scope.visit_expr(backtrace, expr.clone())?;
        let Some(value) = value.borrow().as_list() else {
            return Err(NixError::todo(
                NixSpan::from_ast_node(&scope.file, &expr).into(),
                "Error handling: List cast",
                None,
            ));
        };

        Ok(value)
    }
}

impl FromNixExpr for PathBuf {
    fn from_nix_expr(
        backtrace: Rc<NixBacktrace>,
        scope: Rc<Scope>,
        expr: ast::Expr,
    ) -> NixResult<Self> {
        let value = scope.visit_expr(backtrace, expr.clone())?;
        let Some(value) = value.borrow().as_path() else {
            return Err(NixError::todo(
                NixSpan::from_ast_node(&scope.file, &expr).into(),
                "Error handling: Path cast",
                None,
            ));
        };

        Ok(value)
    }
}

impl FromNixExpr for String {
    fn from_nix_expr(
        backtrace: Rc<NixBacktrace>,
        scope: Rc<Scope>,
        expr: ast::Expr,
    ) -> NixResult<Self> {
        let value = scope.visit_expr(backtrace, expr.clone())?;
        let Some(value) = value.borrow().as_string() else {
            return Err(NixError::todo(
                NixSpan::from_ast_node(&scope.file, &expr).into(),
                "Error handling: String cast",
                None,
            ));
        };

        Ok(value)
    }
}

// TODO:
// impl FromNixExpr for NixAttrSet {
//     fn from_nix_expr(
//             backtrace: Rc<NixBacktrace>,
//             scope: Rc<Scope>,
//             expr: ast::Expr,
//         ) -> NixResult<Self> {
//         let value = scope.visit_expr(backtrace, expr.clone())?;
//         let Some(value) = value.borrow().as_attr_set() else {
//             todo!("Error handling");
//         };
//     }
// }

pub trait NixBuiltinInfo {
    const NAME: &str;
}

pub trait NixBuiltin {
    fn get_name(&self) -> &'static str;

    fn run(&self, backtrace: Rc<NixBacktrace>, scope: Rc<Scope>, argument: ast::Expr) -> NixResult;
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
