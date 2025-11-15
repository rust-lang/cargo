// mutators.rs

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use syn::File;
use syn::fold::{fold_expr_binary, Fold};

use super::ast_iabr::{build_index_with_cache, OpOccurrence, IndexResult};
use crate::core::Workspace;

/// Standard, mutator-agnostic target: address a single operator occurrence by
/// file path and a stable per-file `id`.
#[derive(Clone, Debug)]
pub struct Target {
    pub path: PathBuf,
    pub id: u32,
    pub line: u32,
    pub column: u32,
}

/// A standardized, minimal interface for mutation operators.
pub trait Mutator {
    /// Machine-readable name used in logs (e.g., "add_sub").
    fn name(&self) -> &'static str;
    /// Build any per-run indexes and caches needed by this mutator.
    fn build_context(&self, ws: &Workspace<'_>) -> syn::Result<MutationContext>;
    /// Enumerate all mutation targets in deterministic order.
    fn enumerate_targets(&self, ctx: &MutationContext) -> Vec<Target>;
    /// Produce a mutated source text for the given target.
    fn mutate(&self, ctx: &MutationContext, target: &Target) -> syn::Result<String>;
}

/// Context holding operator indexes and selectively cached ASTs.
pub struct MutationContext {
    pub index: HashMap<PathBuf, Vec<OpOccurrence>>, // files -> occurrences
    pub cached_asts: HashMap<PathBuf, File>,        // heavy files only
}

/// Add/Sub mutator.
pub struct AddSubMutator;

impl Mutator for AddSubMutator {
    fn name(&self) -> &'static str { "add_sub" }

    fn build_context(&self, ws: &Workspace<'_>) -> syn::Result<MutationContext> {
        let IndexResult { index, cached_asts } = build_index_with_cache(ws)?;
        Ok(MutationContext { index, cached_asts })
    }

    fn enumerate_targets(&self, ctx: &MutationContext) -> Vec<Target> {
        let mut out = Vec::new();
        for (path, occs) in ctx.index.iter() {
            for occ in occs {
                out.push(Target { path: path.clone(), id: occ.id, line: occ.line, column: occ.column });
            }
        }
        out
    }

    fn mutate(&self, ctx: &MutationContext, target: &Target) -> syn::Result<String> {
        mutate_add_sub(ctx, &target.path, target.id)
    }
}

/// Flip Add<->Sub for the given occurrence id in the specified file.
/// Uses cached AST if available; otherwise parses on demand.
fn mutate_add_sub(ctx: &MutationContext, path: &Path, id: u32) -> syn::Result<String> {
    // Fetch AST from cache or parse source.
    let ast: File = if let Some(cached) = ctx.cached_asts.get(path) {
        cached.clone()
    } else {
        let src = fs::read_to_string(path).expect("Failed to open file");
        syn::parse_file(&src)?
    };

    // Fold and flip the targeted operator.
    let mut folder = AddSubFlipFold { target_id: id, seen: 0 };
    let mutated = folder.fold_file(ast);

    // Pretty-print the mutated AST back to source.
    Ok(prettyplease::unparse(&mutated))
}

/// Folder that flips the Nth add/sub operator where N == target_id.
struct AddSubFlipFold {
    target_id: u32,
    seen: u32,
}

impl Fold for AddSubFlipFold {
    fn fold_expr_binary(&mut self, mut node: syn::ExprBinary) -> syn::ExprBinary {
        let is_add = matches!(node.op, syn::BinOp::Add(_));
        let is_sub = matches!(node.op, syn::BinOp::Sub(_));
        if is_add || is_sub {
            if self.seen == self.target_id {
                node.op = if is_add {
                    syn::BinOp::Sub(Default::default())
                } else {
                    syn::BinOp::Add(Default::default())
                };
            }
            self.seen += 1;
        }
        fold_expr_binary(self, node)
    }
}
