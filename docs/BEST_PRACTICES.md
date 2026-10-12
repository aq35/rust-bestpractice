# Rust ベストプラクティス（実験から得たもの）

各項目には、根拠になった計測を書く。数字は [results/](results/) の生データから取った。
環境は 4 コアの VM（README を参照）。**2 倍以上の差だけを「効く」とみなし**、それより小さい差は「好みで選んでよい」とする。

「推測」と書いたものは、理由をまだ計測で確かめていない。

---

## 1. 性能（単一スレッド）

| # | やること | 計測 | 差 |
|---|---|---|---|
| P1 | 文字列は `format!` で繋ぎ直さない。`write!` か `push_str` で 1 つのバッファに足す | `string/build_2k` | **8.0 倍** |
| P2 | 型が決まっているならジェネリクス、種類が限られるなら `enum`。`Box<dyn Trait>` は最後の手段 | `dispatch/100k`: generic 72µs / enum 94µs / dyn 548µs | **dyn は 7.6 倍遅い** |
| P3 | 失敗が頻繁に起きる経路（パース、検証）では型付きエラー（`thiserror` の enum）を返す。`anyhow` は上位層（アプリの main、CLI）で使う | `errors/all_err_10k`: enum 77µs / anyhow 732µs | **9.5 倍**。`RUST_BACKTRACE=1` だと **586 倍** |
| P4 | 成功の経路では anyhow と enum の差はない。使い分けは失敗の経路と API の都合で決める | `errors/all_ok_10k` | 差なし |
| P5 | 1 つのフィールドだけを大量に走査するなら、構造体の配列（AoS）ではなく配列の構造体（SoA）にする | `layout/sum_mass_1M`: SoA 0.72ms / AoS 2.62ms | **3.6 倍** |
| P6 | 整数キーの `HashMap` で、外部からの入力をキーにしない（HashDoS の心配がない）なら `FxHashMap`。外部入力なら `ahash` か標準のまま | `hash/u64_100k`: fx 2.2ms / ahash 2.8ms / std 7.1ms | **3.3 倍** |
| P7 | 標準の `HashMap` を使うときも、件数が分かるなら `with_capacity` | `hash/u64_100k`: std 7.1ms → 4.2ms | **1.7 倍** |
| P8 | `Vec` は `push` のループより、イテレータの `collect` で作る | `alloc/vec_100k`: collect 25µs / push 104µs | **4.1 倍** |
| P9 | 浮動小数の総和や内積を速くしたいなら、独立したアキュムレータを複数持つ（`chunks_exact(8)`）。コンパイラは加算の順番を変えられないので、自動では SIMD にならない | `iter/dot_f32_64k`: 8.2µs / 47µs | **5.7 倍** |
| P10 | 要素数が小さいと分かっている一時的なリストは `SmallVec` | `small/tokens_10k_lines` | 1.7 倍 |
| P11 | ループの中で毎回作るバッファは、外に出して `clear()` して使い回す | `alloc/buffer_10k_lines` | 1.3 倍 |

**効かなかったこと（気にしなくてよい）**

- 添字ループ（`v[i]`）とイテレータ（`iter().fold`）の差: 整数の総和は 1.03 倍、内積も同じ。
  境界チェックは最適化で消える。読みやすさで選んでよい。
- `push` のループに `with_capacity` を足しても速くならなかった（104µs → 108µs）。
  P8 の差は再確保ではなく、`collect` ではループが単純になって SIMD 化されたためと推測する。
- `write!` と `push_str(&x.to_string())` の差は 1.02 倍。

## 2. 並列

| # | やること | 計測 | 差 |
|---|---|---|---|
| C1 | CPU を使う処理のデータ並列は、まず `rayon`。自分でスレッドに分けない | `cpu/primes_2M`: rayon 51ms / 手で等分割 67ms / 逐次 196ms | 逐次比 **3.8 倍**（4 コア）、手分け比 1.3 倍 |
| C2 | 手で分けるなら仕事量の偏りに注意する。ストライド（交互）分割は、偶数だけを受け持つスレッドが一瞬で終わり、**手分けの中で最悪** になった | `cpu/primes_2M/scoped_strided` 102ms | 等分割より 1.5 倍遅い |
| C3 | 1 要素の仕事が軽いなら並列にしない。するなら `with_min_len` で塊を大きくする | `tiny_work/sum_1000`: rayon 25µs / 逐次 0.1µs | **220 倍遅くなる**。1000 万要素で初めて rayon が 3.4 倍速い |
| C4 | 共有カウンタは、スレッドごとにローカル変数で数えて最後に合算する | `shared_counter`: ローカル 0.42ms / Mutex 99.6ms | **Mutex 比 239 倍** |
| C5 | スレッドごとに別の Atomic を持つなら、`#[repr(align(128))]` でキャッシュラインを分ける。分けないと false sharing で 1 つの Atomic を取り合うのと同じ遅さになる | 隣接 15.3ms / パディング 1.7ms / 単一 Atomic 15.9ms | **9 倍** |
| C6 | チャネルは 1 件ずつ送らず、`Vec` でまとめて送る | `channel/1M_msgs`: まとめて 11ms / 1 件ずつ（有界）26ms | 2.3 倍 |
| C7 | チャネルは有界（`sync_channel` / `mpsc::channel(n)`）を使う。速さでも無制限より良かった。可用性の理由は A3 | std 無制限 55ms / 有界 26ms | 2.1 倍 |
| C8 | `crossbeam-channel` を、速さのためだけに入れない。std の mpsc は 1.67 から crossbeam の実装を取り込んでいて、今回は std の方が速かった | crossbeam 34ms / std 26ms | — |
| C9 | async（tokio）の中で重い CPU 処理を直接しない。`rayon::spawn` + `oneshot`、または `spawn_blocking` に逃がす | `tokio_cpu`: 他タスクの最大遅延 直接 170ms / spawn_blocking 5.6ms / rayon 1.4ms | **120 倍** |

## 3. 可用性

数字は tokio の止めた時計（`start_paused`）を使った仮想時間で、毎回同じになる（[results/availability.md](results/availability.md)）。

| # | やること | 計測 |
|---|---|---|
| A1 | 外部呼び出しには必ずタイムアウトを付ける。ただしタイムアウトだけでは成功率が下がる | 上流の 10% がハングする条件で、なし: p99 **30 秒** / タイムアウト 100ms のみ: p99 100ms だが成功率 81.6% → 73.7% |
| A2 | タイムアウト + 指数バックオフ（full jitter）でリトライし、**全体の期限** も設ける | リトライ 5 回 + 期限 300ms: 成功率 **99.6%**、最大 300ms で頭打ち |
| A3 | キューは有界にして、背圧を生産者に伝える | 生産が消費の 2 倍のとき、無制限: キュー長 1000・待ち p99 **1979ms** / 有界 64: キュー長 64・待ち p99 128ms |
| A4 | 処理能力を超える要求には、同時実行数を `Semaphore` で制限し、空きがなければ **すぐ断る**（load shedding）。待たせるだけでは遅延は減りきらない | 容量の 1.25 倍の負荷で、無制限: p99 793ms・同時 737 / 待たせる: p99 504ms / 即拒否: 2 割を断り、受けたものは p99 **10ms** |
| A5 | タスクのパニックは `JoinSet` / `JoinHandle` の `JoinError::is_panic()` で拾う。ランタイムと他のタスクは止まらない | 100 タスク中 10 がパニック → 残り 90 完了、ランタイムは生きている |
| A6 | `std::sync::Mutex` のロックは `unwrap()` しない。パニック後は毒状態になるので `unwrap_or_else(PoisonError::into_inner)` で中身を取り出せるようにする | 毒状態になったあとも値を回収できた |
| A7 | 止めるときは、新規の受付を止め、処理中のものを期限付きで待ってから残りを止める（graceful shutdown） | 即 abort: 100 件中 **28 件が途中で中断** / 受付停止 + 最大 1 秒待つ: 中断 0、停止に 228ms |
| A8 | `panic = "abort"` は配布用プロファイルだけにする。abort にすると A5 の隔離が効かず、プロセスごと落ちる | （設計上の理由。計測はしていない） |

## 4. ビルド

`crates/buildexp`（serde + tokio + rayon）で測った（[results/build.md](results/build.md)）。

| # | やること | 計測 |
|---|---|---|
| B1 | 開発用は `[profile.dev.package."*"] opt-level = 1`（依存だけ最適化）と `debug = "line-tables-only"` | 初回のクリーンビルドは 10.6s → 22.4s に増えるが、**dev での実行が 1.9 倍速く**、バイナリは 21MB → 9.8MB。依存は一度ビルドすればキャッシュされ、1 行変更の再ビルドは 0.3s のまま |
| B2 | `release` に thin LTO を入れない。入れるのは配布用プロファイルだけ | thin LTO で実行は約 2% しか速くならず、**1 行変更の再ビルドが 0.9s → 4.8s** |
| B3 | 配布用には `dist` プロファイル（fat LTO、`codegen-units=1`、`panic="abort"`、`strip`）を別に作る | サイズ 1132KB → **540KB**、実行は 8% 速い。再ビルドは 6.9s |
| B4 | リンカは rustc 1.90 以降の既定（rust-lld）のままでよい | GNU ld に戻すと再ビルドが 0.3s → 0.5s |
| B5 | `target-cpu=native` は配る先の CPU が決まっているときだけ | 実行 4% 速いだけ。別の CPU では動かない恐れがある |
| B6 | ツールチェーンを `rust-toolchain.toml` で固定し、`Cargo.lock` をコミットする | 計測の再現性のため（計測はしていない） |

## 5. 保守性

計測ではなく、このリポジトリで実際に使ってみて決めたこと。

| # | やること | 理由 |
|---|---|---|
| M1 | lint はワークスペースの `[workspace.lints]` で一元管理し、各 crate は `[lints] workspace = true` で継承する | crate ごとの設定のずれがなくなる |
| M2 | `clippy::pedantic` を warn にし、合わないものだけ理由を書いて `allow` する | 今回 pedantic が見つけたもの: ドキュメントの書き方、`if let` で書ける `match`、使われない `pub` フィールドなど 12 件 |
| M3 | `unsafe_code = "forbid"`。並列の実験も unsafe なしで書けた（`thread::scope` と `repr(align)`） | 安全でないコードを入れるかどうかを、レビューで必ず話し合うようにするため |
| M4 | 性能のための書き換えは「素朴な版」を残し、**テストで両者の結果が一致することを確かめてから** 比べる | 今回、全実験にこの一致テストを付けた |
| M5 | ベンチマークの前に環境変数（`RUST_BACKTRACE` など）を確認する | 今回、`RUST_BACKTRACE=1` のせいで anyhow の結果が 60 倍ずれた |
| M6 | 時間に関わるテストは tokio の `start_paused` で仮想時間にする | 30 秒ハングする上流を 1000 回呼ぶ実験が、数秒で毎回同じ結果になった |
| M7 | criterion を使う crate は `[lib] bench = false` にする | そうしないと `cargo bench -- --noplot` が lib の標準 harness に渡って失敗する（今回実際に起きた） |

---

## まだ試していないこと

- PGO（プロファイルを使った最適化）、BOLT
- グローバルアロケータの交換（mimalloc、jemalloc）
- `sccache` などのビルドキャッシュ、`cargo-nextest`
- コア数が多いマシン（16 コア以上）での並列の伸び方
- 本物のネットワーク越しの可用性（今回は仮想時間のシミュレーション）
