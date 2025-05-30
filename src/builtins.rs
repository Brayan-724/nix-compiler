pub mod hash;
mod r#impl;

use std::fmt::{self, Write};
use std::path::PathBuf;

use crate::value::{NixAttrSet, NixLambda, NixList};
use crate::{NixBacktrace, NixError, NixResult, NixValue, NixValueWrapped, NixVar};

pub use r#impl::{
    get_builtins, Abort, BaseNameOf, DerivationImpl, Import, Map, RemoveAttrs, Throw, ToString,
};

pub trait FromNixExpr: Sized {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self>;
}

impl FromNixExpr for NixValueWrapped {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)
    }
}

impl FromNixExpr for NixVar {
    fn from_nix_expr(_: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        Ok(var)
    }
}

impl FromNixExpr for (NixBacktrace, NixVar) {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        Ok((backtrace.clone(), var))
    }
}

macro_rules! int_from_nix_expr {
    ($($ty:ident),+) => { $(
        #[allow(unused_imports)]
        use std::primitive::$ty;

        impl FromNixExpr for $ty {
            fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
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
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?.borrow().cast_lambda(backtrace)
    }
}

impl FromNixExpr for NixList {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_list()
            .ok_or_else(|| todo!("Error handling: List cast"))
    }
}

impl FromNixExpr for PathBuf {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_path()
            .ok_or_else(|| todo!("Error handling: Path cast"))
    }
}

impl FromNixExpr for String {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .cast_to_string()
            .ok_or_else(|| todo!("Error handling: String cast"))
    }
}

impl FromNixExpr for bool {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_bool()
            .ok_or_else(|| NixError::todo(backtrace.0.clone(), "Bool cast", backtrace.1.clone()))
    }
}

impl FromNixExpr for NixAttrSet {
    fn from_nix_expr(backtrace: &NixBacktrace, var: NixVar) -> NixResult<Self> {
        var.resolve(backtrace)?
            .borrow()
            .as_attr_set()
            .cloned()
            .ok_or_else(|| todo!("Error handling: Attrset cast"))
    }
}

pub trait NixBuiltinInfo {
    const NAME: &str;
}

pub trait NixBuiltin {
    fn get_name(&self) -> &'static str;

    fn run(&self, backtrace: &NixBacktrace, argument: NixVar) -> NixResult;
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
