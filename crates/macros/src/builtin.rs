use convert_case::{Case, Casing};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use venial::{Error, Function};

use crate::err_syn_to_venial;
use crate::params::NixBuiltinParams;

const REFLECTION_BUILTIN: &str = "__rust_reflection__nix-macros__builtins";

pub fn get_builtins() -> Result<String, Error> {
    std::env::var(REFLECTION_BUILTIN).map_err(|_| Error::new("Set at least one builtin"))
}

fn append_builtin(name: String) {
    if let Ok(old) = std::env::var(REFLECTION_BUILTIN) {
        std::env::set_var(REFLECTION_BUILTIN, format!("{old};{name}"))
    } else {
        std::env::set_var(REFLECTION_BUILTIN, name)
    }
}

pub struct Builtin {
    func: Function,
    params: NixBuiltinParams,
    nix_ident: String,
    struct_name: Ident,
}

impl Builtin {
    pub fn new(func: Function, renamed: Option<String>) -> Result<Self, Error> {
        let func_name = func.name.to_string();
        let struct_name = func_name
            .strip_prefix("r#")
            .unwrap_or(&func_name)
            .to_case(Case::Pascal);

        let struct_name = format_ident!("{struct_name}", span = func.name.span());

        append_builtin(struct_name.to_string());

        let params = NixBuiltinParams::new(&struct_name, &func.params)?;

        let nix_ident = renamed.unwrap_or_else(|| struct_name.to_string().to_case(Case::Camel));

        Ok(Self {
            func,
            params,
            nix_ident,
            struct_name,
        })
    }

    fn generate_builtin(&self) -> TokenStream {
        let nix_ident = &self.nix_ident;
        let struct_name = &self.struct_name;
        let params_decl = &self.params.decl;
        let params_def = &self.params.def;
        let params_list = self.params.param_list();

        quote_spanned! { self.func.tk_params_parens.span =>
            impl crate::builtins::NixBuiltin for #struct_name {
                fn get_name(&self) -> &'static str {
                    #nix_ident
                }

                fn run(
                    &self,
                    _backtrace: &crate::NixBacktrace,
                    _argument: crate::NixVar
                ) -> crate::result::NixResult {
                    let Self(#(#params_list),*) = &self;
                    #(#params_decl)*
                    #struct_name::run(#(#params_def),*)
                }
            }
        }
    }

    fn generate_declaration(&self) -> TokenStream {
        let struct_name = &self.struct_name;
        let struct_decl = self.params.struct_decl();

        quote_spanned! { self.func.span() =>
            #[derive(Clone)]
            pub struct #struct_name(#(#struct_decl),*);
        }
    }

    fn generate_impl(&self) -> Result<TokenStream, Error> {
        let struct_name = &self.struct_name;
        let struct_def = self.params.struct_def();

        let func_body = self
            .func
            .body
            .as_ref()
            .ok_or_else(|| Error::new_at_span(self.func.span(), "Function should have body"))?;
        let func_body = quote_spanned! {func_body.span() => #func_body};

        let func_params = &self.func.params;
        let func_params = quote_spanned! {self.func.tk_params_parens.span => #func_params};

        Ok(quote_spanned! { self.func.span() =>
            impl #struct_name {
                pub fn generate() -> crate::value::NixValue {
                    crate::value::NixValue::Lambda(crate::value::NixLambda::Builtin(::std::rc::Rc::new(Box::new(#struct_name(#(#struct_def),*)))))
                }

                fn run(#func_params) -> crate::result::NixResult {
                    #func_body
                }
            }
        })
    }

    fn generate_info(&self) -> TokenStream {
        let nix_ident = &self.nix_ident;
        let struct_name = &self.struct_name;

        quote_spanned! { self.struct_name.span() =>
            impl crate::builtins::NixBuiltinInfo for #struct_name {
                const NAME: &str = #nix_ident;
            }
        }
    }

    pub fn generate(self) -> Result<TokenStream, Error> {
        let decl = self.generate_declaration();
        let def_impl = self.generate_impl()?;
        let builtin = self.generate_builtin();
        let builtin_info = self.generate_info();

        Ok(quote! {
            #decl

            #def_impl

            #builtin

            #builtin_info
        })
    }
}

impl super::AttributeMacro for Builtin {
    type Item = (Function, Option<String>);

    fn parse_attribute(
        input: proc_macro::TokenStream,
        body: proc_macro::TokenStream,
    ) -> Result<Self::Item, Error> {
        let venial::Item::Function(item) = venial::parse_item(body.into())? else {
            return Err(Error::new("A function is expected in #[builtin]"));
        };

        let renamed = syn::parse::<Option<syn::LitStr>>(input)
            .map_err(err_syn_to_venial)?
            .map(|v| v.value());

        Ok((item, renamed))
    }

    fn expand((func, renamed): Self::Item) -> Result<TokenStream, Error> {
        Builtin::new(func, renamed).and_then(Builtin::generate)
    }
}
