#!/usr/bin/env python3
"""target/criterion の estimates.json を集め、グループごとに Markdown の表にする（最速比つき）。"""
import json, sys, pathlib
from collections import defaultdict

root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "target/criterion")
groups = defaultdict(list)
for est in root.glob("**/new/estimates.json"):
    bench = json.loads((est.parent / "benchmark.json").read_text())
    mean = json.loads(est.read_text())["mean"]["point_estimate"]
    groups[bench["group_id"]].append((bench["function_id"] or bench["value_str"], mean))

def fmt(ns):
    for unit, div in (("s", 1e9), ("ms", 1e6), ("µs", 1e3)):
        if ns >= div:
            return f"{ns/div:.2f} {unit}"
    return f"{ns:.1f} ns"

for g in sorted(groups):
    rows = sorted(groups[g], key=lambda r: r[1])
    best = rows[0][1]
    print(f"\n#### {g}\n\n| 実装 | 平均 | 最速比 |\n|---|---|---|")
    for name, ns in rows:
        print(f"| {name} | {fmt(ns)} | {ns/best:.2f}x |")
