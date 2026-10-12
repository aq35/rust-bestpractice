#!/usr/bin/env bash
# すべての実験を実行し、docs/results/ に生の結果を書き出す。
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p docs/results
cargo test --workspace -q
cargo run -q --release -p availability > docs/results/availability.md
cargo run -q --release -p parallel --bin tokio_cpu > docs/results/tokio_cpu.md
cargo bench -q -p perf -- --noplot 2>&1 | tee target/perf_bench.txt
cargo bench -q -p parallel -- --noplot 2>&1 | tee target/parallel_bench.txt
python3 scripts/criterion_table.py target/criterion > docs/results/benches.md
scripts/build_matrix.sh > docs/results/build.md
