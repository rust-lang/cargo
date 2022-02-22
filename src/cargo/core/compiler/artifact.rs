/// Generate artifact information from unit dependencies for configuring the compiler environment.
use crate::core::compiler::unit_graph::UnitDep;
use crate::core::compiler::{Context, CrateType, FileFlavor, Unit};
use crate::core::TargetKind;
use crate::CargoResult;
use std::collections::HashMap;
use std::ffi::OsString;

/// Return all environment variables for the given unit-dependencies
/// if artifacts are present.
pub fn get_env(
    cx: &Context<'_, '_>,
    dependencies: &[UnitDep],
) -> CargoResult<HashMap<String, OsString>> {
    let mut env = HashMap::new();
    for unit_dep in dependencies.iter().filter(|d| d.unit.artifact.is_true()) {
        for artifact_path in cx
            .outputs(&unit_dep.unit)?
            .iter()
            .filter_map(|f| (f.flavor == FileFlavor::Normal).then(|| &f.path))
        {
            let artifact_type_upper = unit_artifact_type_name_upper(&unit_dep.unit);
            let dep_name = unit_dep.dep_name.unwrap_or(unit_dep.unit.pkg.name());
            let dep_name_upper = dep_name.to_uppercase().replace("-", "_");

            let var = format!("CARGO_{}_DIR_{}", artifact_type_upper, dep_name_upper);
            let path = artifact_path.parent().expect("parent dir for artifacts");
            env.insert(var, path.to_owned().into());

            let var = format!(
                "CARGO_{}_FILE_{}_{}",
                artifact_type_upper,
                dep_name_upper,
                unit_dep.unit.target.name()
            );
            env.insert(var, artifact_path.to_owned().into());

            if unit_dep.unit.target.name() == dep_name.as_str() {
                let var = format!("CARGO_{}_FILE_{}", artifact_type_upper, dep_name_upper,);
                env.insert(var, artifact_path.to_owned().into());
            }
        }
    }
    Ok(env)
}

fn unit_artifact_type_name_upper(unit: &Unit) -> &'static str {
    match unit.target.kind() {
        TargetKind::Lib(kinds) => match kinds.as_slice() {
            &[CrateType::Cdylib] => "CDYLIB",
            &[CrateType::Staticlib] => "STATICLIB",
            invalid => unreachable!("BUG: artifacts cannot be of type {:?}", invalid),
        },
        TargetKind::Bin => "BIN",
        invalid => unreachable!("BUG: artifacts cannot be of type {:?}", invalid),
    }
}
