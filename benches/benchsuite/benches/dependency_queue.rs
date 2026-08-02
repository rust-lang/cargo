use cargo::util::DependencyQueue;
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

fn independent_nodes(nodes: usize) -> DependencyQueue<usize, (), ()> {
    let mut queue = DependencyQueue::new();
    for node in 0..nodes {
        queue.queue(node, (), std::iter::empty::<(usize, ())>(), 1);
    }
    queue.queue_finished();
    queue
}

fn blocked_frontier(nodes: usize) -> DependencyQueue<usize, (), ()> {
    let mut queue = DependencyQueue::new();
    queue.queue(0, (), std::iter::empty::<(usize, ())>(), 1);
    for node in 1..=nodes {
        queue.queue(node, (), [(0, ())], 1);
    }
    queue.queue_finished();
    assert_eq!(queue.dequeue(), Some((0, (), nodes + 2)));
    queue
}

fn drain(queue: &mut DependencyQueue<usize, (), ()>) -> usize {
    let mut dequeued = 0;
    while queue.dequeue().is_some() {
        dequeued += 1;
    }
    dequeued
}

fn drain_ready_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_queue/drain_ready");

    for nodes in [100usize, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &nodes, |b, &nodes| {
            b.iter_batched(
                || independent_nodes(nodes),
                |mut queue| assert_eq!(drain(&mut queue), nodes),
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn release_wide_frontier(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_queue/release_wide_frontier");

    for nodes in [100usize, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &nodes, |b, &nodes| {
            b.iter_batched(
                || blocked_frontier(nodes),
                |mut queue| {
                    assert_eq!(queue.finish(&0, &()).len(), nodes);
                    assert_eq!(drain(&mut queue), nodes);
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, drain_ready_nodes, release_wide_frontier);
criterion_main!(benches);
