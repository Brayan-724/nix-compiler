pub mod builtins;
pub mod derivation;
mod expr;
pub mod flake;
mod result;
mod scope;
mod value;

pub use builtins::{NixBuiltin, NixBuiltinInfo};
pub use result::{
    NixBacktrace, NixBacktraceKind, NixError, NixLabel, NixLabelKind, NixLabelMessage, NixResult,
    NixSpan,
};
pub use scope::{FileScope, Scope};
use std::env;
pub use value::{LazyNixValue, NixAttrSet, NixLambdaParam, NixValue, NixValueWrapped, NixVar};

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

    // Profiling
    #[cfg(feature = "profiling")]
    tracing_subscriber::fmt()
        // .with_target(false)
        // .with_timer(tracing_subscriber::fmt::time::uptime())
        .without_time()
        .with_level(false)
        .init();

    // File evaluation
    let is_flake = !is_evaluation && arg.ends_with("flake.nix");

    let file = if is_evaluation {
        FileScope::repl_file(std::env::current_dir().unwrap(), arg)
    } else {
        FileScope::get_file(None, arg)
    };

    let (backtrace, result) = file.unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    let outputs = if is_flake {
        flake::resolve_flake(&backtrace, result).unwrap()
    } else {
        result
    };

    let outputs = LazyNixValue::Concrete(outputs)
        .wrap_var()
        .resolve_set(true, &backtrace)
        .unwrap_or_else(|err| {
            eprintln!("{err}");
            std::process::exit(1);
        });

    println!("Result (Expanded): {:#}", outputs.borrow());
    println!("Result (Minimized): {}", outputs.borrow());
}
