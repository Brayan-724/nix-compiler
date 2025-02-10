mod builtin;
mod gen_builtins;
mod params;
mod profile;

use proc_macro2::TokenStream;
use quote::quote;
use venial::Error;

macro_rules! setup_macro {
    (proc_macro; $name:ident => $struct:ty) => {
        #[proc_macro]
        pub fn $name(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
            <$struct as Macro<_, _>>::setup(input)
        }
    };

    (attribute; $name:ident => $struct:ty) => {
        #[proc_macro_attribute]
        pub fn $name(
            input: proc_macro::TokenStream,
            body: proc_macro::TokenStream,
        ) -> proc_macro::TokenStream {
            <$struct as Macro<_, _>>::setup((input, body))
        }
    };
}

setup_macro!(attribute ; builtin       => builtin::Builtin         );
setup_macro!(proc_macro; gen_builtins  => gen_builtins::GetBuiltins);

setup_macro!(attribute ; profile       => profile::Profile         );
setup_macro!(attribute ; profile_scope => profile::ProfileScope    );

trait Macro<A, R> {
    fn parse(args: A) -> Result<R, Error>;
    fn expand(value: R) -> Result<TokenStream, Error>;

    fn setup(args: A) -> proc_macro::TokenStream {
        Self::parse(args)
            .and_then(Self::expand)
            .unwrap_or_else(|e| e.to_compile_error())
            .into()
    }
}

trait ProcMacro<R> {
    fn parse(input: proc_macro::TokenStream) -> Result<R, Error>;

    fn expand(value: R) -> Result<TokenStream, Error>;
}

trait AttributeMacro<R> {
    fn parse_attribute(
        input: proc_macro::TokenStream,
        body: proc_macro::TokenStream,
    ) -> Result<R, Error>;

    fn expand(value: R) -> Result<TokenStream, Error>;
}

impl<R, S: AttributeMacro<R>> Macro<(proc_macro::TokenStream, proc_macro::TokenStream), R> for S {
    fn parse(
        (input, body): (proc_macro::TokenStream, proc_macro::TokenStream),
    ) -> Result<R, Error> {
        S::parse_attribute(input, body)
    }

    fn expand(value: R) -> Result<TokenStream, Error> {
        S::expand(value)
    }
}

impl<R, S: ProcMacro<R>> Macro<proc_macro::TokenStream, R> for S {
    fn parse(input: proc_macro::TokenStream) -> Result<R, Error> {
        S::parse(input)
    }

    fn expand(value: R) -> Result<TokenStream, Error> {
        S::expand(value)
    }
}
