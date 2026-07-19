# grep-excel 性能测评脚本包（可复现验证）

配合《grep-excel 竞品对比》文档使用。全部脚本开源透明，可在 Linux x86_64 / macOS arm64 环境复现。测试日期 2026-07-19。

## 1. 复现流程总览

```bash
# ① 约定工作目录（脚本默认 /tmp/bench，可用 BENCH_DIR 覆盖）
export BENCH_DIR=/tmp/bench && mkdir -p $BENCH_DIR && cd $BENCH_DIR

# ② 按第 3 节安装各工具，并把脚本保存到该目录
# ③ 生成测试数据（约 1-2 分钟）
python3 gen_data.py

# ④ 正确性校验：各工具命中数应全部 = 285（见第 5 节）

# ⑤ 依次运行四组基准
python3 bench.py "$(cat jobsA.json)" 600   # A: xlsx 全文搜索（Python 系较慢）
python3 bench.py "$(cat jobsB.json)" 300   # B: CSV 全文搜索
python3 bench.py "$(cat jobsC.json)" 300   # C: CSV SQL 聚合
python3 bench.py "$(cat jobsD.json)" 300   # D: 跨文件 JOIN
```

## 2. 测试数据

合成数据：200,000 行 × 10 列（ID/Name/Dept/City/Level/Amount/Date/Status/Note/TraceCode），中英文混合；检索词 `ZXQ-7734` 命中 **285 行**（`i % 701 == 0`）。

- `bench.csv` ≈ 16.9 MB（200,001 行含表头），随机种子固定（seed=42），**任何机器上重新生成的 CSV 逐字节一致**，MD5 应为：

```
b3fd130f786c908ecaabd9939de3ad79  bench.csv
```

- `bench.xlsx` ≈ 13.0 MB（同数据，zip 打包时间戳会导致 MD5 不同，属正常）
- `depts.xlsx`：10 行部门维表，用于 JOIN 测试

## 3. 工具安装清单（版本锁定）

| 工具 | 版本 | 安装方式 |
|------|------|----------|
| grep-excel | v0.7.1 | `curl -L -o gx.zip https://github.com/c2j/grep-excel/releases/download/v0.7.1/grep_excel-duckdb-x86_64-unknown-linux-gnu-v0.7.1.zip && unzip gx.zip`（macOS：`cargo build -p grep-excel --features full --release` 源码构建） |
| xlsxgrep | 0.0.32 | `pip install xlsxgrep==0.0.32` |
| xgrep | 0.2.12 | `pip install xgrep==0.2.12`（底层为 python-calamine） |
| duckdb | 1.5.4 | `pip install duckdb==1.5.4`；xlsx 读取自动 `LOAD excel`（duck.py 已含 `INSTALL excel; LOAD excel`，首次运行自动安装扩展） |
| openpyxl | 3.1.x | `pip install openpyxl`（仅用于生成测试数据） |
| csvq | v1.18.1 | `curl -L https://github.com/mithrandie/csvq/releases/download/v1.18.1/csvq-v1.18.1-linux-amd64.tar.gz` |
| dsq | v0.23.0 | `curl -L https://github.com/multiprocessio/dsq/releases/download/v0.23.0/dsq-linux-x64-v0.23.0.zip` |
| OctoSQL | v0.13.0 | `curl -L https://github.com/cube2222/octosql/releases/download/v0.13.0/octosql_0.13.0_linux_amd64.tar.gz` |
| xsv | 0.13.0 | `curl -L https://github.com/BurntSushi/xsv/releases/download/0.13.0/xsv-0.13.0-x86_64-unknown-linux-musl.tar.gz` |
| Miller | v6.20.2 | `curl -L https://github.com/johnkerl/miller/releases/download/v6.20.2/miller-6.20.2-linux-amd64.tar.gz` |
| trdsql | v1.2.3 | 预编译版要求 glibc 2.38；不满足时源码构建：`go install github.com/noborus/trdsql/cmd/trdsql@v1.2.3` |
| q | 3.1.6 | 参考项：Python 3.12 下安装失败（维护停滞），未纳入计时 |

> 网络受限环境提示：GitHub release 资产直连可能极慢，可经镜像加速（如在下载 URL 前加代理前缀），或改用包管理器/源码构建。

## 4. 脚本

### 4.1 `gen_data.py` — 数据生成

```python
#!/usr/bin/env python3
"""生成基准测试数据：bench.xlsx (200k行) 与 bench.csv（同数据）。
用法: BENCH_DIR=/path python3 gen_data.py  (默认 /tmp/bench)"""
import csv, os, random, datetime
from openpyxl import Workbook

BASE = os.environ.get("BENCH_DIR", "/tmp/bench")
os.makedirs(BASE, exist_ok=True)

random.seed(42)
N = 200_000
DEPTS = ["Engineering", "Sales", "Finance", "HR", "Support", "Marketing", "Legal", "Ops", "Research", "QA"]
CITIES = ["上海", "北京", "深圳", "杭州", "成都", "南京", "武汉", "西安", "苏州", "重庆"]
SURNAMES = ["张", "李", "王", "刘", "陈", "杨", "赵", "黄", "周", "吴"]
GIVEN = ["伟", "芳", "娜", "磊", "静", "强", "洋", "艳", "勇", "军"]
STATUSES = ["正常", "正常", "正常", "正常", "待复核", "ERROR-超时", "ERROR-校验失败"]
NOTES = ["例行巡检通过", "批量导入", "人工录入", "接口同步", "历史迁移数据", "季度结算生成"]
NEEDLE = "ZXQ-7734-异常交易"

header = ["ID", "Name", "Dept", "City", "Level", "Amount", "Date", "Status", "Note", "TraceCode"]
base = datetime.date(2023, 1, 1)

def row(i):
    needle = (i % 701 == 0)  # ~285 行命中
    note = (NEEDLE + "，需人工复核") if needle else random.choice(NOTES)
    return [
        i,
        random.choice(SURNAMES) + random.choice(GIVEN) + str(random.randint(1, 99)),
        random.choice(DEPTS),
        random.choice(CITIES),
        random.randint(1, 15),
        round(random.uniform(100, 99999), 2),
        (base + datetime.timedelta(days=random.randint(0, 900))).strftime("%Y%m%d"),
        random.choice(STATUSES),
        note,
        f"T{random.randint(100000, 999999)}",
    ]

with open(f"{BASE}/bench.csv", "w", newline="", encoding="utf-8") as f:
    w = csv.writer(f)
    w.writerow(header)
    for i in range(1, N + 1):
        w.writerow(row(i))
print("csv done")

random.seed(42)  # 重置以保证与 CSV 完全一致
wb = Workbook(write_only=True)
ws = wb.create_sheet("Sheet1")
ws.append(header)
for i in range(1, N + 1):
    ws.append(row(i))
wb.save(f"{BASE}/bench.xlsx")
print("xlsx done")

# 第二个小文件，用于跨文件 JOIN 演示
wb2 = Workbook(write_only=True)
ws2 = wb2.create_sheet("Sheet1")
ws2.append(["Dept", "DeptName", "Manager"])
for d in DEPTS:
    ws2.append([d, d + "部", random.choice(SURNAMES) + random.choice(GIVEN)])
wb2.save(f"{BASE}/depts.xlsx")
print("depts done")
```

### 4.2 `bench.py` — 通用计时器

```python
#!/usr/bin/env python3
"""基准测试：对指定命令列表计时，stdout 重定向到 /dev/null，取 min/median。
用法: python3 bench.py '<json array of {name, cmd, runs?, timeout?}>' [默认超时秒]"""
import subprocess, time, sys, json, statistics, os

DEVNULL = open(os.devnull, "w")

def run(cmd, runs=3, warmup=1, shell=True, timeout=300):
    times = []
    for i in range(warmup + runs):
        t0 = time.perf_counter()
        try:
            r = subprocess.run(cmd, shell=shell, stdout=DEVNULL, stderr=DEVNULL, timeout=timeout)
            dt = time.perf_counter() - t0
            if r.returncode != 0:
                return {"cmd": cmd, "error": f"exit={r.returncode}"}
        except subprocess.TimeoutExpired:
            return {"cmd": cmd, "error": f"timeout>{timeout}s"}
        if i >= warmup:
            times.append(dt)
    return {"cmd": cmd, "min": round(min(times), 3), "median": round(statistics.median(times), 3)}

if __name__ == "__main__":
    jobs = json.loads(sys.argv[1])
    timeout = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    for j in jobs:
        res = run(j["cmd"], runs=j.get("runs", 3), warmup=j.get("warmup", 1), timeout=j.get("timeout", timeout))
        res["name"] = j["name"]
        if "error" in res:
            print(f'{j["name"]:24s} ERROR {res["error"]}', flush=True)
        else:
            print(f'{j["name"]:24s} min={res["min"]:8.3f}s  median={res["median"]:8.3f}s', flush=True)
```

### 4.3 `duck.py` — DuckDB 基准入口（Python 驱动）

```python
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
```

### 4.4 任务定义 `jobsA.json`（xlsx 全文搜索）

```json
[
  {"name": "grep-excel 0.7.1", "cmd": "cd /tmp/bench && ./grep_excel bench.xlsx -q ZXQ-7734 -f simple", "runs": 3},
  {"name": "duckdb 1.5.4 (read_xlsx)", "cmd": "cd /tmp/bench && python3 duck.py xlsx_search", "runs": 3},
  {"name": "dsq 0.23.0", "cmd": "cd /tmp/bench && ./dsq bench.xlsx \"SELECT * FROM {} WHERE Note LIKE '%ZXQ-7734%'\"", "runs": 3},
  {"name": "xgrep 0.2.12", "cmd": "cd /tmp/bench && xgrep 'ZXQ-7734' bench.xlsx --format csv", "runs": 2, "timeout": 600},
  {"name": "xlsxgrep 0.0.32", "cmd": "cd /tmp/bench && xlsxgrep 'ZXQ-7734' bench.xlsx -F", "runs": 2, "timeout": 600}
]
```

### 4.5 任务定义 `jobsB.json`（CSV 全文搜索）

```json
[
  {"name": "GNU grep (基线)", "cmd": "cd /tmp/bench && grep 'ZXQ-7734' bench.csv", "runs": 3},
  {"name": "grep-excel 0.7.1", "cmd": "cd /tmp/bench && ./grep_excel bench.csv -q ZXQ-7734 -f simple", "runs": 3},
  {"name": "grep-excel -c Note", "cmd": "cd /tmp/bench && ./grep_excel bench.csv -q ZXQ-7734 -c Note -f simple", "runs": 3},
  {"name": "duckdb 1.5.4", "cmd": "cd /tmp/bench && python3 duck.py csv_search", "runs": 3},
  {"name": "xsv 0.13.0", "cmd": "cd /tmp/bench && ./xsv search -s Note 'ZXQ-7734' bench.csv", "runs": 3},
  {"name": "xsv all-columns", "cmd": "cd /tmp/bench && ./xsv search ZXQ-7734 bench.csv", "runs": 3},
  {"name": "miller 6.20.2", "cmd": "cd /tmp/bench && ./mlr --csv filter '$Note =~ \"ZXQ-7734\"' bench.csv", "runs": 3},
  {"name": "csvq 1.18.1", "cmd": "sh /tmp/bench/csvq_search.sh", "runs": 3},
  {"name": "octosql 0.13.0", "cmd": "cd /tmp/bench && OCTOSQL_NO_TELEMETRY=1 ./octosql \"SELECT * FROM bench.csv WHERE \\\"Note\\\" LIKE '%ZXQ-7734%'\" -o stream_native", "runs": 3},
  {"name": "dsq 0.23.0", "cmd": "cd /tmp/bench && ./dsq bench.csv \"SELECT * FROM {} WHERE Note LIKE '%ZXQ-7734%'\"", "runs": 3},
  {"name": "trdsql 1.2.3", "cmd": "cd /tmp/bench && ./trdsql -ih \"SELECT * FROM bench.csv WHERE Note LIKE '%ZXQ-7734%'\"", "runs": 3}
]
```

### 4.6 任务定义 `jobsC.json`（CSV SQL 聚合）

```json
[
  {"name": "grep-excel 0.7.1", "cmd": "cd /tmp/bench && ./grep_excel bench.csv -x \"SELECT Dept, COUNT(*), AVG(Amount) FROM bench.bench GROUP BY Dept ORDER BY 2 DESC\" -f simple", "runs": 3},
  {"name": "duckdb 1.5.4", "cmd": "cd /tmp/bench && python3 duck.py csv_agg", "runs": 3},
  {"name": "csvq 1.18.1", "cmd": "sh /tmp/bench/csvq_agg.sh", "runs": 3},
  {"name": "octosql 0.13.0", "cmd": "cd /tmp/bench && OCTOSQL_NO_TELEMETRY=1 ./octosql \"SELECT Dept, COUNT(*), AVG(Amount) FROM bench.csv GROUP BY Dept ORDER BY 2 DESC\" -o stream_native", "runs": 3},
  {"name": "dsq 0.23.0", "cmd": "cd /tmp/bench && ./dsq bench.csv \"SELECT Dept, COUNT(*), AVG(Amount) FROM {} GROUP BY Dept ORDER BY 2 DESC\"", "runs": 3},
  {"name": "trdsql 1.2.3", "cmd": "cd /tmp/bench && ./trdsql -ih \"SELECT Dept, COUNT(*), AVG(Amount) FROM bench.csv GROUP BY Dept ORDER BY 2 DESC\"", "runs": 3},
  {"name": "miller 6.20.2 (DSL)", "cmd": "cd /tmp/bench && ./mlr --csv stats1 -a count,mean -f Amount -g Dept bench.csv", "runs": 3}
]
```

### 4.7 任务定义 `jobsD.json`（跨文件 JOIN）

```json
[
  {"name": "grep-excel 0.7.1", "cmd": "cd /tmp/bench && ./grep_excel bench.csv depts.xlsx -x \"SELECT d.DeptName, COUNT(*) FROM bench.bench b JOIN depts.Sheet1 d ON b.Dept = d.Dept GROUP BY d.DeptName\" -f simple", "runs": 3},
  {"name": "duckdb 1.5.4", "cmd": "cd /tmp/bench && python3 duck.py join", "runs": 3},
  {"name": "dsq 0.23.0", "cmd": "cd /tmp/bench && ./dsq bench.csv depts.xlsx \"SELECT d.DeptName, COUNT(*) FROM {0} b JOIN {1} d ON b.Dept = d.Dept GROUP BY d.DeptName\"", "runs": 3}
]
```

### 4.8 csvq 包装脚本（规避 shell 反引号转义问题）

`csvq_search.sh`：

```sh
#!/bin/sh
cd /tmp/bench && ./csvq "SELECT * FROM \`bench.csv\` WHERE Note LIKE '%ZXQ-7734%'"
```

`csvq_agg.sh`：

```sh
#!/bin/sh
cd /tmp/bench && ./csvq "SELECT Dept, COUNT(*), AVG(Amount) FROM \`bench.csv\` GROUP BY Dept ORDER BY 2 DESC"
```

> csvq 的表名是文件路径、需反引号包裹；反引号在 JSON/命令替换中会被 shell 吞掉，故用包装脚本隔离。

### 4.9 Top-K 测试专用脚本

#### 4.9.1 `gen_topk_data.py` — Top-K 数据生成

```python
#!/usr/bin/env python3
"""生成 Top-K 基准测试数据：N 行随机浮点数。
用法: BENCH_DIR=/path N=50000000 python3 gen_topk_data.py
默认: BENCH_DIR=/tmp/bench, N=50_000_000 (五千万行约 920MB)

数据特征: 每行一个浮点数，范围 [0, 1)，均匀分布。
为便于正确性校验，在随机位置插入 K 个已知标记值 (999.0 + i)，
确保各工具返回的最大 K 个值可交叉验证。"""
import os, sys, random

BASE = os.environ.get("BENCH_DIR", "/tmp/bench")
N = int(os.environ.get("N", "50_000_000"))
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
print(f"File size: {os.path.getsize(outpath) / 1024 / 1024:.1f} MB")
```

#### 4.9.2 `topk_heap.py` — Python heapq 基线（O(n log K) 流式）

```python
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
```

#### 4.9.3 `topk_duck.py` — DuckDB Top-N（ORDER BY ... LIMIT 堆优化）

```python
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
```

#### 4.9.4 `jobsE.json` — Top-K 任务定义

```json
[
  {"name": "grep-excel (DuckDB)", "cmd": "cd /tmp/bench && ./grep_excel bench_topk.csv -x \"SELECT * FROM bench_topk.bench_topk ORDER BY val DESC LIMIT 100\" -f simple", "runs": 3},
  {"name": "DuckDB 1.4.4", "cmd": "cd /tmp/bench && python3 topk_duck.py bench_topk.csv 100", "runs": 3},
  {"name": "Python heapq (O(n log k))", "cmd": "cd /tmp/bench && python3 topk_heap.py bench_topk.csv 100", "runs": 1, "timeout": 600},
  {"name": "Unix sort|head (O(n log n))", "cmd": "cd /tmp/bench && tail -n +2 bench_topk.csv | sort -rn | head -100", "runs": 1, "timeout": 600}
]
```

> 注意：grep-excel 需以 `engine-duckdb` 编译（`cargo build --features engine-duckdb,file-dialog --release`）才能使用 Top-N 堆优化。内存引擎不支持 SQL。macOS 上需设置 `DYLD_FALLBACK_LIBRARY_PATH` 指向 DuckDB dylib 所在目录。

## 5. 正确性校验（先于计时执行）

各工具命中数必须全部为 **285**，否则说明查询语义不一致：

```bash
cd /tmp/bench
grep -c "ZXQ-7734" bench.csv                                   # 285
./grep_excel bench.csv -q ZXQ-7734 -f simple | grep -c ZXQ-7734  # 285
./grep_excel bench.xlsx -q ZXQ-7734 -f simple | grep -c ZXQ-7734 # 285
python3 duck.py xlsx_search                                    # 285
./xsv search -s Note "ZXQ-7734" bench.csv | tail -n +2 | wc -l   # 285
./mlr --csv filter '$Note =~ "ZXQ-7734"' bench.csv | tail -n +2 | wc -l  # 285
sh csvq_search.sh | tail -n +2 | wc -l                          # 285
./dsq bench.csv "SELECT COUNT(*) FROM {} WHERE Note LIKE '%ZXQ-7734%'"   # 285
./dsq bench.xlsx "SELECT COUNT(*) FROM {} WHERE Note LIKE '%ZXQ-7734%'"  # 285
./trdsql -ih "SELECT COUNT(*) FROM bench.csv WHERE Note LIKE '%ZXQ-7734%'"  # 285
OCTOSQL_NO_TELEMETRY=1 ./octosql "SELECT COUNT(*) FROM bench.csv WHERE \"Note\" LIKE '%ZXQ-7734%'" -o stream_native  # 285
xlsxgrep "ZXQ-7734" bench.xlsx -F | wc -l                        # 285
```

### 5.1 Top-K 正确性校验

各工具的 Top-100 输出应从 999.099 递减到 999.0（共 K=100 个标记值）：

```bash
cd /tmp/bench
# 生成 Top-K 测试数据（默认 50M 行，可通过 N 环境变量调整）
N=1000000 python3 gen_topk_data.py

# 各工具输出应一致（前 5 个应为: 999.099 999.098 999.097 999.096 999.095）
python3 topk_heap.py bench_topk.csv 100 | head -3          # 999.099 / 999.098 / 999.097
python3 topk_duck.py bench_topk.csv 100 | head -3          # 同上
./grep_excel bench_topk.csv -x "SELECT * FROM bench_topk.bench_topk ORDER BY val DESC LIMIT 3" -f simple 2>/dev/null  # 同上
```

## 6. 计时方法学（重要）

- 每工具 **1 次预热 + 3 次正式计时，取最小值**；stdout 一律重定向到 /dev/null，排除终端渲染差异。
- 各工具在同机同文件上连续运行，页缓存已预热；绝对值随机器而异，**横向相对比较**有效。
- DuckDB 经 Python 驱动（含约 50ms 解释器启动开销）；其余为独立进程。
- dsq 以默认模式运行（每次灌库，即其一次性命令的真实成本；官方缓存机制需额外配置）。
- OctoSQL 列名区分大小写，需 `"Note"` 加引号；`-o stream_native` 避免交互式表格渲染开销。
- OctoSQL / csvq / trdsql / q / xsv / Miller 不能直接读 xlsx，未计入测试 A；转换格式的时间成本若计入会更高。
- **Top-K 测试**：`sort|head` 在 50M 行上未实测（O(n log n) 预期过长），其耗时通过 10M 实测值按复杂度公式外推。grep-excel 需以 DuckDB 引擎编译方可参与 Top-K 测试（内存引擎不支持 SQL）。

## 7. 预期结果参考（2026-07-19 实测，min of 3）

### Linux x86_64（grep-excel v0.7.1 预编译 DuckDB 引擎版）

```
== A. xlsx 全文搜索 ==
grep-excel 0.7.1        min=   3.872s
duckdb 1.5.4 (read_xlsx) min=   5.113s
xgrep 0.2.12             min=   6.810s
dsq 0.23.0               min=  17.511s
xlsxgrep 0.0.32          min=  49.932s

== B. CSV 全文搜索 ==
GNU grep (基线)          min=   0.003s
xsv 0.13.0 (单列/全行)   min=   0.064s / 0.114s
duckdb 1.5.4             min=   0.270s
octosql 0.13.0           min=   0.569s
grep-excel (单列/全列)   min=   0.615s / 0.765s
trdsql 1.2.3             min=   0.815s
miller 6.20.2            min=   1.083s
csvq 1.18.1              min=   2.772s
dsq 0.23.0               min=   4.978s

== C. CSV SQL 聚合 ==
octosql 0.13.0           min=   0.264s
duckdb 1.5.4             min=   0.265s
miller 6.20.2 (DSL)      min=   0.504s
grep-excel 0.7.1        min=   0.515s
trdsql 1.2.3             min=   0.866s
csvq 1.18.1              min=   2.391s
dsq 0.23.0               min=   5.077s

== D. 跨文件 JOIN (CSV × xlsx) ==
duckdb 1.5.4             min=   0.371s
grep-excel 0.7.1        min=   0.565s
dsq 0.23.0               min=   3.864s
```

### macOS arm64（grep-excel v0.7.1 源码构建 / DuckDB 1.4.4，交叉验证）

> 绝对值因 CPU 架构不同不可直比，横向相对排名为有效参考。

```
== A. xlsx 全文搜索 ==
grep-excel 0.7.1        min=   1.371s
duckdb 1.4.4 (read_xlsx) min=   0.994s
xlsxgrep 0.0.32          min=  19.574s

== B. CSV 全文搜索 ==
xsv 0.13.0 (单列/全行)   min=   0.038s / 0.037s
duckdb 1.4.4             min=   0.124s
grep-excel -c Note       min=   0.130s
GNU grep (BSD,基线)     min=   0.129s
grep-excel 0.7.1        min=   0.181s
miller 6.20.2            min=   0.230s

== C. CSV SQL 聚合 ==
miller 6.20.2 (DSL)      min=   0.071s
duckdb 1.4.4             min=   0.126s
grep-excel 0.7.1        min=   0.128s

== D. 跨文件 JOIN (CSV × xlsx) ==
grep-excel 0.7.1        min=   0.119s
duckdb 1.4.4             min=   0.127s
```

### Top-K 大规模筛选（macOS arm64，50M 行 / 919MB，取 K=100）

> 记 n = 数据总行数，K = 需返回的最大值个数。本节测试不同算法实现的复杂度特征。
> Linux x86_64 结果待验证（DuckDB Top-N 优化为跨平台行为，相对排名应与 macOS 一致）。

```
== E. Top-K (50M 行, K=100, 取最大 100 个) ==
duckdb 1.4.4 (ORDER BY LIMIT) min=   0.503s
grep-excel 0.7.1 (DuckDB)    min=   0.611s
Python heapq (O(n log k))    min=  18.426s
Unix sort|head (O(n log n))  min= ~175s (从 10M 实测外推)

缩放验证（同一算法在不同数据量下的线性度）:
  1M 行 (18 MB):  grep-excel 0.074s  DuckDB 0.123s  heapq 0.400s  sort 2.508s
  10M 行 (184 MB): grep-excel 0.180s  DuckDB 0.186s  heapq 3.652s  sort 32.620s
  50M 行 (919 MB): grep-excel 0.611s  DuckDB 0.503s  heapq 18.426s sort ~175s

复杂度分析 (n=50M, K=100):
  O(n log K) 堆方案：~7n 次比较，DuckDB 在 C++ 中实现，实测 0.5s
  O(n log K) 堆方案：~7n 次比较，Python heapq 解释执行，实测 18.4s
  O(n log n) 全排序：~26n 次比较，实测 ~175s
  → 堆方案的理论比较次数仅为全排序的 27%，DuckDB (C++) 比 Python 快 37 倍
```

## 8. 数据指纹

重新生成后执行 `md5sum bench.csv`，应得到：

```
b3fd130f786c908ecaabd9939de3ad79  bench.csv
```

若 MD5 不同，请检查 Python 版本差异（`random` 序列在 CPython 各版本稳定，但建议 3.10+）；行数必须为 200,001。
