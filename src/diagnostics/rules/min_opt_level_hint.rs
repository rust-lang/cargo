use crate::CargoResult;
use crate::compiler::BuildContext;
use crate::workspace::profiles::{MinOptLevelHintError, parse_min_opt_level_hint};

/// Emits diagnostics for `hints.min-opt-level` once per package selected for compilation.
#[tracing::instrument(skip_all)]
pub(crate) fn diagnose(bcx: &BuildContext<'_, '_>) -> CargoResult<()> {
    let gctx = bcx.gctx;
    let mut packages = bcx
        .unit_graph
        .keys()
        .filter(|unit| !unit.skip_non_compile_time_dep && unit.show_warnings(gctx))
        .map(|unit| unit.pkg.clone())
        .collect::<Vec<_>>();
    packages.sort_by_key(|pkg| pkg.package_id());
    packages.dedup_by_key(|pkg| pkg.package_id());

    for pkg in packages {
        let warn = |message: &str| {
            gctx.shell()
                .warn(format!("{}@{}: {message}", pkg.name(), pkg.version()))
        };
        let min_opt_level = match parse_min_opt_level_hint(
            pkg.hints().and_then(|hints| hints.min_opt_level.as_ref()),
        ) {
            Ok(level) => level,
            Err(MinOptLevelHintError::OutOfRange(level)) => {
                warn(&format!(
                    "ignoring unsupported value ({level}) for 'hints.min-opt-level', which only supports integers from 0 to 3"
                ))?;
                None
            }
            Err(MinOptLevelHintError::WrongType(value_type)) => {
                warn(&format!(
                    "ignoring unsupported value type ({value_type}) for 'hints.min-opt-level', which expects an integer"
                ))?;
                None
            }
        };

        if matches!(min_opt_level, Some(1..=3)) && !gctx.cli_unstable().hint_min_opt_level {
            warn("ignoring 'hints.min-opt-level', pass `-Zhint-min-opt-level` to enable it")?;
        }
    }

    Ok(())
}
