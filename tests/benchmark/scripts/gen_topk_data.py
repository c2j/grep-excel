#!/usr/bin/env python3
"""生成 Top-K 基准测试数据：N 行随机浮点数。
用法: BENCH_DIR=/path N=10000000 python3 gen_topk_data.py
默认: BENCH_DIR=/tmp/bench, N=10_000_000 (一千万行约 100MB)

数据特征: 每行一个浮点数，范围 [0, 1)，均匀分布。
为便于正确性校验，在随机位置插入 K 个已知标记值 (999.0 + i)，
确保各工具返回的最大 K 个值可交叉验证。"""
import os, sys, random

BASE = os.environ.get("BENCH_DIR", "/tmp/bench")
N = int(os.environ.get("N", "10_000_000"))
K = 100  # top-K 取值数
os.makedirs(BASE, exist_ok=True)

random.seed(12345)

# 生成标记最大值: 999.0, 999.1, ..., 999.(K-1) 共 K 个
markers = [999.0 + i / (K * 10) for i in range(K)]
# 在数据中随机撒入 K 个标记位置（保证至少 K 个位置有标记值）
marker_positions = set(random.sample(range(N), K))

outpath = f"{BASE}/bench_topk.csv"
with open(outpath, "w") as f:
    f.write("val\n")  # header
    for i in range(N):
        if i in marker_positions:
            val = markers.pop()
        else:
            val = random.random()
        f.write(f"{val}\n")

# 输出期望的最大值列表（标记值从大到小排序）
expected = sorted([999.0 + i / (K * 10) for i in range(K)], reverse=True)
print(f"Generated {N} rows to {outpath}")
print(f"Expected top {K} values (first 5): {expected[:5]}")
print(f"Expected top {K} values (last 5):  {expected[-5:]}")
print(f"File size: {os.path.getsize(outpath) / 1024 / 1024:.1f} MB")
