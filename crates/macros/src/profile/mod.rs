mod func;
pub use func::Profile;

mod scope;
pub use scope::ProfileScope;

use crate::ProcMacro;

pub struct ProfileScopeStart;
pub struct ProfileScopeEnd;

impl ProcMacro<()> for ProfileScopeStart {
    fn parse(_: proc_macro::TokenStream) -> Result<(), venial::Error> {
        Ok(())
    }

    fn expand(_: ()) -> Result<proc_macro2::TokenStream, venial::Error> {
        Ok(quote::quote!(
            let _profile_start = ::std::time::SystemTime::now();
        ))
    }
}

impl ProcMacro<String> for ProfileScopeEnd {
    fn parse(input: proc_macro::TokenStream) -> Result<String, venial::Error> {
        let name = syn::parse::<syn::LitStr>(input).map_err(|err| venial::Error::new(err))?;
        let name = name.value();
        Ok(name)
    }

    fn expand(name: String) -> Result<proc_macro2::TokenStream, venial::Error> {
        let exit = "exit in {:?}";
        Ok(quote::quote!(
            ::tracing::warn!(target: #name, #exit, duration = _profile_start.elapsed());
        ))
    }
}
