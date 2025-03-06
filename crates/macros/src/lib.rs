mod builtin;
mod gen_builtins;
mod params;
mod profile;

use proc_macro2::TokenStream;
use venial::Error;

fn err_syn_to_venial(e: syn::Error) -> venial::Error {
    venial::Error::new_at_span(e.span(), e)
}

macro_rules! setup_macro {
    (proc_macro; $name:ident => $struct:ty) => {
        #[proc_macro]
        pub fn $name(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
            <$struct as Macro<_>>::setup(input)
        }
    };

    (attribute; $name:ident => $struct:ty) => {
        #[proc_macro_attribute]
        pub fn $name(
            input: proc_macro::TokenStream,
            body: proc_macro::TokenStream,
        ) -> proc_macro::TokenStream {
            <$struct as Macro<_>>::setup((input, body))
        }
    };
}

setup_macro!(attribute ; builtin       => builtin::Builtin          );
setup_macro!(proc_macro; gen_builtins  => gen_builtins::GetBuiltins );

setup_macro!(attribute ; profile       => profile::Profile          );
setup_macro!(attribute ; profile_scope => profile::ProfileScope     );
setup_macro!(proc_macro; profile_start => profile::ProfileScopeStart);
setup_macro!(proc_macro; profile_end   => profile::ProfileScopeEnd  );

trait Macro<A> {
    type Item;

    fn parse(args: A) -> Result<Self::Item, Error>;
    fn expand(value: Self::Item) -> Result<TokenStream, Error>;

    fn setup(args: A) -> proc_macro::TokenStream {
        Self::parse(args)
            .and_then(Self::expand)
            .unwrap_or_else(|e| e.to_compile_error())
            .into()
    }
}

trait ProcMacro {
    type Item;

    fn parse(input: proc_macro::TokenStream) -> Result<Self::Item, Error>;

    fn expand(value: Self::Item) -> Result<TokenStream, Error>;
}

trait AttributeMacro {
    type Item;

    fn parse_attribute(
        input: proc_macro::TokenStream,
        body: proc_macro::TokenStream,
    ) -> Result<Self::Item, Error>;

    fn expand(value: Self::Item) -> Result<TokenStream, Error>;
}

impl<S: AttributeMacro> Macro<(proc_macro::TokenStream, proc_macro::TokenStream)> for S {
    type Item = S::Item;

    fn parse(
        (input, body): (proc_macro::TokenStream, proc_macro::TokenStream),
    ) -> Result<Self::Item, Error> {
        S::parse_attribute(input, body)
    }

    fn expand(value: Self::Item) -> Result<TokenStream, Error> {
        S::expand(value)
    }
}

impl<S: ProcMacro> Macro<proc_macro::TokenStream> for S {
    type Item = S::Item;

    fn parse(input: proc_macro::TokenStream) -> Result<Self::Item, Error> {
        S::parse(input)
    }

    fn expand(value: Self::Item) -> Result<TokenStream, Error> {
        S::expand(value)
    }
}
