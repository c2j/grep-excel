#!/usr/bin/env python3
"""Top-K 基准：DuckDB Python 驱动。
用法: python3 topk_duck.py <csv_file> <K>
DuckDB 的 ORDER BY ... LIMIT 内部使用 Top-N 优化（非全量排序）。"""
import sys, os, duckdb

BASE = os.environ.get("BENCH_DIR", "/tmp/bench")
csv_file = sys.argv[1] if len(sys.argv) > 1 else f"{BASE}/bench_topk.csv"
K = int(sys.argv[2]) if len(sys.argv) > 2 else 100

con = duckdb.connect()
rows = con.execute(f"""
    SELECT * FROM read_csv_auto('{csv_file}', header=true)
    ORDER BY val DESC LIMIT {K}
""").fetchall()
for r in rows:
    print(r[0])
