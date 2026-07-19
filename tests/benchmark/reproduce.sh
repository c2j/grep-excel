#!/bin/bash
# grep-excel 性能测评复现脚本
# 支持 Linux x86_64 和 macOS arm64
# 用法: bash reproduce.sh [linux|macos|all]

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BENCH_DIR="${BENCH_DIR:-/tmp/bench}"

echo "=== grep-excel Benchmark Reproduction ==="
echo "BENCH_DIR: $BENCH_DIR"
echo "Script dir: $SCRIPT_DIR"
echo ""

# Step 1: Setup bench directory
mkdir -p "$BENCH_DIR"

# Step 2: Copy scripts
cp "$SCRIPT_DIR/scripts/gen_data.py" "$BENCH_DIR/"
cp "$SCRIPT_DIR/scripts/bench.py" "$BENCH_DIR/"
cp "$SCRIPT_DIR/scripts/duck.py" "$BENCH_DIR/"
cp "$SCRIPT_DIR/scripts/gen_topk_data.py" "$BENCH_DIR/"

# Step 3: Locate or build grep_excel
if [ -f "$SCRIPT_DIR/../../target/release/grep_excel" ]; then
    cp "$SCRIPT_DIR/../../target/release/grep_excel" "$BENCH_DIR/"
    echo "Using workspace build grep_excel"
elif command -v grep_excel &>/dev/null; then
    cp "$(which grep_excel)" "$BENCH_DIR/"
    echo "Using system grep_excel"
else
    echo "ERROR: grep_excel not found. Build with: cargo build -p grep-excel --features full --release"
    exit 1
fi

# Step 4: Generate test data
echo ""
echo "--- Generating test data ---"
cd "$BENCH_DIR" && python3 gen_data.py

# Step 5: Verify MD5 fingerprint
echo ""
echo "--- MD5 fingerprint ---"
if command -v md5sum &>/dev/null; then
    md5sum bench.csv
elif command -v md5 &>/dev/null; then
    md5 -q bench.csv && echo "  bench.csv"
else
    echo "WARNING: No md5 tool found"
fi

# Step 6: Correctness verification
echo ""
echo "--- Correctness checks (expected: 285) ---"
echo -n "grep -c ZXQ-7734: "; grep -c "ZXQ-7734" bench.csv
echo -n "grep-excel CSV:  "; ./grep_excel bench.csv -q ZXQ-7734 -f simple 2>/dev/null | grep -c ZXQ-7734
echo -n "grep-excel XLSX: "; ./grep_excel bench.xlsx -q ZXQ-7734 -f simple 2>/dev/null | grep -c ZXQ-7734
echo -n "DuckDB CSV:     "; python3 duck.py csv_search
echo -n "DuckDB XLSX:    "; python3 duck.py xlsx_search

# Step 7: Run benchmarks (tools that are available)
# Substitute BENCH_DIR into job JSONs so they work when BENCH_DIR is overridden
_bench_jobs() {
    local json_file="$1"; shift
    sed "s|/tmp/bench|$BENCH_DIR|g" "$json_file"
}

echo ""
echo "--- Group A: xlsx Fulltext Search ---"
python3 bench.py "$(_bench_jobs "$SCRIPT_DIR/scripts/jobsA.json")" 600 2>/dev/null || echo "  (some tools unavailable - continuing)"

echo ""
echo "--- Group B: CSV Fulltext Search ---"
python3 bench.py "$(_bench_jobs "$SCRIPT_DIR/scripts/jobsB.json")" 300 2>/dev/null || echo "  (some tools unavailable - continuing)"

echo ""
echo "--- Group C: CSV SQL Aggregation ---"
python3 bench.py "$(_bench_jobs "$SCRIPT_DIR/scripts/jobsC.json")" 300 2>/dev/null || echo "  (some tools unavailable - continuing)"

echo ""
echo "--- Group D: Cross-file JOIN ---"
python3 bench.py "$(_bench_jobs "$SCRIPT_DIR/scripts/jobsD.json")" 300 2>/dev/null || echo "  (some tools unavailable - continuing)"

# Step 8: Top-K (optional, requires DuckDB engine grep-excel and large disk space)
echo ""
echo "--- Group E: Top-K (50M rows, requires ~1GB disk) ---"
if [ "${SKIP_TOPK:-0}" = "1" ]; then
    echo "  SKIP_TOPK=1, skipping"
else
    echo "Generating Top-K data (set N= env var to change row count, default 1M for quick test)..."
    N="${N:-1000000}" python3 "$BENCH_DIR/gen_topk_data.py"
    cp "$SCRIPT_DIR/scripts/topk_heap.py" "$BENCH_DIR/"
    cp "$SCRIPT_DIR/scripts/topk_duck.py" "$BENCH_DIR/"
    echo "  Correctness check..."
    python3 topk_heap.py bench_topk.csv 100 2>/dev/null | head -3
    python3 bench.py "$(_bench_jobs "$SCRIPT_DIR/scripts/jobsE.json")" 600 2>/dev/null || echo "  (some tools unavailable - continuing)"
fi

echo ""
echo "=== Benchmark complete ==="
echo "Results saved to stdout above. Redirect with: bash reproduce.sh > results.txt"
