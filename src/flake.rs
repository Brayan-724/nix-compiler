use crate::result::NixBacktrace;
use crate::value::NixAttrSet;
use crate::{
    LazyNixValue, NixAttrSetDynamic, NixError, NixResult, NixValue, NixValueWrapped, Scope,
};

pub fn resolve_flake(backtrace: &NixBacktrace, result: NixValueWrapped) -> NixResult {
    let result = result.borrow();

    let Some(flake) = result.as_attr_set() else {
        todo!("Error handling");
    };

    let inputs = flake
        .get("inputs")
        .map(|inputs| inputs.resolve(backtrace))
        .unwrap_or_else(|| {
            Ok(NixValue::AttrSet(NixAttrSet::Dynamic(NixAttrSetDynamic::new().into())).wrap())
        })?;

    let inputs = inputs.borrow();

    let Some(inputs) = inputs.as_attr_set() else {
        todo!("inputs should be attr set");
    };

    let inputs = inputs.iter().map::<NixResult<_>, _>(|(key, var)| {
        let var = var.resolve(backtrace)?;
        let var = var.borrow();

        let Some(var) = var.as_attr_set() else {
            todo!("input should be attr set")
        };

        let path = var
            .get("path")
            .expect("TODO: Cloning repos")
            .resolve(backtrace)?
            .borrow()
            .as_path()
            .unwrap_or_else(|| todo!("Eror handling"));

        let flake_path = path.join("flake.nix");

        let flake = Scope::import_path(backtrace, flake_path)?;

        let mut out = NixAttrSetDynamic::new();

        out.insert(
            "_type".to_owned(),
            NixValue::String("flake".to_owned()).wrap_var(),
        );
        out.insert("outPath".to_owned(), NixValue::Path(path).wrap_var());

        out.insert(
            "outputs".to_owned(),
            LazyNixValue::Concrete(flake).wrap_var(),
        );

        Ok((
            key.clone(),
            NixValue::AttrSet(NixAttrSet::Dynamic(out.into())).wrap_var(),
        ))
    });

    nix_macros::profile_start!();

    let Some(outputs_var) = flake.get("outputs") else {
        return Err(NixError::todo(
            backtrace.0.clone(),
            "Meh",
            backtrace.1.clone(),
        ));
    };

    let outputs = outputs_var.resolve(backtrace)?;
    let outputs = outputs.borrow();

    let Some(lambda) = outputs.as_lambda() else {
        todo!("outputs should be a lambda")
    };

    let mut value = NixAttrSetDynamic::new();

    macro_rules! insert {
        ($key:ident = $value:expr) => {
            value.insert($key, $value)
        };
    }

    value.insert("self".to_owned(), outputs_var.clone());

    for input in inputs {
        let (input, input_path) = input?;

        insert!(input = input_path);
    }

    nix_macros::profile_end!("resolve_flake_outputs");

    lambda
        .call(
            backtrace,
            NixValue::AttrSet(NixAttrSet::Dynamic(value.into())).wrap_var(),
        )?
        .resolve(backtrace)
}
