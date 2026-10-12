| 設定 | クリーンビルド | 再ビルド（1 行変更） | サイズ | 実行時間（最小 / 5 回） |
|---|---|---|---|---|
| dev: cargo 既定（deps も opt0, debug=full） | 10.6 s | 0.3 s | 21068 KiB | 3067 ms |
| dev: このリポジトリ（deps opt1, line-tables-only） | 22.4 s | 0.3 s | 9820 KiB | 1656 ms |
| dev: + GNU ld（rust-lld を使わない） | 23.4 s | 0.5 s | 9684 KiB | 1624 ms |
| release: cargo 既定（lto=false, cgu=16） | 13.1 s | 0.9 s | 1132 KiB | 343 ms |
| release: このリポジトリ（thin LTO） | 14.8 s | 4.8 s | 1072 KiB | 336 ms |
| release: thin LTO + target-cpu=native | 15.7 s | 4.8 s | 1072 KiB | 322 ms |
| dist: fat LTO, cgu=1, panic=abort, strip | 19.0 s | 6.9 s | 540 KiB | 315 ms |
