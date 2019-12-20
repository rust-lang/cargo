use crate::core::Workspace;
use crate::ops::CompileOptions;
use crate::util::{CliError, CliResult};

use failure;
use toml::Value;

/// Check if specific features are available under `[package.metadata.X]` (`X`
/// being one of the supported types).
///
/// For now, it only supports it for "doc" build.
pub fn get_features_for(
    r#for: &str,
    compile_opts: &mut CompileOptions<'_>,
    ws: &Workspace<'_>,
) -> CliResult {
    match r#for {
        "doc" => {} // If other features-specific kind are added, they should be put here.
        _ => return Ok(()),
    }
    let packages = match compile_opts.spec.get_packages(ws) {
        Ok(p) => p,
        _ => return Ok(()),
    };
    for package in packages {
        if let Some(Value::Table(ref metadata)) = package.manifest().custom_metadata() {
            if let Some(ref doc) = metadata.get(r#for) {
                match doc.get("features") {
                    Some(Value::Array(ref features)) => {
                        let mut additional_features = Vec::with_capacity(2);

                        for feature in features.iter() {
                            if let Value::String(s) = feature {
                                additional_features.push(s.clone());
                            } else {
                                Err(CliError::new(
                                    failure::format_err!(
                                        "Only strings are allowed in `features` array"
                                    ),
                                    1,
                                ))?
                            }
                        }
                        compile_opts
                            .features
                            .extend(additional_features.into_iter());
                    }
                    Some(_) => Err(CliError::new(
                        failure::format_err!("`features` should be an array"),
                        1,
                    ))?,
                    None => {}
                }
            }
        }
    }
    Ok(())
}
