// ast_iabr.rs
// Purpose: Build syn ASTs for all relevant Rust source files in the
// workspace and provide indexers for specific operator kinds used by
// mutation testing (addition/subtraction/multiplication/division).
// Key references:
// - Workspace file discovery uses Cargo's own `sources::path::list_files` to
//   respect include/exclude rules and ignore `target/` and subpackages.
// - Parsing uses `syn` with the `full` feature to get complete AST nodes.
// - Traversal uses `syn::visit::Visit` to collect operator occurrences.
use crate::core::Workspace;
use crate::sources::path as src_path;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use syn::File;
use syn::visit::Visit;


/// Parse all relevant `.rs` files for each workspace member into syn `File` ASTs.
/// Returns a map from absolute file paths to their parsed ASTs.
pub fn create_trees(ws: &Workspace<'_>) -> syn::Result<HashMap<PathBuf, File>> {
    let mut trees = HashMap::new();

    // Iterate over all packages in the workspace (respects `[workspace]`).
    for package in ws.members() {
        // Use Cargo's file lister to get the authoritative set of files
        let files = match src_path::list_files(package, ws.gctx()) {
            Ok(list) => list,
            Err(_) => continue,
        };

        for entry in files.into_iter() {
            if !entry.is_file() {
                continue;
            }
            let path = entry.into_path_buf();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }

            // Read source and parse into a full-file AST.
            let source = fs::read_to_string(&path).expect("Failed to open file");
            let ast: File = syn::parse_file(&source)?;
            // Minimal progress logging; avoids dumping full ASTs to stderr.
            eprintln!("Tree created for {:?}", path);
            trees.insert(path, ast);
        }
    }

    Ok(trees)
}

/// Kinds of operators we currently index for mutation testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    Add,
    Sub,
    Mul,
    Div,
}

/// A single operator occurrence with a stable per-file identifier.
/// The `id` is assigned by a preorder traversal counter; it is stable
/// as long as the source file does not change.
#[derive(Clone, Debug)]
pub struct OpOccurrence {
    pub id: u32,
    pub kind: OpKind,
    pub line: u32,
    pub column: u32,
}

/// Visitor that walks expressions and collects binary arithmetic operators.
/// Uses a monotonic `next_id` counter to assign IDs to each matched node.
struct ArithVisitor {
    next_id: u32,
    occurrences: Vec<OpOccurrence>,
}

impl<'ast> Visit<'ast> for ArithVisitor {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let (kind, line, column) = match &node.op {
            syn::BinOp::Add(tok) => {
                let lc = tok.span.start();
                (Some(OpKind::Add), lc.line as u32, lc.column as u32)
            }
            syn::BinOp::Sub(tok) => {
                let lc = tok.span.start();
                (Some(OpKind::Sub), lc.line as u32, lc.column as u32)
            }
            syn::BinOp::Mul(tok) => {
                let lc = tok.span.start();
                (Some(OpKind::Mul), lc.line as u32, lc.column as u32)
            }
            syn::BinOp::Div(tok) => {
                let lc = tok.span.start();
                (Some(OpKind::Div), lc.line as u32, lc.column as u32)
            }
            _ => (None, 0, 0),
        };
        if let Some(kind) = kind {
            let id = self.next_id;
            self.next_id += 1;
            self.occurrences.push(OpOccurrence { id, kind, line, column });
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

/// Build the operator index for a single parsed file.
/// Returns a list of occurrences in preorder, each with a unique `id`.
/// Operators indexed include `+`, `-`, `*`, and `/`.
pub fn index_add_sub_in_file(ast: &File) -> Vec<OpOccurrence> {
    let mut v = ArithVisitor {
        next_id: 0,
        occurrences: Vec::new(),
    };
    v.visit_file(ast);
    v.occurrences
}

/// Build operator indexes for the entire workspace.
/// Internally parses files first via `create_trees` and returns matches per file.
/// Operators indexed include `+`, `-`, `*`, and `/`.
pub fn index_add_sub(ws: &Workspace<'_>) -> syn::Result<HashMap<PathBuf, Vec<OpOccurrence>>> {
    let trees = create_trees(ws)?;
    let mut index: HashMap<PathBuf, Vec<OpOccurrence>> = HashMap::new();
    for (path, file_ast) in trees.iter() {
        let occ = index_add_sub_in_file(file_ast);
        if !occ.is_empty() {
            index.insert(path.clone(), occ);
        }
    }
    Ok(index)
}

/// Variant of the indexer that reuses already-parsed ASTs to avoid reparsing.
/// Operators indexed include `+`, `-`, `*`, and `/`.
pub fn index_add_sub_from_trees(
    trees: &HashMap<PathBuf, File>,
) -> HashMap<PathBuf, Vec<OpOccurrence>> {
    let mut index: HashMap<PathBuf, Vec<OpOccurrence>> = HashMap::new();
    for (path, file_ast) in trees.iter() {
        let occ = index_add_sub_in_file(file_ast);
        if !occ.is_empty() {
            index.insert(path.clone(), occ);
        }
    }
    index
}

/// Minimum number of operator occurrences in a file to keep its AST cached.
pub const CACHE_THRESHOLD: usize = 10;

/// Index arithmetic operators across the workspace while caching only
/// ASTs for files with at least `CACHE_THRESHOLD` targets.
/// This balances memory usage against repeated parse cost for files
/// with many planned mutations.
pub struct IndexResult {
    pub index: HashMap<PathBuf, Vec<OpOccurrence>>,   // all files with ≥1 target
    pub cached_asts: HashMap<PathBuf, File>,          // only files with ≥CACHE_THRESHOLD
}

pub fn build_index_with_cache(ws: &Workspace<'_>) -> syn::Result<IndexResult> {
    let mut index: HashMap<PathBuf, Vec<OpOccurrence>> = HashMap::new();
    let mut cached_asts: HashMap<PathBuf, File> = HashMap::new();

    // Iterate over packages and enumerate relevant files via Cargo.
    for package in ws.members() {
        let files = match src_path::list_files(package, ws.gctx()) {
            Ok(list) => list,
            Err(_) => continue,
        };

        for entry in files.into_iter() {
            if !entry.is_file() {
                continue;
            }
            let path = entry.into_path_buf();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }

            // Parse once per file, decide whether to cache based on count.
            let source = fs::read_to_string(&path).expect("Failed to open file");
            let ast: File = syn::parse_file(&source)?;
            let occ = index_add_sub_in_file(&ast);
            if occ.is_empty() {
                continue;
            }
            if occ.len() >= CACHE_THRESHOLD {
                cached_asts.insert(path.clone(), ast);
            }
            index.insert(path, occ);
        }
    }

    Ok(IndexResult { index, cached_asts })
}