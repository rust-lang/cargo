//! A graph-like structure used to represent the rustc commands to build the project and the
//! interdependencies between them.
//!
//! The BuildPlan structure is used to store the dependency graph of a dry run so that it can be
//! shared with an external build system. Each Module in the BuildPlan comprises a single
//! subprocess and defines the build environment, the outputs produced by the subprocess, and the
//! dependencies on other Modules.

use std::collections::{HashMap};

use super::{Context, Unit};
use util::{internal, CargoResult, ProcessBuilder};
use std::sync::Arc;
use std::path::PathBuf;
use serde_json;

#[derive(Debug, Serialize)]
struct Module {
    deps: Vec<String>,
    outputs: Vec<PathBuf>,
    links: HashMap<PathBuf, PathBuf>,
    program: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    cwd: String,
}

#[derive(Debug, Serialize)]
pub struct BuildPlan {
    modules: HashMap<String, Module>,
}

impl Module {
    pub fn new(deps: Vec<Unit>) -> Module {
        Module {
            deps: deps.iter().map(|dep| buildkey(dep)).collect(),
            outputs: Vec::new(),
            links: HashMap::new(),
            program: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: String::new(),
        }
    }

    pub fn add_output(&mut self, path: &PathBuf, link: &Option<PathBuf>) {
        self.outputs.push(path.clone());
        if link.is_some() {
            self.links.insert(link.as_ref().unwrap().clone(), path.clone());
        }
    }

    pub fn update_cmd(&mut self, cmd: ProcessBuilder) {
        self.program = cmd.get_program().to_str().expect("unicode program string required").to_string().clone();
        self.cwd = cmd.get_cwd().expect("cwd is required").to_str().expect("unicode cwd string required").to_string().clone();
        for arg in cmd.get_args().iter() {
            self.args.push(arg.to_str().expect("unicode argument string required").to_string().clone());
        }
        for (var, value) in cmd.get_envs() {
            self.env.insert(var.clone(), value.as_ref().expect("environment value required").to_str().expect("unicode environment value required").to_string().clone());
        }
    }
}

pub fn buildkey(unit: &Unit) -> String {
    format!("{} {} {} {:?}", unit.pkg, unit.target, unit.profile, unit.kind)
}

impl BuildPlan {
    pub fn new() -> BuildPlan {
        BuildPlan {
            modules: HashMap::new(),
        }
    }

    pub fn add(&mut self,
               cx: &Context,
               unit: &Unit,
              ) -> CargoResult<()> {
        let k = buildkey(unit);
        let deps = cx.dep_targets(&unit)?;
        let module = Module::new(deps);
        self.modules.insert(k, module);
        Ok(())
    }

    pub fn update(&mut self,
                  module_name: String,
                  cmd: ProcessBuilder,
                  filenames: Arc<Vec<(PathBuf, Option<PathBuf>, bool)>>,
                  ) -> CargoResult<()> {
        let module = self.modules.get_mut(&module_name).ok_or_else(|| {
            internal(format!("couldn't find module for {}", module_name))
            })?;

        module.update_cmd(cmd);
        for &(ref dst, ref link_dst, _) in filenames.iter() {
            module.add_output(dst, link_dst);
        }

        Ok(())
    }

    pub fn output_plan(self) {
        let encoded = serde_json::to_string_pretty(&self).unwrap();
        println!("{}", encoded);
    }
}
