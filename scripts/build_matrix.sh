#!/usr/bin/env bash
# ビルド設定ごとに「クリーンビルド時間 / 1 行変えた後の再ビルド時間 / バイナリサイズ / 実行時間」を測る。
# 使い方: scripts/build_matrix.sh > docs/results/build.md
set -euo pipefail
cd "$(dirname "$0")/.."
SRC=crates/buildexp/src/main.rs
RUNS=${RUNS:-5}

now() { date +%s.%N; }
secs() { awk -v a="$1" -v b="$2" 'BEGIN{printf "%.1f", b-a}'; }

# name | cargo の引数 | RUSTFLAGS
CONFIGS=(
  "dev: cargo 既定（deps も opt0, debug=full）|--profile dev --config profile.dev.debug=true --config 'profile.dev.package.\"*\".opt-level=0'|"
  "dev: このリポジトリ（deps opt1, line-tables-only）|--profile dev|"
  "dev: + GNU ld（rust-lld を使わない）|--profile dev|-C linker-features=-lld"
  "release: cargo 既定（lto=false, cgu=16）|--release --config profile.release.lto=false|"
  "release: このリポジトリ（thin LTO）|--release|"
  "release: thin LTO + target-cpu=native|--release|-C target-cpu=native"
  "dist: fat LTO, cgu=1, panic=abort, strip|--profile dist|"
)

mkdir -p target; : > target/matrix.log
echo "| 設定 | クリーンビルド | 再ビルド（1 行変更） | サイズ | 実行時間（最小 / ${RUNS} 回） |"
echo "|---|---|---|---|---|"
for c in "${CONFIGS[@]}"; do
  IFS='|' read -r name args flags <<<"$c"
  tdir="target/matrix/$(echo "$name" | md5sum | cut -c1-8)"
  rm -rf "$tdir"
  export CARGO_TARGET_DIR="$tdir" RUSTFLAGS="$flags"
  # 依存のダウンロード時間を除くため先に fetch 済みであること
  t0=$(now); eval cargo build -q -p buildexp $args 2>>target/matrix.log; t1=$(now)
  cp "$SRC" "$SRC.bak"; echo "// touch $(date +%s%N)" >>"$SRC"
  t2=$(now); eval cargo build -q -p buildexp $args 2>>target/matrix.log; t3=$(now)
  mv "$SRC.bak" "$SRC"
  bin=$(find "$tdir" -maxdepth 2 -type f -name buildexp -perm -u+x | head -1)
  size=$(du -k "$bin" | cut -f1)
  best=999
  for _ in $(seq "$RUNS"); do
    s=$(now); "$bin" 200000 >/dev/null; e=$(now)
    best=$(awk -v a="$s" -v b="$e" -v m="$best" 'BEGIN{d=b-a; print (d<m)?d:m}')
  done
  printf '| %s | %s s | %s s | %s KiB | %.0f ms |\n' "$name" "$(secs "$t0" "$t1")" "$(secs "$t2" "$t3")" "$size" "$(awk -v d="$best" 'BEGIN{print d*1000}')"
  rm -rf "$tdir"
done
