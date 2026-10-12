#![allow(clippy::wildcard_imports)] // 実験対象を一括で取り込む
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use parallel::*;
use std::time::Duration;

fn cfg() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3))
        .sample_size(20)
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, std::num::NonZero::get)
}

fn bench_cpu(c: &mut Criterion) {
    let r = 0..2_000_000u64;
    let t = threads();
    let mut g = c.benchmark_group("cpu/primes_2M");
    g.bench_function("sequential", |b| {
        b.iter(|| count_primes_seq(black_box(r.clone())));
    });
    g.bench_function("scoped_equal_split", |b| {
        b.iter(|| count_primes_scoped(black_box(r.clone()), t));
    });
    g.bench_function("scoped_strided", |b| {
        b.iter(|| count_primes_scoped_strided(black_box(r.clone()), t));
    });
    g.bench_function("rayon", |b| b.iter(|| count_primes_rayon(black_box(r.clone()))));
    g.finish();
}

fn bench_tiny(c: &mut Criterion) {
    for n in [1_000usize, 100_000, 10_000_000] {
        let v: Vec<u64> = (0..n as u64).collect();
        let mut g = c.benchmark_group(format!("tiny_work/sum_{n}"));
        g.bench_function("sequential", |b| b.iter(|| sum_seq(black_box(&v))));
        g.bench_function("rayon", |b| b.iter(|| sum_rayon_tiny(black_box(&v))));
        g.bench_function("rayon_min_len_64k", |b| {
            b.iter(|| sum_rayon_min_len(black_box(&v), 1 << 16));
        });
        g.finish();
    }
}

fn bench_counters(c: &mut Criterion) {
    let t = threads();
    let per = 200_000;
    let mut g = c.benchmark_group("shared_counter/4threads_x200k");
    g.bench_function("mutex", |b| b.iter(|| counter_mutex(t, per)));
    g.bench_function("single_atomic", |b| b.iter(|| counter_atomic(t, per)));
    g.bench_function("per_thread_atomic_false_sharing", |b| {
        b.iter(|| counter_false_sharing(t, per));
    });
    g.bench_function("per_thread_atomic_padded", |b| b.iter(|| counter_padded(t, per)));
    g.bench_function("local_then_merge", |b| {
        b.iter(|| counter_local_then_merge(t, per));
    });
    g.finish();
}

fn bench_channels(c: &mut Criterion) {
    let n = 1_000_000;
    let mut g = c.benchmark_group("channel/1M_msgs");
    g.bench_function("std_unbounded", |b| b.iter(|| channel_std(n, None)));
    g.bench_function("std_bounded_1024", |b| b.iter(|| channel_std(n, Some(1024))));
    g.bench_function("crossbeam_bounded_1024", |b| {
        b.iter(|| channel_crossbeam(n, 1024));
    });
    g.bench_function("std_batched_256", |b| b.iter(|| channel_std_batched(n, 256)));
    g.finish();
}

criterion_group! {
    name = benches;
    config = cfg();
    targets = bench_cpu, bench_tiny, bench_counters, bench_channels
}
criterion_main!(benches);
