pub mod builtins;
mod expr;
pub mod flake;
mod result;
mod scope;
mod value;

use std::env;
use std::rc::Rc;

pub use builtins::{NixBuiltin, NixBuiltinInfo};
pub use result::{
    NixBacktrace, NixError, NixLabel, NixLabelKind, NixLabelMessage, NixResult, NixSpan,
};
pub use scope::{FileScope, Scope};
pub use value::{
    AsAttrSet, AsString, LazyNixValue, NixLambdaParam, NixValue, NixValueWrapped, NixVar,
};

fn main() {
    let mut iter = env::args().skip(1).peekable();

    let is_evaluation = iter
        .peek()
        .is_some_and(|arg| arg == "-e" || arg == "--eval");

    if is_evaluation {
        iter.next();
    }

    let Some(arg) = iter.next() else {
        eprintln!("Usage: nix-compiler <file>");
        eprintln!("Usage: nix-compiler (--eval | -e) <expr>");
        return;
    };

    let is_flake = !is_evaluation && arg.ends_with("flake.nix");

    let file = if is_evaluation {
        Rc::new(FileScope {
            path: std::env::current_dir().unwrap(),
            content: arg,
        })
    } else {
        FileScope::from_path(&arg)
    };

    let (backtrace, result) = file.evaluate(None).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    let outputs = if is_flake {
        flake::resolve_flake(backtrace.clone(), result).unwrap()
    } else {
        result
    };

    let outputs = LazyNixValue::Concrete(outputs)
        .wrap_var()
        .resolve_set(true, backtrace)
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });

    println!("Result (Expanded): {:#}", outputs.borrow());
    println!("Result (Minimized): {}", outputs.borrow());
}
