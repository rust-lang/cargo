//! A graph-like structure used to represent the rustc commands to build the package and the
//! interdependencies between them.
//!
//! The BuildPlan structure is used to store the dependency graph of a dry run so that it can be
//! shared with an external build system. Each Invocation in the BuildPlan comprises a single
//! subprocess and defines the build environment, the outputs produced by the subprocess, and the
//! dependencies on other Invocations.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use super::context::OutputFile;
use super::{CompileKind, CompileMode, Context, Unit};
use crate::core::TargetKind;
use crate::util::{internal, CargoResult, ProcessBuilder};

#[derive(Debug, Serialize)]
struct Invocation {
    package_name: String,
    package_version: semver::Version,
    target_kind: TargetKind,
    kind: CompileKind,
    compile_mode: CompileMode,
    deps: Vec<usize>,
    outputs: Vec<PathBuf>,
    links: BTreeMap<PathBuf, PathBuf>,
    program: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: Option<PathBuf>,
}

#[derive(Debug)]
pub struct BuildPlan {
    invocation_map: BTreeMap<String, usize>,
    plan: SerializedBuildPlan,
}

#[derive(Debug, Serialize)]
struct SerializedBuildPlan {
    invocations: Vec<Invocation>,
    inputs: Vec<PathBuf>,
}

impl Invocation {
    pub fn new(unit: &Unit<'_>, deps: Vec<usize>) -> Invocation {
        let id = unit.pkg.package_id();
        Invocation {
            package_name: id.name().to_string(),
            package_version: id.version().clone(),
            kind: unit.kind,
            target_kind: unit.target.kind().clone(),
            compile_mode: unit.mode,
            deps,
            outputs: Vec::new(),
            links: BTreeMap::new(),
            program: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
        }
    }

    pub fn add_output(&mut self, path: &PathBuf, link: &Option<PathBuf>) {
        self.outputs.push(path.clone());
        if let Some(ref link) = *link {
            self.links.insert(link.clone(), path.clone());
        }
    }

    pub fn update_cmd(&mut self, cmd: &ProcessBuilder) -> CargoResult<()> {
        self.program = cmd
            .get_program()
            .to_str()
            .ok_or_else(|| failure::format_err!("unicode program string required"))?
            .to_string();
        self.cwd = Some(cmd.get_cwd().unwrap().to_path_buf());
        for arg in cmd.get_args().iter() {
            self.args.push(
                arg.to_str()
                    .ok_or_else(|| failure::format_err!("unicode argument string required"))?
                    .to_string(),
            );
        }
        for (var, value) in cmd.get_envs() {
            let value = match value {
                Some(s) => s,
                None => continue,
            };
            self.env.insert(
                var.clone(),
                value
                    .to_str()
                    .ok_or_else(|| failure::format_err!("unicode environment value required"))?
                    .to_string(),
            );
        }
        Ok(())
    }
}

impl BuildPlan {
    pub fn new() -> BuildPlan {
        BuildPlan {
            invocation_map: BTreeMap::new(),
            plan: SerializedBuildPlan::new(),
        }
    }

    pub fn add<'a>(&mut self, cx: &Context<'a, '_>, unit: &Unit<'a>) -> CargoResult<()> {
        let id = self.plan.invocations.len();
        self.invocation_map.insert(unit.buildkey(), id);
        let deps = cx
            .dep_targets(unit)
            .iter()
            .map(|dep| self.invocation_map[&dep.buildkey()])
            .collect();
        let invocation = Invocation::new(unit, deps);
        self.plan.invocations.push(invocation);
        Ok(())
    }

    pub fn update(
        &mut self,
        invocation_name: &str,
        cmd: &ProcessBuilder,
        outputs: &[OutputFile],
    ) -> CargoResult<()> {
        let id = self.invocation_map[invocation_name];
        let invocation =
            self.plan.invocations.get_mut(id).ok_or_else(|| {
                internal(format!("couldn't find invocation for {}", invocation_name))
            })?;

        invocation.update_cmd(cmd)?;
        for output in outputs.iter() {
            invocation.add_output(&output.path, &output.hardlink);
        }

        Ok(())
    }

    pub fn set_inputs(&mut self, inputs: Vec<PathBuf>) {
        self.plan.inputs = inputs;
    }

    pub fn output_plan(self) {
        let encoded = serde_json::to_string(&self.plan).unwrap();
        println!("{}", encoded);
    }
}

impl SerializedBuildPlan {
    pub fn new() -> SerializedBuildPlan {
        SerializedBuildPlan {
            invocations: Vec::new(),
            inputs: Vec::new(),
        }
    }
}
