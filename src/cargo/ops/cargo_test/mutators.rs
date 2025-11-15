// mutators.rs

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use syn::{File};
use syn::fold::{Fold, fold_expr_binary};

use super::ast_iabr::{build_index_with_cache, OpOccurrence, IndexResult};
use crate::core::Workspace;

/// Mutation kinds supported (extensible).
pub enum MutationKind {
    AddSub,
}

/// A specific mutation target within a file.
pub struct MutationTarget {
    pub path: PathBuf,
    pub id: u32,
    pub kind: MutationKind,
}

/// Context holding operator indexes and selectively cached ASTs.
/// Build once per run; reuse across mutators.
pub struct MutationContext {
    pub index: HashMap<PathBuf, Vec<OpOccurrence>>, // all files with targets
    pub cached_asts: HashMap<PathBuf, File>,         // only heavy files
}

impl MutationContext {
    /// Build the index and cache using the workspace and threshold policy.
    pub fn build(ws: &Workspace<'_>) -> syn::Result<Self> {
        let IndexResult { index, cached_asts } = build_index_with_cache(ws)?;
        Ok(Self { index, cached_asts })
    }

    /// Return all add/sub targets for a file, if any.
    pub fn targets_for_file(&self, path: &Path) -> Option<&[OpOccurrence]> {
        self.index.get(path).map(|v| v.as_slice())
    }

    /// Apply a mutation by kind. For now, only Add<->Sub is supported.
    pub fn apply(&self, target: &MutationTarget) -> syn::Result<String> {
        match target.kind {
            MutationKind::AddSub => self.mutate_add_sub(&target.path, target.id),
        }
    }

    /// Flip Add<->Sub for the given occurrence id in the specified file.
    /// Uses cached AST if available; otherwise parses on demand.
    pub fn mutate_add_sub(&self, path: &Path, id: u32) -> syn::Result<String> {
        // Fetch AST from cache or parse source.
        let (mut ast, source_owned);
        if let Some(cached) = self.cached_asts.get(path) {
            ast = cached.clone();
            source_owned = None;
        } else {
            let src = fs::read_to_string(path).expect("Failed to open file");
            let parsed: File = syn::parse_file(&src)?;
            ast = parsed;
            source_owned = Some(src);
        }

        // Fold and flip the targeted operator.
        let mut folder = AddSubFlipFold { target_id: id, seen: 0 };
        let mutated = folder.fold_file(ast);

        // Pretty-print the mutated AST back to source.
        let out = prettyplease::unparse(&mutated);
        Ok(out)
    }
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
                // Flip the operator.
                node.op = if is_add { syn::BinOp::Sub(Default::default()) } else { syn::BinOp::Add(Default::default()) };
            }
            self.seen += 1;
        }
        fold_expr_binary(self, node)
    }
}
