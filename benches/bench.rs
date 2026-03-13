//! Criterion benchmarks.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn placeholder_benchmark(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| black_box(1 + 1)));
}

criterion_group!(benches, placeholder_benchmark);
criterion_main!(benches);
