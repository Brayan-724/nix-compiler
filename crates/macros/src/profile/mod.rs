mod func;
pub use func::Profile;

mod scope;
pub use scope::ProfileScope;

use crate::ProcMacro;

pub struct ProfileScopeStart;
pub struct ProfileScopeEnd;

impl ProcMacro for ProfileScopeStart {
    type Item = ();

    fn parse(_: proc_macro::TokenStream) -> Result<(), venial::Error> {
        Ok(())
    }

    #[cfg(feature = "profiling")]
    fn expand(_: ()) -> Result<proc_macro2::TokenStream, venial::Error> {
        Ok(quote::quote!(
            let _profile_start = ::std::time::SystemTime::now();
        ))
    }

    #[cfg(not(feature = "profiling"))]
    fn expand(_: ()) -> Result<proc_macro2::TokenStream, venial::Error> {
        Ok(quote::quote!())
    }
}

impl ProcMacro for ProfileScopeEnd {
    type Item = String;

    fn parse(input: proc_macro::TokenStream) -> Result<String, venial::Error> {
        let name = syn::parse::<syn::LitStr>(input).map_err(|err| venial::Error::new(err))?;
        let name = name.value();
        Ok(name)
    }

    #[cfg(feature = "profiling")]
    fn expand(name: String) -> Result<proc_macro2::TokenStream, venial::Error> {
        let exit = "exit in {:?}";
        Ok(quote::quote!(
            ::tracing::warn!(target: #name, #exit, duration = _profile_start.elapsed());
        ))
    }

    #[cfg(not(feature = "profiling"))]
    fn expand(_: String) -> Result<proc_macro2::TokenStream, venial::Error> {
        Ok(quote::quote!())
    }
}
