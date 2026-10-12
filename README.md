# rust-bestpractice

Rust の **性能・保守性・ビルド・可用性・並列** のベストプラクティスを、
「言われているから」ではなく **このリポジトリで測った結果** から決めるための実験集。

- 結論（ベストプラクティス一覧）: [docs/BEST_PRACTICES.md](docs/BEST_PRACTICES.md)
- 生の計測結果: [docs/results/](docs/results/)

## 構成

| 場所 | 中身 | 測るもの |
|---|---|---|
| `crates/perf` | 単一スレッドの書き方の比較（criterion） | 確保、文字列、ハッシュ、動的ディスパッチ、イテレータ、データ配置、エラー型、SmallVec |
| `crates/parallel` | 並列化の比較（criterion + `tokio_cpu`） | 逐次 / scoped thread / rayon、共有カウンタと false sharing、チャネル、async 上の CPU 処理 |
| `crates/availability` | 障害の再現（tokio の仮想時間で決定的に） | タイムアウトとリトライ、過負荷と負荷制限、背圧、パニック隔離、グレースフルシャットダウン |
| `crates/buildexp` | ビルド設定の実験台（serde + tokio + rayon の小さな CLI） | プロファイル・リンカごとのビルド時間、再ビルド時間、サイズ、実行時間 |
| `Cargo.toml` | 実験から決めたプロファイルと lint を一元管理 | 保守性 |

各実験は「素朴な版」と「推奨する版」を並べ、**テストで両者が同じ結果を返すことを確かめてから** 速さを比べる。

## 実行

```sh
cargo test --workspace                              # 実装どうしの結果が一致するか
cargo run --release -p availability                 # 可用性（数秒。仮想時間なので速い）
cargo run --release -p parallel --bin tokio_cpu     # async と CPU 処理
cargo bench -p perf                                 # 単一スレッド性能
cargo bench -p parallel                             # 並列
scripts/build_matrix.sh                             # ビルド設定（数分〜十数分）
scripts/run_all.sh                                  # 全部を実行して docs/results/ を更新
```

## 計測環境（docs/results の数字）

- Intel Xeon 2.1GHz、4 コア（クラウドの VM）、メモリ 15GB、Linux 6.18
- rustc 1.97.0（`rust-toolchain.toml` で固定）
- VM なので、数 % の差はノイズとみなす。結論には **2 倍以上など大きな差** だけを使う。
