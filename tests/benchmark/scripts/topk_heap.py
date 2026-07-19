#!/usr/bin/env python3
"""Top-K 基准：Python heapq 流式实现（基线）。
用法: python3 topk_heap.py <csv_file> <K>
复杂度: O(n log K) 时间, O(K) 空间。
流式读取，单遍扫描，内存恒定。"""
import sys, os, csv, heapq

BASE = os.environ.get("BENCH_DIR", "/tmp/bench")
csv_file = sys.argv[1] if len(sys.argv) > 1 else f"{BASE}/bench_topk.csv"
K = int(sys.argv[2]) if len(sys.argv) > 2 else 100

heap = []
with open(csv_file, "r") as f:
    reader = csv.reader(f)
    header = next(reader, None)  # skip header
    for row in reader:
        val = float(row[0])
        if len(heap) < K:
            heapq.heappush(heap, val)
        elif val > heap[0]:
            heapq.heapreplace(heap, val)

topk = sorted(heap, reverse=True)
for v in topk:
    print(v)
