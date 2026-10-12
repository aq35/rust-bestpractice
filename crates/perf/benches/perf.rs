#![allow(clippy::unwrap_used)]
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perf::*;
use std::time::Duration;

fn cfg() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(30)
}

fn lines(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("line {i} with some words in it {}", i * 31))
        .collect()
}

fn bench_alloc(c: &mut Criterion) {
    let mut g = c.benchmark_group("alloc/vec_100k");
    let n = 100_000;
    g.bench_function("push_no_capacity", |b| b.iter(|| alloc::push_no_capacity(black_box(n))));
    g.bench_function("push_with_capacity", |b| b.iter(|| alloc::push_with_capacity(black_box(n))));
    g.bench_function("collect", |b| b.iter(|| alloc::collect(black_box(n))));
    g.finish();

    let owned = lines(10_000);
    let l: Vec<&str> = owned.iter().map(String::as_str).collect();
    let mut g = c.benchmark_group("alloc/buffer_10k_lines");
    g.bench_function("fresh_each_iter", |b| b.iter(|| alloc::fresh_buffer_each_iter(black_box(&l))));
    g.bench_function("reuse", |b| b.iter(|| alloc::reuse_buffer(black_box(&l))));
    g.finish();
}

fn bench_strings(c: &mut Criterion) {
    let v: Vec<u32> = (0..2_000).collect();
    let mut g = c.benchmark_group("string/build_2k");
    g.bench_function("format_concat", |b| b.iter(|| strings::format_concat(black_box(&v))));
    g.bench_function("push_str_to_string", |b| b.iter(|| strings::push_str_tostring(black_box(&v))));
    g.bench_function("write_with_capacity", |b| b.iter(|| strings::write_into(black_box(&v))));
    g.finish();
}

fn bench_hash(c: &mut Criterion) {
    use hashing::*;
    let keys: Vec<u64> = (0..100_000u64).map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15)).collect();
    let mut g = c.benchmark_group("hash/u64_100k");
    g.bench_function("std_siphash", |b| b.iter(|| insert_lookup(black_box(&keys), StdMap::default())));
    g.bench_function("std_siphash_with_capacity", |b| {
        b.iter(|| insert_lookup(black_box(&keys), StdMap::with_capacity(keys.len())));
    });
    g.bench_function("fxhash", |b| b.iter(|| insert_lookup(black_box(&keys), FxMap::default())));
    g.bench_function("ahash", |b| b.iter(|| insert_lookup(black_box(&keys), AMap::default())));
    g.finish();

    let skeys: Vec<String> = (0..50_000).map(|i| format!("user:{i:08}:session")).collect();
    let mut g = c.benchmark_group("hash/string_50k");
    g.bench_function("std_siphash", |b| b.iter(|| insert_lookup_str(black_box(&skeys), StdMap::default())));
    g.bench_function("fxhash", |b| b.iter(|| insert_lookup_str(black_box(&skeys), FxMap::default())));
    g.bench_function("ahash", |b| b.iter(|| insert_lookup_str(black_box(&skeys), AMap::default())));
    g.finish();
}

fn bench_dispatch(c: &mut Criterion) {
    use dispatch::*;
    let n = 100_000;
    let dyns: Vec<Box<dyn Shape>> = (0..n)
        .map(|i| -> Box<dyn Shape> {
            if i % 2 == 0 { Box::new(Circle(f64::from(i))) } else { Box::new(Rect(1.0, f64::from(i))) }
        })
        .collect();
    let enums: Vec<ShapeE> = (0..n)
        .map(|i| if i % 2 == 0 { ShapeE::Circle(f64::from(i)) } else { ShapeE::Rect(1.0, f64::from(i)) })
        .collect();
    let rects: Vec<Rect> = (0..n).map(|i| Rect(1.0, f64::from(i))).collect();
    let mut g = c.benchmark_group("dispatch/100k");
    g.bench_function("box_dyn_mixed", |b| b.iter(|| sum_dyn(black_box(&dyns))));
    g.bench_function("enum_mixed", |b| b.iter(|| sum_enum(black_box(&enums))));
    g.bench_function("generic_single_type", |b| b.iter(|| sum_generic(black_box(&rects))));
    g.finish();
}

fn bench_iter(c: &mut Criterion) {
    let n = 1 << 16;
    let a: Vec<f32> = (0..n).map(|i| (i % 13) as f32 * 0.5).collect();
    let bb: Vec<f32> = (0..n).map(|i| (i % 7) as f32 * 0.25).collect();
    let mut g = c.benchmark_group("iter/dot_f32_64k");
    g.bench_function("index", |b| b.iter(|| iter::dot_index(black_box(&a), black_box(&bb))));
    g.bench_function("zip", |b| b.iter(|| iter::dot_zip(black_box(&a), black_box(&bb))));
    g.bench_function("chunks8_multi_acc", |b| b.iter(|| iter::dot_chunks8(black_box(&a), black_box(&bb))));
    g.finish();

    let v: Vec<u64> = (0..n as u64).collect();
    let mut g = c.benchmark_group("iter/sum_u64_64k");
    g.bench_function("index", |b| b.iter(|| iter::sum_u64_index(black_box(&v))));
    g.bench_function("iter_fold", |b| b.iter(|| iter::sum_u64_iter(black_box(&v))));
    g.finish();
}

fn bench_layout(c: &mut Criterion) {
    use layout::*;
    let n = 1_000_000;
    let aos: Vec<Particle> = (0..n).map(|i| Particle { mass: (i % 10) as f32, ..Default::default() }).collect();
    let soa = Particles { mass: (0..n).map(|i| (i % 10) as f32).collect(), ..Default::default() };
    let mut g = c.benchmark_group("layout/sum_mass_1M");
    g.bench_function("aos_64B_struct", |b| b.iter(|| total_mass_aos(black_box(&aos))));
    g.bench_function("soa", |b| b.iter(|| total_mass_soa(black_box(&soa))));
    g.finish();
}

fn bench_errors(c: &mut Criterion) {
    let inputs_ok: Vec<String> = (0..10_000).map(|i| i.to_string()).collect();
    let inputs_bad: Vec<String> = (0..10_000).map(|i| format!("{i}x")).collect();
    for (name, inputs) in [("all_ok", &inputs_ok), ("all_err", &inputs_bad)] {
        let mut g = c.benchmark_group(format!("errors/{name}_10k"));
        g.bench_function("thiserror_enum", |b| {
            b.iter(|| inputs.iter().filter(|s| errors::parse_typed(black_box(s)).is_ok()).count());
        });
        g.bench_function("anyhow", |b| {
            b.iter(|| inputs.iter().filter(|s| errors::parse_anyhow(black_box(s)).is_ok()).count());
        });
        g.finish();
    }
}

fn bench_small(c: &mut Criterion) {
    let owned = lines(10_000);
    let l: Vec<&str> = owned.iter().map(String::as_str).collect();
    let mut g = c.benchmark_group("small/tokens_10k_lines");
    g.bench_function("vec", |b| b.iter(|| small::tokens_vec(black_box(&l))));
    g.bench_function("smallvec_8", |b| b.iter(|| small::tokens_smallvec(black_box(&l))));
    g.finish();
}

criterion_group! {
    name = benches;
    config = cfg();
    targets = bench_alloc, bench_strings, bench_hash, bench_dispatch, bench_iter, bench_layout, bench_errors, bench_small
}
criterion_main!(benches);
