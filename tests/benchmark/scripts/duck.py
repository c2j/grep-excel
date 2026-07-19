#!/usr/bin/env python3
"""DuckDB 基准: python3 duck.py <csv_search|csv_agg|xlsx_search|join>
首次运行会自动 INSTALL excel 扩展（读 xlsx 需要）。"""
import sys, os, duckdb

BASE = os.environ.get("BENCH_DIR", "/tmp/bench")
mode = sys.argv[1]
con = duckdb.connect()
if mode == "csv_search":
    n = con.execute(f"SELECT COUNT(*) FROM read_csv_auto('{BASE}/bench.csv', header=true) WHERE Note LIKE '%ZXQ-7734%'").fetchone()[0]
elif mode == "csv_agg":
    n = con.execute(f"SELECT Dept, COUNT(*), AVG(Amount) FROM read_csv_auto('{BASE}/bench.csv', header=true) GROUP BY Dept ORDER BY 2 DESC").fetchall()
elif mode == "xlsx_search":
    con.execute("INSTALL excel; LOAD excel")
    n = con.execute(f"SELECT COUNT(*) FROM read_xlsx('{BASE}/bench.xlsx') WHERE Note LIKE '%ZXQ-7734%'").fetchone()[0]
elif mode == "join":
    con.execute("INSTALL excel; LOAD excel")
    n = con.execute(f"""SELECT d.DeptName, COUNT(*) FROM read_csv_auto('{BASE}/bench.csv', header=true) b
                       JOIN read_xlsx('{BASE}/depts.xlsx') d ON b.Dept = d.Dept GROUP BY d.DeptName""").fetchall()
print(n)
