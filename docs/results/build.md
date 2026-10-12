# ビルド設定の計測（crates/buildexp、scripts/build_matrix.sh）

## 2 回目（現在の設定名）

| 設定 | クリーンビルド | 再ビルド（1 行変更） | サイズ | 実行時間（最小 / 5 回） |
|---|---|---|---|---|
| dev: cargo 既定（deps も opt0, debug=full） | 10.8 s | 0.3 s | 21084 KiB | 3026 ms |
| dev: このリポジトリ（deps opt1, line-tables-only） | 21.5 s | 0.2 s | 9832 KiB | 1536 ms |
| dev: + GNU ld（rust-lld を使わない） | 23.4 s | 0.5 s | 9692 KiB | 1505 ms |
| release: cargo 既定（このリポジトリも同じ） | 12.4 s | 0.8 s | 1120 KiB | 327 ms |
| release: + thin LTO | 14.9 s | 4.4 s | 1056 KiB | 313 ms |
| release: + target-cpu=native | 13.8 s | 0.9 s | 1120 KiB | 337 ms |
| dist: fat LTO, cgu=1, panic=abort, strip | 19.0 s | 7.1 s | 540 KiB | 302 ms |

## 1 回目（release の行は当時の設定名。「このリポジトリ」は当時の thin LTO、native は thin LTO との組み合わせ）

| 設定 | クリーンビルド | 再ビルド（1 行変更） | サイズ | 実行時間（最小 / 5 回） |
|---|---|---|---|---|
| dev: cargo 既定（deps も opt0, debug=full） | 10.6 s | 0.3 s | 21068 KiB | 3067 ms |
| dev: このリポジトリ（deps opt1, line-tables-only） | 22.4 s | 0.3 s | 9820 KiB | 1656 ms |
| dev: + GNU ld（rust-lld を使わない） | 23.4 s | 0.5 s | 9684 KiB | 1624 ms |
| release: cargo 既定（lto=false, cgu=16） | 13.1 s | 0.9 s | 1132 KiB | 343 ms |
| release: このリポジトリ（thin LTO） | 14.8 s | 4.8 s | 1072 KiB | 336 ms |
| release: thin LTO + target-cpu=native | 15.7 s | 4.8 s | 1072 KiB | 322 ms |
| dist: fat LTO, cgu=1, panic=abort, strip | 19.0 s | 6.9 s | 540 KiB | 315 ms |
