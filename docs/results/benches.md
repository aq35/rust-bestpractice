# ベンチマーク結果（criterion、平均）

環境は README の「計測環境」。`errors/*` は `RUST_BACKTRACE` を外して測った値。
`RUST_BACKTRACE=1` のときは `errors/all_err_10k` の anyhow が **45.13 ms（thiserror の 586 倍）** になった。

#### alloc/buffer_10k_lines

| 実装 | 平均 | 最速比 |
|---|---|---|
| reuse | 796.56 µs | 1.00x |
| fresh_each_iter | 1.03 ms | 1.30x |

#### alloc/vec_100k

| 実装 | 平均 | 最速比 |
|---|---|---|
| collect | 25.15 µs | 1.00x |
| push_no_capacity | 103.88 µs | 4.13x |
| push_with_capacity | 107.57 µs | 4.28x |

#### channel/1M_msgs

| 実装 | 平均 | 最速比 |
|---|---|---|
| std_batched_256 | 11.28 ms | 1.00x |
| std_bounded_1024 | 25.52 ms | 2.26x |
| crossbeam_bounded_1024 | 34.16 ms | 3.03x |
| std_unbounded | 54.52 ms | 4.83x |

#### cpu/primes_2M

| 実装 | 平均 | 最速比 |
|---|---|---|
| rayon | 51.08 ms | 1.00x |
| scoped_equal_split | 67.38 ms | 1.32x |
| scoped_strided | 102.38 ms | 2.00x |
| sequential | 196.23 ms | 3.84x |

#### dispatch/100k

| 実装 | 平均 | 最速比 |
|---|---|---|
| generic_single_type | 72.03 µs | 1.00x |
| enum_mixed | 94.03 µs | 1.31x |
| box_dyn_mixed | 548.33 µs | 7.61x |

#### errors/all_err_10k

| 実装 | 平均 | 最速比 |
|---|---|---|
| thiserror_enum | 77.14 µs | 1.00x |
| anyhow | 716.00 µs | 9.28x |

#### errors/all_ok_10k

| 実装 | 平均 | 最速比 |
|---|---|---|
| anyhow | 59.28 µs | 1.00x |
| thiserror_enum | 64.87 µs | 1.09x |

#### hash/string_50k

| 実装 | 平均 | 最速比 |
|---|---|---|
| fxhash | 6.85 ms | 1.00x |
| ahash | 7.54 ms | 1.10x |
| std_siphash | 9.29 ms | 1.36x |

#### hash/u64_100k

| 実装 | 平均 | 最速比 |
|---|---|---|
| fxhash | 2.17 ms | 1.00x |
| ahash | 2.77 ms | 1.28x |
| std_siphash_with_capacity | 4.24 ms | 1.96x |
| std_siphash | 7.07 ms | 3.26x |

#### iter/dot_f32_64k

| 実装 | 平均 | 最速比 |
|---|---|---|
| chunks8_multi_acc | 8.24 µs | 1.00x |
| index | 46.79 µs | 5.68x |
| zip | 47.38 µs | 5.75x |

#### iter/sum_u64_64k

| 実装 | 平均 | 最速比 |
|---|---|---|
| index | 8.12 µs | 1.00x |
| iter_fold | 8.38 µs | 1.03x |

#### layout/sum_mass_1M

| 実装 | 平均 | 最速比 |
|---|---|---|
| soa | 723.33 µs | 1.00x |
| aos_64B_struct | 2.62 ms | 3.63x |

#### shared_counter/4threads_x200k

| 実装 | 平均 | 最速比 |
|---|---|---|
| local_then_merge | 417.14 µs | 1.00x |
| per_thread_atomic_padded | 1.72 ms | 4.12x |
| per_thread_atomic_false_sharing | 15.27 ms | 36.62x |
| single_atomic | 15.91 ms | 38.14x |
| mutex | 99.62 ms | 238.82x |

#### small/tokens_10k_lines

| 実装 | 平均 | 最速比 |
|---|---|---|
| smallvec_8 | 1.02 ms | 1.00x |
| vec | 1.76 ms | 1.72x |

#### string/build_2k

| 実装 | 平均 | 最速比 |
|---|---|---|
| write_with_capacity | 44.23 µs | 1.00x |
| push_str_to_string | 45.12 µs | 1.02x |
| format_concat | 354.84 µs | 8.02x |

#### tiny_work/sum_1000

| 実装 | 平均 | 最速比 |
|---|---|---|
| sequential | 115.4 ns | 1.00x |
| rayon_min_len_64k | 119.9 ns | 1.04x |
| rayon | 25.44 µs | 220.41x |

#### tiny_work/sum_100000

| 実装 | 平均 | 最速比 |
|---|---|---|
| sequential | 12.61 µs | 1.00x |
| rayon_min_len_64k | 12.87 µs | 1.02x |
| rayon | 37.82 µs | 3.00x |

#### tiny_work/sum_10000000

| 実装 | 平均 | 最速比 |
|---|---|---|
| rayon | 1.02 ms | 1.00x |
| rayon_min_len_64k | 1.04 ms | 1.02x |
| sequential | 3.48 ms | 3.40x |
