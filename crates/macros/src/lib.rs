use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use venial::{parse_item, Error, Function, Item};

#[proc_macro_attribute]
pub fn builtin(
    _: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let result = match parse_item(body.into()) {
        Err(e) => Err(e),
        Ok(Item::Function(func)) => parse_builtin(func),
        Ok(_) => Err(Error::new("")),
    };

    result.unwrap_or_else(|e| e.to_compile_error()).into()
}

fn parse_builtin(func: Function) -> Result<TokenStream, Error> {
    let nix_ident = func.name.to_string().to_case(Case::Camel);
    let nix_ident = format_ident!("{nix_ident}", span = func.name.span());

    let struct_name = func.name.to_string().to_case(Case::Pascal);
    let struct_name = format_ident!("{struct_name}", span = func.name.span());

    {
        if let Ok(old) = std::env::var("__rust_reflection__nix-macros__builtins") {
            std::env::set_var(
                "__rust_reflection__nix-macros__builtins",
                format!("{old};{}", struct_name.to_string()),
            )
        } else {
            std::env::set_var(
                "__rust_reflection__nix-macros__builtins",
                struct_name.to_string(),
            )
        }
    }

    let func_body = &func.body.expect("Function should have body");
    let func_body = quote_spanned! {func_body.span() => #func_body};

    let func_params = &func.params;

    let total_params = func.params.len();

    let (mut param_list, (params, struct_)) = func
        .params
        .items()
        .filter_map(|param| match param {
            venial::FnParam::Receiver(receiver) => {
                Some(Err(Error::new_at_tokens(receiver, "self is not permitted")))
            }
            venial::FnParam::Typed(venial::FnTypedParam {
                name,
                ty: venial::TypeExpr { tokens },
                ..
            }) => {
                Some(Ok((name, tokens.first()?)))
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .enumerate()
        .map(|(idx, (param, ty))| {
            let param_ident = format_ident!("__param_{idx}", span = param.span());

            let struct_decl = quote_spanned! {param.span() => ::std::rc::Rc<(::std::rc::Rc<Scope>, ::rnix::ast::Expr)>};
            let struct_def = quote_spanned! {param.span() => None};


            let (param_def, param_decl) = if idx == total_params - 1 {
                let param_def = quote_spanned! {param.span() => #param};
                let param_decl = quote_spanned! {ty.span() =>
                    let #param = <#ty as crate::builtins::FromNixExpr>::from_nix_expr(scope, argument)?;
                };

                (param_def, param_decl)
            } else {
                let old_params = (0..idx).map(|i| format_ident!("__param_{i}", span = param.span())).collect::<Vec<_>>();
                // let new_param = quote_spanned! {ty.span() => <#ty as crate::builtins::FromNixExpr>::from_nix_expr(scope, argument)};
                let new_param = quote_spanned! {ty.span() => Some(::std::rc::Rc::new((scope, argument)))};

                let param_def = quote_spanned! {param.span() => <#ty as crate::builtins::FromNixExpr>::from_nix_expr(#param.0.clone(), #param.1.clone())?};
                let param_decl = quote_spanned! {ty.span() =>
                    let Some(#param) = #param_ident else {
                        return Ok(NixValue::Builtin(::std::rc::Rc::new(Box::new(#struct_name(#(#old_params,)* #new_param)))).wrap())
                    };
                };

                (param_def, param_decl)
            };


            (param_ident, ((param_def, param_decl), (struct_def, struct_decl)))
        })
        .unzip::<_, _, Vec<_>, ((Vec<_>, Vec<_>), (Vec<_>, Vec<_>))>();

    let (params_def, params_decl) = params;
    let (mut struct_def, mut struct_decl) = struct_;
    param_list.pop();
    struct_def.pop();
    struct_decl.pop();

    let def = func
        .vis_marker
        .map(|_| {
            quote! {
                #[derive(Clone)]
                pub struct #struct_name(#(Option<#struct_decl>)*);
            }
        })
        .unwrap_or_else(|| {
            quote! {
                #[derive(Clone)]
                struct #struct_name(#(Option<#struct_decl>)*);
            }
        });

    let def_body = quote_spanned! { func.tk_params_parens.span =>
        let Self(#(#param_list),*) = &self;
        #(#params_decl)*
        #struct_name::run(#(#params_def),*)
    };

    Ok(quote! {
        #def

        impl #struct_name {
            pub fn generate() -> crate::value::NixValue {
                crate::value::NixValue::Builtin(::std::rc::Rc::new(Box::new(#struct_name(#(#struct_def),*))))
            }

            fn run(#func_params) -> crate::result::NixResult {
                #func_body
            }
        }

        impl crate::builtins::NixBuiltinInfo for #struct_name {
            const NAME: &str = stringify!(#nix_ident);
        }

        impl crate::builtins::NixBuiltin for #struct_name {
            fn get_name(&self) -> &'static str {
                stringify!(#nix_ident)
            }

            fn run(&self, scope: ::std::rc::Rc<crate::scope::Scope>, argument: ::rnix::ast::Expr) -> crate::result::NixResult {
                #def_body
            }
        }
    })
}

#[proc_macro]
pub fn gen_builtins(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Ok(builtins) = std::env::var("__rust_reflection__nix-macros__builtins") else {
        return Error::new("Set at least one builtin")
            .to_compile_error()
            .into();
    };

    let builtins = builtins
        .split(";")
        .map(|builtin| {
            let builtin = format_ident!("{builtin}");

            quote! { builtins.insert(#builtin::NAME.to_owned(), #builtin::generate().wrap_var()) }
        })
        .collect::<Vec<_>>();

    let nix_version_key = "nixVersion";
    let nix_version = "2.24.9";

    quote! {
        pub fn get_builtins() -> NixValue {
            use super::*;

            let mut builtins = HashMap::new();

            builtins.insert(
                #nix_version_key.to_owned(),
                NixValue::String(String::from(#nix_version)).wrap_var(),
            );

            #(#builtins;)*

            NixValue::AttrSet(builtins)
        }
    }
    .into()
}
