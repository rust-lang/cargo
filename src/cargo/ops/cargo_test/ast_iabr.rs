// ast_iabr.rs
// Purpose: Build syn ASTs for all relevant Rust source files in the
// workspace and provide indexers for specific operator kinds used by
// mutation testing (starting with addition/subtraction).
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
}

/// A single operator occurrence with a stable per-file identifier.
/// The `id` is assigned by a preorder traversal counter; it is stable
/// as long as the source file does not change.
#[derive(Clone, Debug)]
pub struct OpOccurrence {
    pub id: u32,
    pub kind: OpKind,
}

/// Visitor that walks expressions and collects binary `+` and `-` operators.
/// Uses a monotonic `next_id` counter to assign IDs to each matched node.
struct AddSubVisitor {
    next_id: u32,
    occurrences: Vec<OpOccurrence>,
}

impl<'ast> Visit<'ast> for AddSubVisitor {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let kind = match node.op {
            syn::BinOp::Add(_) => Some(OpKind::Add),
            syn::BinOp::Sub(_) => Some(OpKind::Sub),
            _ => None,
        };
        if let Some(kind) = kind {
            let id = self.next_id;
            self.next_id += 1;
            self.occurrences.push(OpOccurrence { id, kind });
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

/// Build the add/sub operator index for a single parsed file.
/// Returns a list of occurrences in preorder, each with a unique `id`.
pub fn index_add_sub_in_file(ast: &File) -> Vec<OpOccurrence> {
    let mut v = AddSubVisitor {
        next_id: 0,
        occurrences: Vec::new(),
    };
    v.visit_file(ast);
    v.occurrences
}

/// Build add/sub operator indexes for the entire workspace.
/// Internally parses files first via `create_trees` and returns matches per file.
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