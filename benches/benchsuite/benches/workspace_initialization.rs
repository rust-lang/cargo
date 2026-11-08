use benchsuite::{Fixtures, WorkspacePath, fixtures};
use cargo::workspace::Workspace;
use std::sync::LazyLock;

static FIXTURES: LazyLock<Fixtures> = LazyLock::new(|| fixtures!());

#[divan::bench(args = FIXTURES.workspaces())]
fn workspace_initialization(b: divan::Bencher<'_, '_>, WorkspacePath(ws_root): &WorkspacePath) {
    let gctx = FIXTURES.make_context(ws_root);
    b.bench_local(|| Workspace::new(&ws_root.join("Cargo.toml"), &gctx).unwrap());
}

fn main() {
    divan::main();
}
