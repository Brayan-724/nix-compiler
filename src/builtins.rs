mod hash;
mod r#impl;

use std::fmt::{self, Write};
use std::path::PathBuf;
use std::rc::Rc;

use crate::value::{NixLambda, NixList};
use crate::{NixBacktrace, NixResult, NixValue, NixValueWrapped, NixVar};

pub use r#impl::{get_builtins, Abort, Import, Map, RemoveAttrs, Throw, ToString};

pub trait FromNixExpr: Sized {
    fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self>;
}

impl FromNixExpr for NixValueWrapped {
    fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)
    }
}

impl FromNixExpr for NixVar {
    fn from_nix_expr(_: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
        Ok(var)
    }
}

impl FromNixExpr for (Rc<NixBacktrace>, NixVar) {
    fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
        Ok((backtrace, var))
    }
}

macro_rules! int_from_nix_expr {
    ($($ty:ident),+) => { $(
        #[allow(unused_imports)]
        use std::primitive::$ty;

        impl FromNixExpr for $ty {
            fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
                match *var.resolve(backtrace)?.borrow() {
                    NixValue::Int(i) => Ok(i as $ty),
                    _ => todo!(concat!("Error handling: ", stringify!($ty)," cast"))
                }
            }
        }
    )+ };
}

int_from_nix_expr! {isize, i64, i32, i16, i8}
int_from_nix_expr! {usize, u64, u32, u16, u8}

impl FromNixExpr for NixLambda {
    fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_lambda()
            .cloned()
            .ok_or_else(|| todo!("Error handling: Lambda cast"))
    }
}

impl FromNixExpr for NixList {
    fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_list()
            .ok_or_else(|| todo!("Error handling: List cast"))
    }
}

impl FromNixExpr for PathBuf {
    fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_path()
            .ok_or_else(|| todo!("Error handling: Path cast"))
    }
}

impl FromNixExpr for String {
    fn from_nix_expr(backtrace: Rc<NixBacktrace>, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_string()
            .ok_or_else(|| todo!("Error handling: String cast"))
    }
}

// TODO:
// impl FromNixExpr for NixAttrSet {
//     fn from_nix_expr(
//             backtrace: Rc<NixBacktrace>,
//             scope: Rc<Scope>,
//             var: NixVar,
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

    fn run(&self, backtrace: Rc<NixBacktrace>, argument: NixVar) -> NixResult;
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
