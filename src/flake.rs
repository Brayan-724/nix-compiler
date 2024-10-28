use std::collections::HashMap;

use crate::{AsAttrSet, LazyNixValue, NixResult, NixValue, NixValueWrapped, Scope};

pub fn resolve_flake(result: NixValueWrapped) -> NixResult {
    let result = result.borrow();

    let Some(flake) = result.as_attr_set() else {
        todo!("Error handling");
    };

    let inputs = flake
        .get("inputs")
        .cloned()
        .unwrap_or_else(|| NixValue::AttrSet(HashMap::new()).wrap_var());

    let inputs = inputs.resolve()?;
    let inputs = inputs.borrow();

    let Some(inputs) = inputs.as_attr_set() else {
        todo!("inputs should be attr set");
    };

    let inputs = inputs.iter().map::<NixResult<_>, _>(|(key, var)| {
        let var = var.resolve()?;
        let var = var.borrow();

        let Some(var) = var.as_attr_set() else {
            todo!("input should be attr set")
        };

        let path = var
            .get("path")
            .expect("TODO: Cloning repos")
            .resolve_map(|value| {
                if let NixValue::Path(path) = value {
                    path.clone()
                } else {
                    todo!("Eror handling")
                }
            })?;

        let flake_path = path.join("flake.nix");

        let flake = Scope::import_path(flake_path)?;

        let mut out = HashMap::new();

        out.insert(
            "_type".to_owned(),
            NixValue::String("flake".to_owned()).wrap_var(),
        );
        out.insert("outPath".to_owned(), NixValue::Path(path).wrap_var());

        out.insert(
            "outputs".to_owned(),
            LazyNixValue::Concrete(flake).wrap_var(),
        );

        Ok((key.clone(), NixValue::AttrSet(out).wrap_var()))
    });

    let outputs = flake.get("outputs").expect("Flake should export `outputs`");

    let outputs = outputs.resolve()?;
    let outputs = outputs.borrow();

    let Some((scope, _param, expr)) = outputs.as_lambda() else {
        todo!("outputs should be a lambda")
    };

    let scope = scope.clone().new_child();
    let outputs = LazyNixValue::Pending(scope.clone(), expr.clone()).wrap_var();

    for input in inputs {
        let (input, input_path) = input?;

        scope.set_variable(input, input_path);
    }

    scope.set_variable("self".to_owned(), outputs.clone());

    outputs.resolve()
}
