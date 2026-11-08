use benchsuite::{Fixtures, fixtures};
use cargo::workspace::Workspace;
use std::sync::LazyLock;

static FIXTURES: LazyLock<Fixtures> = LazyLock::new(|| fixtures!());

#[divan::bench(args = FIXTURES.workspaces().into_iter().map(|(n, _)| n))]
fn workspace_initialization(b: divan::Bencher<'_, '_>, ws_name: &String) {
    let ws_root = FIXTURES
        .workspaces()
        .into_iter()
        .find_map(|(n, r)| (n == *ws_name).then_some(r))
        .unwrap();
    let gctx = FIXTURES.make_context(&ws_root);
    b.bench_local(|| Workspace::new(&ws_root.join("Cargo.toml"), &gctx).unwrap());
}

fn main() {
    divan::main();
}
