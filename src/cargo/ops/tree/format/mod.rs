use std::fmt;

use anyhow::{bail, Error};

use self::parse::{Parser, RawChunk};
use super::{Graph, Node};

mod parse;

enum Chunk {
    Raw(String),
    Package,
    License,
    Repository,
    Features,
    LibName,
}

pub struct Pattern(Vec<Chunk>);

impl Pattern {
    pub fn new(format: &str) -> Result<Pattern, Error> {
        let mut chunks = vec![];

        for raw in Parser::new(format) {
            let chunk = match raw {
                RawChunk::Text(text) => Chunk::Raw(text.to_owned()),
                RawChunk::Argument("p") => Chunk::Package,
                RawChunk::Argument("l") => Chunk::License,
                RawChunk::Argument("r") => Chunk::Repository,
                RawChunk::Argument("f") => Chunk::Features,
                RawChunk::Argument("lib") => Chunk::LibName,
                RawChunk::Argument(a) => {
                    bail!("unsupported pattern `{}`", a);
                }
                RawChunk::Error(err) => bail!("{}", err),
            };
            chunks.push(chunk);
        }

        Ok(Pattern(chunks))
    }

    pub fn display<'a>(&'a self, graph: &'a Graph<'a>, node_index: usize) -> Display<'a> {
        Display {
            pattern: self,
            graph,
            node_index,
        }
    }
}

pub struct Display<'a> {
    pattern: &'a Pattern,
    graph: &'a Graph<'a>,
    node_index: usize,
}

impl<'a> fmt::Display for Display<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node = self.graph.node(self.node_index);
        match node {
            Node::Package {
                package_id,
                features,
                ..
            } => {
                let package = self.graph.package_for_id(*package_id);
                for chunk in &self.pattern.0 {
                    match chunk {
                        Chunk::Raw(s) => fmt.write_str(s)?,
                        Chunk::Package => {
                            let proc_macro_suffix = if package.proc_macro() {
                                " (proc-macro)"
                            } else {
                                ""
                            };
                            write!(
                                fmt,
                                "{} v{}{}",
                                package.name(),
                                package.version(),
                                proc_macro_suffix
                            )?;

                            let source_id = package.package_id().source_id();
                            if !source_id.is_default_registry() {
                                write!(fmt, " ({})", source_id)?;
                            }
                        }
                        Chunk::License => {
                            if let Some(license) = &package.manifest().metadata().license {
                                write!(fmt, "{}", license)?;
                            }
                        }
                        Chunk::Repository => {
                            if let Some(repository) = &package.manifest().metadata().repository {
                                write!(fmt, "{}", repository)?;
                            }
                        }
                        Chunk::Features => {
                            write!(fmt, "{}", features.join(","))?;
                        }
                        Chunk::LibName => {
                            if let Some(target) = package
                                .manifest()
                                .targets()
                                .iter()
                                .find(|target| target.is_lib())
                            {
                                write!(fmt, "{}", target.crate_name())?;
                            }
                        }
                    }
                }
            }
            Node::Feature { name, node_index } => {
                let for_node = self.graph.node(*node_index);
                match for_node {
                    Node::Package { package_id, .. } => {
                        write!(fmt, "{} feature \"{}\"", package_id.name(), name)?;
                        if self.graph.is_cli_feature(self.node_index) {
                            write!(fmt, " (command-line)")?;
                        }
                    }
                    // The node_index in Node::Feature must point to a package
                    // node, see `add_feature`.
                    _ => panic!("unexpected feature node {:?}", for_node),
                }
            }
        }

        Ok(())
    }
}
