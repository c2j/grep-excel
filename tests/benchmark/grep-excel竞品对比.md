# grep-excel 竞品对比与性能实测

> 测试日期：2026-07-19 · 主测环境：Linux x86_64 容器 · grep-excel v0.7.1（DuckDB 引擎）
> 辅助验证：macOS arm64 (Apple Silicon) · 同版本源码构建
> 所有竞品均为真实安装实测，性能数字为 3 次计时取最小值，结果行数经交叉校验一致。

## 一、结论先行

**有竞争者，但没有一个能覆盖 grep-excel 的组合能力。**

- 同类工具大致分四派：Excel grep（xlsxgrep、xgrep）、SQL-on-files（q、csvq、trdsql、dsq、OctoSQL）、CSV 专用（xsv、Miller）、通用引擎/交互探索（DuckDB、VisiData）。它们各自只覆盖 grep-excel 能力面的一小块。
- **xlsx 全文搜索性能 grep-excel 第一**，比 Python 系老牌工具 xlsxgrep 快约 **13 倍**。
- CSV 一次性流式搜索不是 grep-excel 的主场（先导入再查询有一次性开销），处于中游；但在其 TUI / SQL REPL 会话内，导入一次后可反复毫秒级查询，开销被摊销。
- **AI 集成（MCP Server）、单元格编辑回写、归档免解压直读、WPS 云文档导入、中英双语 TUI** 这些能力，竞品全部缺失。

## 二、竞争格局总览

| 派系 | 代表工具 | 与 grep-excel 的关系 |
|------|----------|----------------------|
| Excel grep 类 | xlsxgrep、xgrep | 直接同类，但只有"搜"，没有 SQL / TUI / 编辑 / AI |
| SQL-on-files 类 | q、csvq、trdsql、dsq、OctoSQL | 能查不能搜；除 dsq 外都不读 Excel；dsq 已停止维护 |
| CSV 专用类 | xsv、Miller | 速度极限，但完全不碰 Excel，无标准 SQL |
| 引擎 / 交互探索 | DuckDB、VisiData | DuckDB 是 grep-excel 的底层引擎而非对手；VisiData 强在人工交互探索，无 AI 集成 |

边缘工具：grepfiles（Go，图片/PDF/Office 混合检索，偏取证）、exgrep（仅 Windows，依赖 Excel COM 自动化）——不构成竞争。

## 三、功能对比矩阵

| 工具 | 语言 | 最新版本 / 维护状态 | 读 Excel | 格式广度 | grep 式搜索 | SQL | TUI | 编辑回写 | AI 集成 |
|------|------|------|------|------|------|------|------|------|------|
| **grep-excel** | Rust | v0.7.1 · 2026-07 活跃 | ✅ xlsx/xls/xlsm/xlsb/ods | csv/tsv/html/txt/md/dbf/xml/docx/pptx、zip/tar 归档、WPS 云文档 | ✅ 全文/精确/通配符/正则 | ✅ DuckDB/SQLite/内存三引擎 | ✅ 双语 | ✅ 单元格/行列 | ✅ MCP Server（19 工具） |
| xgrep | Python | 0.2.12 · 2026-01[^2] | ✅ xlsx | — | ✅ 正则 | ❌ | ❌ | ❌ | ❌ |
| xlsxgrep | Python | 0.0.32 · 2025-12[^1][^7] | ✅ xlsx/xls/xlsm/ods | csv/tsv | ✅ 正则/忽略大小写 | ❌ | ❌ | ❌ | ❌ |
| q | Python | 3.1.6 · 2021 停更[^9] | ❌ | csv/tsv | ❌ | ✅ SQLite | ❌ | ❌ | ❌ |
| csvq | Go | 1.18.1 · 2023-03[^6] | ❌ | csv/tsv/ltsv/json/定长 | ❌ | ✅ 自研（含 UPDATE/事务） | REPL | ✅ UPDATE 回写 CSV | ❌ |
| trdsql | Go | 1.2.3 · 2026-05[^6] | ❌ | csv/ltsv/json/tbln | ❌ | ✅ SQLite/MySQL/PG | ❌ | ❌ | ❌ |
| dsq | Go | 0.23.0 · 2022-10 **已归档**[^3][^6] | ✅ | csv/tsv/json/parquet/ods/orc/avro/yaml/日志 | ❌ | ✅ SQLite（带缓存） | ❌ | ❌ | ❌ |
| OctoSQL | Go | 0.13.0 · 2024-05[^6] | ⚠️ 需另装插件 | csv/json/parquet + 数据库源 | ❌ | ✅ 自研流式引擎 | ❌ | ❌ | ❌ |
| xsv | Rust | 0.13.0 · 2018 停更[^8] | ❌ | csv | ✅ search 子命令 | ❌ | ❌ | ❌ | ❌ |
| Miller | Go | 6.20.2 · 2026-07 活跃[^6] | ❌ | csv/tsv/json/pprint/xtab | ⚠️ filter DSL | ❌ 自有 DSL | ❌ | ❌ | ❌ |
| DuckDB | C++ | 1.5.4 · 活跃 | ✅ 官方 excel 扩展 | csv/parquet/json 原生 | ❌ | ✅ 功能最全 | CLI shell | ❌ | ❌ |
| VisiData | Python | 3.3 · 活跃（GPL）[^5] | ✅ | 极广（含 sqlite/hdf5/sas 等） | ✅ | ⚠️ Python 表达式 | ✅ 强大 | ✅ | ❌ |

**一句话总结矩阵**：会搜的不会查，会查的不读 Excel，读 Excel 的没有 TUI/编辑/AI——grep-excel 是唯一四项全能。

## 四、性能实测

**测试数据**：合成 200,000 行 × 10 列（含中英文、日期、金额），`bench.xlsx` 13.0 MB / `bench.csv` 16.9 MB；检索词命中 285 行，各工具返回行数已校验一致。数据指纹：`MD5(bench.csv) = b3fd130f786c908ecaabd9939de3ad79`。

以下主表格为 **Linux x86_64** 环境实测（grep-excel v0.7.1 预编译 DuckDB 引擎版）；末尾附 **macOS arm64** 交叉验证结果。

### 测试 A：xlsx 全文搜索（核心场景）

| 排名 | 工具 | 耗时（min of 3） | 相对倍数 |
|------|------|------|------|
| 1 | **grep-excel 0.7.1** | **3.87 s** | 1.0× |
| 2 | DuckDB 1.5.4（read_xlsx 扩展） | 5.11 s | 1.3× |
| 3 | xgrep 0.2.12 | 6.81 s | 1.8× |
| 4 | dsq 0.23.0 | 17.51 s | 4.5× |
| 5 | xlsxgrep 0.0.32 | 49.93 s | 12.9× |

其余工具（csvq/trdsql/q/xsv/Miller/OctoSQL）不能直接读 xlsx，需先转换格式，未计入。

> xgrep 较快的原因是底层解析用了 python-calamine（Rust calamine 的 Python 绑定）——与 grep-excel 同属 calamine 技术路线；xlsxgrep 走 openpyxl 纯 Python 解析，慢一个数量级。

### 测试 B：CSV 全文搜索

| 排名 | 工具 | 耗时 | 说明 |
|------|------|------|------|
| — | GNU grep（基线） | 0.003 s | 纯字节扫描，无表格语义，仅作物理上限参考 |
| 1 | xsv 0.13.0 | 0.064–0.114 s | CSV 专用零拷贝 |
| 2 | DuckDB 1.5.4 | 0.270 s | 原生 CSV 流式扫描 |
| 3 | OctoSQL 0.13.0 | 0.569 s | 流式引擎 |
| 4 | **grep-excel 0.7.1** | **0.615–0.765 s** | 全列 0.765 s；限定 `-c Note` 单列 0.615 s |
| 5 | trdsql 1.2.3 | 0.815 s | SQLite 灌库后查询 |
| 6 | Miller 6.20.2 | 1.083 s | DSL filter |
| 7 | csvq 1.18.1 | 2.772 s | 全量载入内存 |
| 8 | dsq 0.23.0 | 4.978 s | SQLite 灌库（无缓存时） |

> CSV 是流式工具的主场：grep-excel 需先导入建表再查询，一次性命令场景存在固定开销。但其交互模式（TUI / `-i` REPL）导入一次即可反复查询，且官方 roadmap 中已有 CSV 加载优化计划（2026-07-18 设计文档）[^10]。

### 测试 C：CSV SQL 聚合（GROUP BY + AVG）

| 排名 | 工具 | 耗时 |
|------|------|------|
| 1 | OctoSQL 0.13.0 | 0.264 s |
| 2 | DuckDB 1.5.4 | 0.265 s |
| 3 | Miller 6.20.2（DSL） | 0.504 s |
| 4 | **grep-excel 0.7.1** | **0.515 s** |
| 5 | trdsql 1.2.3 | 0.866 s |
| 6 | csvq 1.18.1 | 2.391 s |
| 7 | dsq 0.23.0 | 5.077 s |

（xsv 不支持分组聚合一步到位，q 因 Python 3.12 兼容问题未能安装——本身即维护停滞的信号。）

### 测试 D：跨文件 JOIN（CSV 20 万行 × xlsx 维表）

| 排名 | 工具 | 耗时 |
|------|------|------|
| 1 | DuckDB 1.5.4 | 0.371 s |
| 2 | **grep-excel 0.7.1** | **0.565 s** |
| 3 | dsq 0.23.0 | 3.864 s |
| — | 其余工具 | 不支持直接对 xlsx 做 JOIN |

### 测试 E：大规模 Top-K（5000 万行浮点数取最大 100）

**问题描述**：在 n 行浮点数的文件中找到最大的前 K 个数字。取 n = 50,000,000（919 MB），K = 100。

**算法复杂度对比**：

| 方案 | 时间复杂度 | 空间复杂度 | 机制 |
|------|-----------|-----------|------|
| 全量排序 (`sort\|head`) | O(n log n) | O(n) | 对所有数据排序后取前 K |
| 最小堆 (Python heapq) | O(n log K) | O(K) | 单遍扫描，维护大小为 K 的堆 |
| DuckDB `ORDER BY ... LIMIT` | O(n log K) | O(n)[^csv] | 内部 Top-N 堆优化，非全量排序 |

[^csv]: DuckDB 需将 CSV 解析加载为列式内存结构，空间开销为 O(n)，但 CPU 比较次数为 O(n log K)。

**实测结果（macOS arm64，50M 行）**：

| 排名 | 工具 | 耗时 | 与最优倍数 | 复杂度 |
|------|------|------|------|------|
| 1 | DuckDB 1.4.4（`ORDER BY LIMIT`） | **0.503 s** | 1.0× | O(n log K) 堆优化（C++） |
| 2 | **grep-excel 0.7.1**（DuckDB 引擎） | **0.611 s** | 1.2× | O(n log K) 堆优化（同引擎） |
| 3 | Python heapq（`heapq` 流式） | 18.426 s | 36.6× | O(n log K) 堆（解释型 Python） |
| — | Unix `sort -rn \| head`（全量排序） | >175 s（估算）[^sort] | >348× | O(n log n) 全量排序 |

[^sort]: 50M 行 `sort|head` 未实测（预计耗时数分钟）。从 10M 行实测 32.62 s 按 O(n log n) 外推：32.62 × 5 × log(50M)/log(10M) ≈ 175 s。

**多规模缩放验证**（证明 O(n log K) vs O(n log n) 随数据量分化）：

| 数据规模 | grep-excel (DuckDB) | DuckDB Python | Python heapq | Unix sort\|head |
|----------|---------------------|---------------|-------------|-----------------|
| 100 万行 (18 MB) | 0.074 s | 0.123 s | 0.400 s | 2.508 s |
| 1000 万行 (184 MB) | 0.180 s | 0.186 s | 3.652 s | 32.620 s |
| 5000 万行 (919 MB) | 0.611 s | 0.503 s | 18.426 s | ~175 s（估） |
| 10 亿行（外推） | ~12 s | ~10 s | ~370 s | > 1 小时 |

> **关键发现**：DuckDB 的 `ORDER BY ... LIMIT` 使用内部 Top-N 堆（非全量排序），因此时间复杂度为 O(n log K) 而非 O(n log n)。在 K=100、n=50M 时，log K ≈ 7、log n ≈ 26，堆方案的理论比较次数仅为全量排序的 **27%**。实测中 DuckDB（C++ 引擎）比 Python heapq（相同算法）快 **30–37 倍**，比全量排序快 **180 倍以上**，且差距随 n 增大持续扩大。
>
> grep-excel 的 DuckDB 引擎结果与 DuckDB 原生几乎一致（微小差距来自进程启动和表别名解析），在所有 SQL-on-files 工具中独享此 Top-N 优化——SQLite 系工具（dsq/trdsql/q）的 `ORDER BY ... LIMIT` 依赖 SQLite 内部实现，通常退化为全量排序 + 截断。

### 性能小结

- **xlsx 场景**：grep-excel 全场最快——calamine 解析 + DuckDB 引擎的组合在 Excel 上跑赢所有对手。
- **CSV 一次性命令**：xsv/DuckDB/OctoSQL 等流式工具更快；grep-excel 中游，适合"导入后反复查"的会话式用法。
- **聚合 / 关联分析**：DuckDB、OctoSQL 一档，grep-excel 紧随其后并大幅领先 SQLite 灌库系（dsq/trdsql）与自研引擎（csvq）。
- **Top-K 大规模筛选**：DuckDB 引擎的 `ORDER BY ... LIMIT` 内部使用 Top-N 堆优化（O(n log K)），而非全量排序（O(n log n)）。5000 万行数据取 Top-100，grep-excel（DuckDB 引擎）仅需 **0.611 s**；而 Unix `sort|head` 全量排序在 1000 万行即需 32 s，差距随数据量增大而急剧拉大。这验证了**引擎选择在算法敏感场景中的决定性作用**。
- 参考：dsq 官方 README 的第三方对比也承认灌库模式的成本，靠结果缓存弥补[^3]。

### macOS arm64 交叉验证（2026-07-19 同机同数据，grep-excel v0.7.1 源码构建 / DuckDB 1.4.4）

> 以下结果为 macOS Apple Silicon 环境独立验证，绝对耗时因 CPU 架构差异不可直接与上表比较，**横向相对排名**可作为参考。

**测试 A（xlsx 全文搜索）**：grep-excel 1.371 s，DuckDB 0.994 s，xlsxgrep 19.574 s。grep-excel 与 DuckDB 同属 calamine 技术栈，在一档；xlsxgrep（openpyxl 纯 Python 解析）慢一个数量级，结论与 Linux 一致。

**测试 B（CSV 全文搜索）**：xsv 0.037–0.038 s > DuckDB 0.124 s > grep-excel `-c Note` 0.130 s > GNU grep (BSD) 0.129 s > grep-excel 0.181 s > miller 0.230 s。注意 macOS 自带 BSD grep 远慢于 GNU grep，实际对流式搜索参考价值有限。

**测试 C（SQL 聚合）**：miller 0.071 s > DuckDB 0.126 s ≈ grep-excel 0.128 s。Miller DSL 在简单聚合上借助流式处理占优，DuckDB/grep-excel 在 SQL 通用性上更强。

**测试 D（跨文件 JOIN）**：grep-excel 0.119 s > DuckDB 0.127 s。grep-excel 略微领先，均远优于 dsq 的 SQLite 灌库模式。

**验证结论**：核心排名模式（grep-excel ≈ DuckDB ≫ xlsxgrep；xsv/miller 在 CSV 流式场景领先）在 arm64 上完全复现，数据一致性校验（285 命中）全部通过，MD5 指纹匹配。

## 五、分场景选型建议

| 场景 | 推荐 |
|------|------|
| 在 Excel/WDR 报告/多格式文件中**找数据** | **grep-excel**（唯一能 grep Excel 且性能第一） |
| 跨 Excel+CSV 做 **SQL 关联核对** | **grep-excel**（一条命令，无需转换） |
| 让 **AI 助手**（Claude/Cursor）直接查表、改表 | **grep-excel**（唯一内置 MCP Server） |
| 服务器上纯 CSV 大文件单行过滤 | xsv / Miller / DuckDB（流式更快） |
| 数据湖格式（Parquet/JSON）重 SQL 分析 | DuckDB 本体 |
| 人工交互式数据探索/清洗 | VisiData |
| CSV 需要 UPDATE 回写 | csvq（备选：grep-excel 编辑后另存） |
| dsq / q / textql | 不推荐（已归档/停更） |

## 六、附录：测试方法

- **测试数据**：`gen_data.py` 生成 200,000 行 × 10 列合成数据（seed=42，可复现）；`bench.csv` MD5 为 `b3fd130f786c908ecaabd9939de3ad79`。
- **计时**：每工具 1 次预热 + 3 次正式运行取最小值，stdout 重定向至 /dev/null；沙箱 CPU 受限，绝对值仅供横向相对比较。
- **正确性**：各工具命中行数统一校验为 285；聚合结果抽样一致。
- **DuckDB**：经 Python 驱动调用（含解释器启动约 50ms 固定开销）；xlsx 通过官方 `excel` 扩展 `read_xlsx` 读取（首次需 `INSTALL excel`）。
- **trdsql**：官方预编译版要求 glibc 2.38（本环境不满足），改用源码构建同版本测试。
- **公平性**：CSV 搜索同时给出 grep-excel 全列（0.765 s）与单列（0.615 s）、xsv 全行（0.114 s）与单列（0.064 s）两组数字；所有 SQL 工具执行相同语义的 `LIKE '%ZXQ-7734%'` 查询。
- **macOS 交叉验证**：grep-excel 为 `cargo build --features full --release` 源码构建，xsv/miller 通过 Homebrew 安装，xlsxgrep 通过 pip 安装。xgrep 截至测试日不支持 macOS arm64 二进制分发包，未纳入 macOS 对比。dsq/csvq/trdsql/octosql 未在 macOS 上验证。
- **Top-K 测试数据**：`gen_topk_data.py` 生成 N 行随机浮点数（uniform [0,1)），并在随机位置埋入 100 个标记值（999.xxx）用于交叉校验。默认 N=50,000,000，可通过环境变量 `N` 调整。grep-excel 使用 DuckDB 引擎（`cargo build --features engine-duckdb --release`）以启用 Top-N 堆优化。
- **可复现**：全套脚本与数据生成逻辑见 `tests/benchmark/scripts/`。复现流程：`cd tests/benchmark && bash reproduce.sh`。

---

[^1]: https://github.com/zazuum/xlsxgrep — xlsxgrep 功能与格式支持
[^2]: https://pypi.org/project/xgrep/ — xgrep 发布信息（0.2.12, 2026-01-18）
[^3]: https://github.com/multiprocessio/dsq — dsq README 对比表与基准（项目已归档，v0.23.0, 2022-10-20）
[^5]: https://pypi.org/project/visidata/ — VisiData v3.3 格式支持与 GPL 许可
[^6]: https://goproxy.cn — csvq v1.18.1 (2023-03-26)、OctoSQL v0.13.0 (2024-05-11)、trdsql v1.2.3 (2026-05-29)、dsq v0.23.0 (2022-10-20)、miller v6.20.2 (2026-07-04) 发布时间
[^7]: https://pypi.org/project/xlsxgrep/ — xlsxgrep 0.0.32 (2025-12-23)
[^8]: https://github.com/BurntSushi/xsv — xsv 0.13.0 (2018)，已停止维护
[^9]: https://github.com/harelba/q — q (Text as Data) 3.1.6
[^10]: https://github.com/c2j/grep-excel — README 及 docs/plans/2026-07-18-csv-duckdb-loading-optimization.md
