use benchsuite::fixtures;
use cargo::core::Workspace;
use criterion::{criterion_group, criterion_main, Criterion};

fn workspace_initialization(c: &mut Criterion) {
    let fixtures = fixtures!();
    let mut group = c.benchmark_group("workspace_initialization");
    for (ws_name, ws_root) in fixtures.workspaces() {
        let config = fixtures.make_config(&ws_root);
        // The resolver info is initialized only once in a lazy fashion. This
        // allows criterion to skip this workspace if the user passes a filter
        // on the command-line (like `cargo bench -- workspace_initialization/tikv`).
        group.bench_function(ws_name, |b| {
            b.iter(|| Workspace::new(&ws_root.join("Cargo.toml"), &config).unwrap())
        });
    }
    group.finish();
}

// Criterion complains about the measurement time being too small, but the
// measurement time doesn't seem important to me, what is more important is
// the number of iterations which defaults to 100, which seems like a
// reasonable default. Otherwise, the measurement time would need to be
// changed per workspace. We wouldn't want to spend 60s on every workspace,
// that would take too long and isn't necessary for the smaller workspaces.
criterion_group!(benches, workspace_initialization);
criterion_main!(benches);
