//! Criterion benchmarks for graph, lanes, and diff operations.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use gitfast_core::{assign_lanes, diff, graph};
use std::hint::black_box;
use tokio::runtime::Runtime;

const TEST_REPO: &str = "/Users/sarvadhisolution/Desktop/git-test";

fn bench_get_commits(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    c.bench_function("get_commits_200", |b| {
        b.iter(|| {
            let commits = rt
                .block_on(graph::get_commits(black_box(TEST_REPO), 200, 0))
                .unwrap();
            black_box(commits)
        })
    });
}

fn bench_assign_lanes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    c.bench_function("assign_lanes_200", |b| {
        b.iter_batched(
            || {
                rt.block_on(graph::get_commits(TEST_REPO, 200, 0))
                    .expect("get_commits failed")
            },
            |commits| {
                let laned = assign_lanes(black_box(commits));
                black_box(laned)
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_diff_commit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let first_hash = rt
        .block_on(graph::get_commits(TEST_REPO, 1, 0))
        .expect("get_commits failed")
        .into_iter()
        .next()
        .map(|c| c.hash)
        .expect("repo has no commits");

    c.bench_function("diff_commit", |b| {
        b.iter(|| {
            let files = rt
                .block_on(diff::diff_commit(black_box(TEST_REPO), black_box(&first_hash)))
                .unwrap();
            black_box(files)
        })
    });
}

criterion_group!(benches, bench_get_commits, bench_assign_lanes, bench_diff_commit);
criterion_main!(benches);
