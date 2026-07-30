[English](README.md)

# grep-excel（中文说明）

基于 DuckDB 的多格式表格文件搜索 TUI 工具（Excel/CSV/TSV/HTML/文本/Markdown/Word/PowerPoint/DBF/XML）。

grep-excel 提供快速的交互式终端界面，用于在多个电子表格与表格文件中进行搜索。使用 DuckDB 作为查询引擎，支持全文搜索、精确匹配、通配符模式匹配、正则搜索和 SQL 查询。支持 CLI 命令行查询、TUI 交互模式、MCP 服务器与 AI 助手集成，以及批量 `--exec` 命令流水线。

## 功能特性

- **多种搜索模式** — 全文（不区分大小写子串）、精确（区分大小写）、通配符（SQL LIKE `%` / `_`）、正则（多关键词 OR 用 `|`）
- **SQL 查询** — 直接对导入的数据执行 `SELECT` 语句，支持 DuckDB 分析函数
- **多引擎后端** — DuckDB（高性能 OLAP）、SQLite 或纯内存引擎；通过 feature flag 选择
- **TUI 交互模式** — 键盘驱动的终端界面，导入后自动浏览、选项卡结果、详情面板、平铺/表格视图，支持 Ctrl+方向键切换文件/Sheet
- **HTML / 文本 / Markdown 表格** — 导入 HTML 报告（如 WDR/AWR）、纯文本表格与 GFM Markdown 管道表为可查询 sheet；自动检测编码（UTF-8 / meta charset / CJK 回退）
- **Word / PowerPoint / DBF / XML** — 提取 `.docx` 文档表格（sheet 名取自标题段落，合并单元格自动前向填充）与 `.pptx` 幻灯片表格（每页一个 sheet）；导入 dBase `.dbf` 与扁平 `.xml` 数据为可查询 sheet；扩展名缺失或误导时可用 `--as` 强制指定格式
- **MCP 服务器模式** — 通过 19 个 MCP 工具与 AI 助手（Claude、Cursor）集成，支持搜索、数据探索、统计分析、编辑和导出
- **交互式 SQL REPL** — 多行 SQL 交互式 shell，支持 `;` 结束输入、命令历史和点命令（`.tables`、`.files`、`.output`、`.save`、`.help`）；用 `-i` 启动
- **CLI `--exec` 流水线** — 在命令行中以单条命令或多步 JSON 数组执行 MCP 工具
- **文件编辑** — 更新单元格、插入/删除行、添加/重命名列、保存回原文件或导出为新文件（电子表格格式；`.docx`/`.pptx` 为只读）
- **聚合统计** — 对匹配结果按列统计不同值的分布
- **修复损坏文件** — 在 ZIP/XML 层面从损坏的 `.xlsx` 文件中恢复数据（`--repair`）
- **云文档链接导入** — 直接传入金山文档 (kdocs.cn) 分享链接；通过登录 Cookie 下载。使用 `--kdocs-cookie` 或 `KDOCS_COOKIE` 环境变量。企业版域名请设置 `SHARE_HOSTS` 环境变量
- **归档文件支持** — 直接导入 `.zip`、`.tar`、`.tar.gz`、`.tar.bz2`、`.tar.xz` 归档文件及 `.zip.001` 分卷压缩包，无需手动解压
- **Excel 日期自动识别** — 自动检测 Excel 日期序列号并转换为 ISO 8601 格式（`YYYY-MM-DD` 或 `YYYY-MM-DD HH:MM:SS`），保留时间信息，支持日期范围搜索
- **多种输出格式** — Markdown 表格、美化打印、JSON 和简单 TSV（`--format`）
- **CSV 导出** — 将搜索或 SQL 结果导出为 CSV 文件
- **友好表别名** — 在 SQL 中使用 `文件名.工作表名` 语法替代内部 `sheet_N_M` 名称
- **国际化 / 中文支持** — 从 `LANG`/`LC_ALL` 环境变量自动检测语言；TUI、CLI 和帮助文本均支持中英双语
- **跨平台** — Windows（x86_64，Win7+）、macOS（Intel x86_64、Apple Silicon ARM64）、Linux（x86_64 glibc 2.31+、ARM64）

## 安装

### 下载预编译二进制文件

从 [GitHub Releases 页面](https://github.com/c2j/grep-excel/releases) 下载适用于您平台的最新版本。

支持的平台：
- Windows (x86_64)
- macOS (Intel x86_64, Apple Silicon ARM64)
- Linux (x86_64, ARM64)

### 从源码构建

要求：Rust 1.70+ 和 Cargo

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# 默认构建（内存引擎 + 文件对话框）
cargo build --release

# 使用 DuckDB 引擎（推荐用于生产环境）
cargo build --release --features full

# DuckDB 捆绑构建（独立运行，无需系统 DuckDB）
cargo build --release --features duckdb-bundled

# 使用 SQLite 引擎
cargo build --release --features engine-sqlite

# 无头构建（无文件对话框）
cargo build --release --no-default-features --features engine-memory
```

> **提示：** 使用 DuckDB 功能构建时，设置 `DUCKDB_DOWNLOAD_LIB=1` 可下载预编译库而非从源码编译，大幅加快构建速度。

### Feature Flags（功能标志）

| 功能标志 | 说明 |
|---------|------|
| `engine-memory` | 内存引擎（默认） |
| `engine-duckdb` | DuckDB 引擎（高性能） |
| `engine-sqlite` | SQLite 引擎 |
| `duckdb-bundled` | DuckDB 捆绑 C 库（独立运行） |
| `file-dialog` | 原生文件选择对话框（默认） |
| `mcp-server` | MCP 服务器模式 + xlsx 写入支持（AI 助手集成，含 `save_as` / `save`） |
| `share-url` | 云文档分享链接导入（kdocs.cn / 企业版域名） |
| `archive-support` | 归档支持：`.zip`、`.tar`、`.tar.gz`、`.tar.bz2`、`.tar.xz`、分卷 `.zip.001` |
| `pdf-support` | PDF 文档表格提取（只读） |
| `parquet-support` | Parquet 列式存储读取（只读） |
| `full` | 全部功能：内存引擎 + 文件对话框 + MCP 服务器 + 分享链接 + 归档支持 + pdf-support + parquet-support |

## 使用方式

```bash
grep_excel [文件...] [选项]
```

### CLI 选项

| 选项 | 缩写 | 说明 |
|------|------|------|
| `--interactive` | `-i` | 启动交互式 SQL REPL：`$` 提示符，多行输入（`;` 执行），上下方向键历史，点命令 |
| `--no-history` | — | 禁用跨会话 SQL 历史持久化（默认保存） |
| `--query` | `-q` | 搜索查询字符串 |
| `--column` | `-c` | 筛选指定列名 |
| `--sheet` | `-s` | 筛选指定工作表名称 |
| `--mode` | `-m` | 搜索模式：`fulltext`（默认）、`exact`、`wildcard`、`regex` |
| `--invert` | `-v` | 反向匹配：显示不匹配的行 |
| `--sql` | `-x` | 对导入数据执行 SQL SELECT 查询 |
| `--export` | `-e` | 将搜索结果导出为 CSV 文件 |
| `--exec` | `-E` | 以 JSON 执行 MCP 工具命令 |
| `--mcp` | — | 启动 MCP 服务器模式（stdio） |
| `--aggregate` | `-g` | 按列统计匹配行中不同值的分布 |
| `--list-tables` | `-t` | 列出已导入表及其友好名称和列名 |
| `--format` | `-f` | 输出格式：`markdown`（默认）、`pretty`、`json`、`simple`（TSV） |
| `--repair` | `-r` | 导入前尝试修复损坏的 xlsx 文件（ZIP/XML 层面） |
| `--run` | `-X` | 对每个匹配行执行 Shell 命令，`${列名}` 引用单元格值 |
| `--run-output-column` | — | 将 `--run` 命令 stdout 写入指定列（列不存在则自动创建） |
| `--help` | `-h` | 显示帮助（含支持的文件格式；自动检测中/英文） |
| `--kdocs-cookie` | — | 金山文档 (kdocs.cn) 分享链接下载专用 Cookie |
| `--share-hosts` | — | 企业版云文档分享链接的额外域名（逗号分隔） |
| `--as` | — | 按文件覆盖格式（粘性）：对后续所有文件生效，直到下一个 `--as`。可选：`csv`、`tsv`、`html`、`txt`、`md`、`dbf`、`xml`、`excel`、`docx`、`pptx`、`pdf`、`parquet` |

### 示例

搜索单个文件：
```bash
grep_excel data.xlsx -q "搜索关键词"
```

多文件搜索并筛选列：
```bash
grep_excel a.xlsx b.xlsx -c "姓名" -m exact
```

通配符搜索（`%` 匹配任意字符，`_` 匹配单个字符）：
```bash
grep_excel data.xlsx -q "张%" -m wildcard
```

正则多关键词搜索（用 `|` 表示 OR）：
```bash
grep_excel data.xlsx -q "张三|李四" -m regex
```

仅搜索指定工作表：
```bash
grep_excel data.xlsx -q "工程部" -s 员工表
```

反向匹配 — 查找不包含查询词的行：
```bash
grep_excel data.xlsx -q "工程部" -v
```

聚合统计 — 统计不同值分布：
```bash
grep_excel data.xlsx -q "工程部" -g 部门
```

列出表别名：
```bash
grep_excel data.xlsx employees.xlsx -t
```

修复并导入损坏的 xlsx：
```bash
grep_excel corrupted.xlsx -q "数据" -r
```

从金山文档分享链接导入：
```bash
export KDOCS_COOKIE='wps_sid=...; ...'
grep_excel 'https://www.kdocs.cn/l/xxxx' -q "关键词"

# 或直接传入 Cookie
grep_excel --kdocs-cookie "$KDOCS_COOKIE" 'https://www.kdocs.cn/l/xxxx' -t
```

搜索归档文件中的表格：
```bash
# 搜索 ZIP 中的 xlsx 文件
grep_excel audit_2026.zip -q "异常交易"

# 搜索 tar.gz 中的 CSV
grep_excel db_dump.tar.gz -x "SELECT * FROM sheet_1_0 LIMIT 10"

# 分卷 ZIP
grep_excel big_data.zip.001 -t
```

指定格式导出：
```bash
grep_excel data.xlsx -q "关键词" -e results.csv -f json
```

启动 TUI 模式（不带 CLI 参数）：
```bash
grep_excel
```

启动交互式 SQL REPL：
```bash
# 预导入文件后启动 REPL
grep_excel data.xlsx employees.xlsx -i

# REPL 内操作：
#   $ SELECT * FROM sheet_1_0 LIMIT 5;
#   $ .tables          # 列出已导入表
#   $ .files           # 列出已导入文件
#   $ .output out.csv  # 将后续 SQL 结果持续重定向到 CSV
#   $ .output          # 恢复终端输出
#   $ .save out.json json  # 保存上次 SQL 结果 (csv|json|tsv|table)
#   $ .let t AS SELECT City, COUNT(*) AS n FROM sheet_1_0 GROUP BY City
#                      # 将 SQL 结果物化为临时表 "t"
#   $ SELECT * FROM t; # 按裸名称查询临时表
#   $ .drop t          # 用完删除临时表
#   $ .help            # 显示点命令
#   $ .exit            # 退出（Ctrl+D 也可退出）
```

REPL 中输入的 SQL 和点命令会保存到历史文件（Linux：
`~/.local/state/grep-excel/history.txt`，macOS：
`~/Library/Application Support/grep-excel/history.txt`），下次启动可用上下方向键跨会话召回。传入 `--no-history` 可在本次会话中关闭。

HTML / 文本 / Markdown 表格与 Excel 用法相同：
```bash
grep_excel report.html -q "CPU"
grep_excel awr.md -x "SELECT * FROM \"Host CPU\" LIMIT 10"
grep_excel data.txt -t
```

Word、PowerPoint、DBF、XML 文件用法相同：
```bash
grep_excel report.docx -q "预算"          # 提取 word/document.xml 中的表格
grep_excel slides.pptx -t                 # 每页幻灯片一个 sheet
grep_excel legacy.dbf -q "Smith"          # dBase 数据库
grep_excel data.xml -x "SELECT * FROM data LIMIT 10"
```

扩展名缺失或误导时强制指定格式（`--as` 为粘性选项，对后续所有文件生效直到下一个 `--as`）：
```bash
grep_excel --as csv access.log --as excel dump.dat -t
```

对每个匹配行执行 Shell 命令（`--run` / `-X`）：
```bash
# 对每个匹配行执行外部工具，使用 ${列名} 替代
grep_excel data.xlsx -q "ERROR" -c "等级" --run './analyzer "${消息}"'

# 将命令输出写入新列，再导出
grep_excel data.xlsx -q "TODO" -c "类型" --run './classifier "${标题}"' --run-output-column "分类" -e output.xlsx

# 配合 --sql 使用
grep_excel data.xlsx --sql "SELECT 姓名, SQL FROM sheet_1_0 WHERE 类型='旧版'" --run './formatter "${SQL}"'
```

> `--run` 对每个匹配行执行 `sh -c`，`${列名}` 引用单元格值（自动 shell 转义），`$$` 表示字面 `$`。

### MCP 配置

在 AI 助手的 MCP 配置中添加（如 `claude_desktop_config.json` 或 Cursor MCP 设置）：

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/path/to/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

> **提示：** `materialize_query`（将 SQL 结果物化为会话临时表）和 `drop_temp_table`（删除临时表）工具可在复杂分析中复用中间结果。REPL 中对应 `.let <名> AS <SQL>` 和 `.drop <名>` 命令。

## 支持的文件格式

- `.xlsx` — Excel 2007+（Open XML）
- `.xls` — Excel 97-2004（BIFF8）
- `.xlsm` — Excel 启用宏
- `.xlsb` — Excel 二进制
- `.ods` — OpenDocument 电子表格
- `.csv` — 逗号分隔值
- `.tsv` / `.tab` — 制表符分隔值
- `.html` / `.htm` — HTML 表格（自动检测编码；每个 `<table>` 作为一个 sheet）
- `.txt` — 纯文本表格（章节 / 短横线分隔 / 对齐启发式）
- `.md` / `.markdown` — GFM Markdown 管道表
- `.dbf` — dBase 数据库文件
- `.xml` — XML 数据文件（扁平约定：根元素下重复的同名子元素作为行，其子标签作为列）
- `.docx` — Word 文档（从 word/document.xml 提取表格；只读，不支持编辑）
- `.pptx` — PowerPoint 演示文稿（从每张幻灯片提取表格；只读，不支持编辑）
- `.pdf` — PDF 文档（文本型，通过线条策略提取表格；只读，不支持 OCR）
- `.parquet` — Parquet 列式存储（只读；通过 DuckDB 引擎保留原生类型）
- `.zip` — ZIP 归档文件，内部表格文件自动提取导入
- `.tar` / `.tar.gz` / `.tgz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` — TAR 归档（压缩或未压缩）
- `.zip.001` / `.zip.002` — 分卷压缩 ZIP

归档文件透明处理：直接传入 `.zip` 或 `.tar.gz`，内部所有可识别的表格文件自动提取并以 `archive::path/file.xlsx` 命名导入。

## TUI 快捷键（摘要）

| 按键 | 功能 |
|------|------|
| `Ctrl+←` / `Ctrl+→` | 在同一文件内切换 Sheet |
| `Ctrl+↑` / `Ctrl+↓` | 切换文件 |
| `v` | 切换平铺/表格视图 |
| `S` | SQL 查询模式 |
| `?` | 帮助 |

导入文件后 TUI **自动浏览**首个 sheet（无需先搜索）。多文件时标签显示 `文件:sheet`；「全部」标签使用单一 **来源** 列（`文件:sheet`）。

## 许可证

MIT License — 详见 [LICENSE](LICENSE)
