# grep-excel 用户手册

grep-excel 是一款基于 DuckDB 的高性能多格式表格文件搜索工具（Excel / CSV / TSV / HTML / 文本 / Markdown / Word / PowerPoint / DBF / XML），支持终端交互（TUI）、命令行（CLI）、MCP 服务器和批量执行等多种使用方式。

## 目录

- [快速开始](#快速开始)
- [支持的文件格式](#支持的文件格式)
- [CLI 命令行模式](#cli-命令行模式)
- [归档文件与云文档链接导入](#归档文件与云文档链接导入)
- [交互式 SQL REPL（-i）](#交互式-sql-repl-i)
- [TUI 交互模式](#tui-交互模式)
- [MCP 服务器模式](#mcp-服务器模式)
- [--exec 执行模式](#--exec-执行模式)
- [--run Shell 命令模式](#--run-shell-命令模式)
- [搜索模式详解](#搜索模式详解)
- [SQL 查询指南](#sql-查询指南)
- [文件编辑](#文件编辑)
- [输出格式](#输出格式)
- [常见问题](#常见问题)

---

## 快速开始

### 方式一：下载预编译二进制

从 [GitHub Releases](https://github.com/c2j/grep-excel/releases) 下载对应平台的压缩包：

- `grep_excel-windows-x86_64.zip` — Windows
- `grep_excel-macos-x86_64.zip` — macOS Intel
- `grep_excel-macos-aarch64.zip` — macOS Apple Silicon
- `grep_excel-linux-x86_64.zip` — Linux x86_64
- `grep_excel-linux-aarch64.zip` — Linux ARM64

解压后即可使用：

```bash
# Linux / macOS
chmod +x grep_excel
./grep_excel --help

# Windows
grep_excel.exe --help
```

### 方式二：从源码构建

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# 快速构建（内存引擎）
cargo build --release

# 推荐：全功能构建
cargo build --release --features full
```

构建完成后，二进制文件位于 `target/release/grep_excel`。

---

## 支持的文件格式

| 扩展名 | 说明 |
|--------|------|
| `.xlsx` / `.xls` / `.xlsm` / `.xlsb` / `.ods` | Excel / OpenDocument 电子表格 |
| `.csv` | 逗号分隔值 |
| `.tsv` / `.tab` | 制表符分隔值 |
| `.html` / `.htm` | HTML 表格；每个 `<table>` 导入为一个 sheet；自动检测编码（UTF-8 / `<meta charset>` / CJK 回退） |
| `.txt` | 纯文本表格（按章节、短横线分隔或列对齐启发式提取） |
| `.md` / `.markdown` | GFM Markdown 管道表（`\| col \|`） |
| `.dbf` | dBase 数据库文件 |
| `.xml` | XML 数据文件（扁平约定：根元素下重复的同名子元素作为行，其子标签作为列） |
| `.docx` | Word 文档；从 word/document.xml 提取表格，sheet 名取自表格前的标题段落，合并单元格自动前向填充；**只读**，不支持编辑 |
| `.pptx` | PowerPoint 演示文稿；每页幻灯片的表格导入为一个 sheet，合并单元格自动填充；**只读**，不支持编辑 |
| `.zip` / `.tar` / `.tar.gz` / `.tgz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` | 归档文件；自动提取内部所有可识别的表格文件（见 [归档文件与云文档链接导入](#归档文件与云文档链接导入)） |
| `.zip.001` / `.zip.002` | 分卷 ZIP 压缩包 |

其他格式与 Excel 用法相同：

```bash
# 搜索 HTML 报告（如 openGauss WDR）
grep_excel report.html -q "CPU"

# 对 Markdown 中的表执行 SQL
grep_excel awr.md -x "SELECT * FROM sheet_1_0 LIMIT 10"

# 列出文本文件中提取到的表
grep_excel data.txt -t

# Word 文档中的表格
grep_excel report.docx -q "预算"

# PowerPoint：每页幻灯片一个 sheet
grep_excel slides.pptx -t

# dBase / XML 数据文件
grep_excel legacy.dbf -q "Smith"
grep_excel data.xml -x "SELECT * FROM data LIMIT 10"
```

> 运行 `grep_excel --help` 可查看与当前版本一致的格式列表与选项说明（中英文随 `LANG` 自动切换）。

---

## CLI 命令行模式

### 基本搜索

```bash
# 全文搜索（不区分大小写，子串匹配）
grep_excel data.xlsx -q "关键词"

# 精确搜索（区分大小写，完全匹配）
grep_excel data.xlsx -q "Engineering" -m exact

# 通配符搜索（% 匹配任意字符，_ 匹配单个字符）
grep_excel data.xlsx -q "张%" -m wildcard

# 正则搜索（支持多关键词 OR）
grep_excel data.xlsx -q "张三|李四" -m regex
```

### 搜索筛选

```bash
# 仅搜索指定列
grep_excel data.xlsx -q "关键词" -c "姓名"

# 仅搜索指定工作表
grep_excel data.xlsx -q "关键词" -s "Sheet1"

# 反向匹配：查找不包含关键词的行
grep_excel data.xlsx -q "已删除" -v
```

### SQL 查询

```bash
# 基本查询
grep_excel data.xlsx -x "SELECT * FROM sheet_1_0 LIMIT 10"

# 聚合统计
grep_excel data.xlsx -x "SELECT 部门, COUNT(*) as 人数 FROM sheet_1_0 GROUP BY 部门"

# 使用友好别名（先 --list-tables 查看可用表名）
grep_excel data.xlsx -t
grep_excel data.xlsx -x "SELECT * FROM data.Sheet1 WHERE 年龄 > 30"
```

### 聚合统计

```bash
# 统计匹配行中某列的值分布
grep_excel data.xlsx -q "工程部" -g 职位

# 输出示例：
# 聚合列 '职位': 工程师 (15), 经理 (3), 实习生 (8)
```

### 导出结果

```bash
# 导出为 CSV
grep_excel data.xlsx -q "关键词" -e results.csv

# 指定输出格式
grep_excel data.xlsx -q "关键词" -f json
grep_excel data.xlsx -q "关键词" -f pretty
grep_excel data.xlsx -q "关键词" -f simple  # TSV 格式
```

### 修复损坏文件

```bash
# 自动修复并导入
grep_excel corrupted.xlsx -q "数据" -r
```

### 查看已导入表

```bash
# 列出所有表及其列名
grep_excel data.xlsx employees.xlsx -t

# 输出示例：
# 可用表:
#   data.Sheet1 → sheet_1_0 (150 行) [姓名, 部门, 职位]
#   employees.Sheet1 → sheet_2_0 (200 行) [姓名, 工号, 薪资]
```

### 强制指定格式（--as）

扩展名缺失或具有误导性时，用 `--as` 显式指定解析格式。`--as` 是**粘性**选项：对命令行中其后的所有文件生效，直到下一个 `--as`；未跟在任何 `--as` 之后的文件仍按扩展名自动检测。

```bash
# access.log 按 CSV 解析，dump.dat 按 Excel 解析
grep_excel --as csv access.log --as excel dump.dat -t

# 无扩展名文件
grep_excel --as tsv exported_data -q "关键词"
```

可选值：`csv`、`tsv`、`html`、`txt`、`md`、`dbf`、`xml`、`excel`、`docx`、`pptx`。

---

## 归档文件与云文档链接导入

### 归档文件

直接传入归档文件，grep-excel 自动提取内部所有可识别的表格文件并逐个导入，条目以 `archive::路径/文件名` 命名：

```bash
# 搜索 ZIP 中的表格文件
grep_excel audit_2026.zip -q "异常交易"

# 查询 tar.gz 中的 CSV
grep_excel db_dump.tar.gz -x "SELECT * FROM sheet_1_0 LIMIT 10"

# 分卷 ZIP（传入第一卷即可）
grep_excel big_data.zip.001 -t
```

支持的归档格式：`.zip`、`.tar`、`.tar.gz` / `.tgz`、`.tar.bz2`、`.tar.xz`、`.tar.zst`、分卷 `.zip.001` / `.zip.002`。

> 需要 `archive-support` feature（`--features full` 已包含）。

### 云文档链接导入

直接传入金山文档 / WPS（kdocs.cn）分享链接，通过登录 Cookie 下载：

```bash
export KDOCS_COOKIE='wps_sid=...; ...'
grep_excel 'https://www.kdocs.cn/l/xxxx' -q "关键词"

# 或直接传入 Cookie
grep_excel --kdocs-cookie "$KDOCS_COOKIE" 'https://www.kdocs.cn/l/xxxx' -t
```

企业版域名：设置 `SHARE_HOSTS` 环境变量（逗号分隔），或使用 `--share-hosts` 选项。

> 需要 `share-url` feature（`--features full` 已包含）。

---

## 交互式 SQL REPL（-i）

`-i` / `--interactive` 启动一个基于 rustyline 的多行 SQL 交互式 shell，适合需要反复执行 SQL 查询的场景。

### 启动

```bash
# 预导入文件后进入 REPL
grep_excel data.xlsx employees.xlsx -i

# 也可不预导入，进入后用 --exec 或 .tables 查看状态
grep_excel -i
```

### 基本用法

REPL 使用 `$` 作为主提示符。输入 SQL 语句后以 `;` 结尾即可执行：

```
$ SELECT * FROM sheet_1_0 LIMIT 5;
$ SELECT 部门, COUNT(*) FROM sheet_1_0 GROUP BY 部门;
```

**多行输入**：未以 `;` 结尾的输入会自动进入续行模式（提示符变为 `>`），可以跨多行编写长查询：

```
$ SELECT 姓名, 部门
> FROM sheet_1_0
> WHERE 薪资 > 10000
> ORDER BY 薪资 DESC;
```

### 点命令

REPL 支持以下以 `.` 开头的元命令：

| 命令 | 功能 |
|------|------|
| `.tables` / `.schema` | 列出已导入的表及其友好别名、列名 |
| `.files` | 列出已导入的文件 |
| `.output <文件>` | 将后续 SQL 结果持续重定向到文件（CSV）；终端不再打印表格 |
| `.output` | 关闭重定向，恢复终端输出 |
| `.save <文件> [fmt]` | 将**上次** SQL 结果一次性保存到文件；`fmt` 可选 `csv`（默认）、`json`、`tsv`、`table` |
| `.help` | 显示帮助信息 |
| `.history` | 查看命令历史 |
| `.clear` / `.cls` | 清屏 |
| `.exit` / `.quit` | 退出 REPL |

导出示例：

```
$ SELECT * FROM sheet_1_0 WHERE 部门 = '工程部';
$ .save eng.csv
$ .save eng.json json
$ .output full.csv
$ SELECT * FROM sheet_1_0;
$ .output
```

> 若上次结果因终端显示限制被截断，`.save` 会提示改用 `.output <文件>` 以导出完整数据（写入文件时不做行数截断）。

### 退出方式

- 输入 `.exit` 或 `.quit`
- 按 `Ctrl+D`（EOF）
- 按 `Ctrl+C` 不会退出，只会取消当前输入行

### 结果展示

查询结果以 Unicode 对齐的表格形式输出，列宽自动适配（最长 40 字符截断），并显示行数统计：

```
  姓名   │ 部门    │ 薪资
  ────────┼─────────┼──────────
  张三    │ 工程部  │ 15000
  李四    │ 市场部  │ 12000

  共 2 行 (用时 3ms)
```

> **提示**：REPL 单次查询在终端最多显示约 1000 行；使用 `.output` 可导出完整结果。命令历史跨会话持久保存到 `~/.local/state/grep-excel/history.txt`（macOS：`~/Library/Application Support/grep-excel/history.txt`），最多 500 条，可用上下方向键浏览过往会话的输入。传入 `--no-history` 可关闭本次会话的持久化。

---

## TUI 交互模式

不带任何查询参数运行即可进入 TUI 交互模式：

```bash
grep_excel
```

带文件启动：

```bash
grep_excel data.xlsx employees.xlsx
```

### 界面概览

```
┌─────────────────────────────────────────────────┐
│ grep-excel │ [普通] │ 2 个文件                  │ ← 标题栏
├─────────────────────────────────────────────────┤
│ 全部(2) │ data:员工 │ emp:Sheet1                │ ← 标签（多文件时带 file:sheet）
├─────────────────────────────────────────────────┤
│ [搜索________] [全文] [列___] [聚合___]         │ ← 搜索栏
├─────────────────────────────────────────────────┤
│ │ 来源        │ 姓名  │ 部门   │ 职位  │       │ ← 结果（全部标签用「来源」列）
│ │ data:员工   │ 张三  │ 工程部 │ 工程师│       │
│ │ data:员工   │ 李四  │ 工程部 │ 经理  │       │
├─────────────────────────────────────────────────┤
│ 找到 2 个匹配 / 150 行, 用时 0.05s             │ ← 状态栏
├─────────────────────────────────────────────────┤
│ /搜索 c列 Tab模式 o打开 ?帮助 s导出 q退出      │ ← 提示栏
└─────────────────────────────────────────────────┘
```

### 自动浏览（Auto-browse）

带文件启动或通过 `o` 导入后，TUI **自动加载并显示首个 sheet 的数据**，无需先输入搜索词。可直接浏览、滚动，再用 `/` 搜索或 `S` 执行 SQL。

### 快捷键

#### 导航

| 按键 | 功能 |
|------|------|
| `j` / `↓` | 下移一行 |
| `k` / `↑` | 上移一行 |
| `g` | 跳转到顶部 |
| `G` | 跳转到底部 |
| `←` / `→` | 左右滚动列 |
| `H` / `L` | 左右滚动列（vim 风格） |
| `[` / `]` | 上一个 / 下一个 Sheet（浏览模式，跨文件） |
| `Ctrl+←` / `Ctrl+→` | 在同一文件内切换 Sheet（浏览 / 平铺 / 表格视图均可用） |
| `Ctrl+↑` / `Ctrl+↓` | 切换文件 |
| `1`–`9` | 切换标签页（浏览模式下跳到第 N 个 Sheet） |

#### 搜索

| 按键 | 功能 |
|------|------|
| `/` 或 `e` | 输入搜索关键词 |
| `c` | 设置列过滤器 |
| `a` | 设置聚合统计列 |
| `Tab` | 循环切换搜索模式（全文 → 精确 → 通配符 → 正则） |
| `Enter` | 执行搜索 |
| `n` | 加载更多结果（搜索截断时；浏览模式下再加载 500 行） |

#### 文件与数据

| 按键 | 功能 |
|------|------|
| `o` | 打开文件选择器 / 查看已加载文件 |
| `s` | 导出当前结果为 CSV |
| `d` | 清除所有数据 |
| `v` | 切换平铺/表格视图 |
| `Enter` | 打开/关闭详情面板 |

#### SQL 模式

| 按键 | 功能 |
|------|------|
| `S` | 进入 SQL 查询模式 |

#### 其他

| 按键 | 功能 |
|------|------|
| `?` | 显示帮助（含 Ctrl+方向键说明） |
| `q` | 退出 |

### SQL 查询（TUI 内）

1. 按 `S` 进入 SQL 模式（搜索栏变为 SQL 输入框）
2. 输入 SQL 查询，例如：
   ```sql
   SELECT 部门, COUNT(*) as 人数 FROM sheet_1_0 GROUP BY 部门
   ```
3. 按 `Enter` 执行
4. 结果在表格中显示
5. 按 `d` 清除 SQL 结果返回正常搜索模式

### 平铺视图与来源列

按 `v` 可在表格视图和平铺视图之间切换。

- **表格视图**：结果在表格中展示；「全部」标签使用单一 **来源** 列（`文件:sheet`），单 sheet 标签不再重复显示文件/Sheet 列
- **平铺视图**：每个工作表独立成块，块标题含来源信息；可用 `Ctrl+方向键` 在文件/Sheet 间切换

---

## MCP 服务器模式

MCP（Model Context Protocol）允许 AI 助手（如 Claude、Cursor）直接调用 grep-excel 的功能。

### 启动 MCP 服务器

```bash
grep_excel --mcp
```

### 配置 AI 助手

#### Claude Desktop

编辑 `claude_desktop_config.json`（位置见 [Claude 文档](https://docs.anthropic.com/en/docs/claude-desktop)）：

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/usr/local/bin/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

#### Cursor

在 Cursor 设置 → MCP 中添加：

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/usr/local/bin/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

### 可用工具

| 工具 | 说明 | 主要参数 |
|------|------|----------|
| `import_file` | 导入表格文件（Excel/CSV/HTML/文本/Markdown） | `file_path` |
| `list_files` | 列出已导入文件 | 无 |
| `get_metadata` | 获取文件元数据（列名等） | `file_name`（可选） |
| `get_sheet_sample` | 均匀采样行 | `file_name`, `sheet_name`, `sample_size` |
| `get_sheet_data` | 分页获取行数据 | `file_name`, `sheet_name`, `start_row`, `end_row`, `columns` |
| `search` | 搜索（支持上下文行、多条件筛选） | `query`, `column`, `sheet`, `mode`, `limit`, `aggregate`, `invert`, `context_lines`, `conditions` |
| `execute_sql` | 执行 SQL | `sql`, `limit` |
| `export_query` | 执行 SQL 并导出结果到 .xlsx | `sql`, `output_path`, `sheet_name` |
| `get_sheet_statistics` | 按列统计（空值/去重计数/Top 值） | `file_name`, `sheet_name`, `max_top_values` |
| `save_as` | 另存为新文件 | `file_name`, `output_path`, `sheet_name` |
| `save` | 覆盖保存原文件 | `file_name`, `sheet_name` |
| `update_cell` | 更新单个单元格 | `file_name`, `sheet_name`, `row`, `column`, `value` |
| `update_cells` | 批量更新单元格 | `file_name`, `sheet_name`, `updates` |
| `insert_rows` | 插入行 | `file_name`, `sheet_name`, `start_row`, `rows` |
| `delete_rows` | 删除行 | `file_name`, `sheet_name`, `start_row`, `count` |
| `add_column` | 添加列 | `file_name`, `sheet_name`, `column_name`, `default_value` |
| `rename_column` | 重命名列 | `file_name`, `sheet_name`, `old_name`, `new_name` |

### 典型 MCP 工作流

#### 探索未知文件

```
你：导入 data.xlsx 并告诉我里面有什么
AI：→ import_file → 显示文件名、工作表数和行数
AI：→ get_metadata → 显示每个工作表的列名
AI：→ get_sheet_sample → 显示 5 行均匀采样数据
```

#### 数据分析

```
你：分析销售数据，按地区统计销售额
AI：→ import_file → 导入 sales.xlsx
AI：→ execute_sql → SELECT 地区, SUM(销售额) FROM sales.Sheet1 GROUP BY 地区
```

#### 数据画像（get_sheet_statistics）

```
你：这个文件的数据质量怎么样？有没有空值？
AI：→ import_file → 导入 data.xlsx
AI：→ get_sheet_statistics → 每列的空值数、去重计数、Top 5 高频值
     → "薪资列: 200 行中 3 行为空, 45 个不同值, 最高频: 10000(15次)"
```

#### 多条件搜索 + 上下文（search 增强）

```
你：找出薪资大于 12000 且部门是工程部的记录，附带前后 2 行上下文
AI：→ search → query="工程部", conditions=[{column:"薪资", operator:">", value:"12000"}], context_lines=2
     → 每个匹配结果额外返回前 2 行和后 2 行
```

#### 过滤导出（export_query）

```
你：把北京地区的高薪员工导出到新文件
AI：→ execute_sql → SELECT * FROM data.Sheet1 WHERE 城市='北京' AND 薪资 > 15000
AI：→ export_query → sql="SELECT * FROM data.Sheet1 WHERE 城市='北京' AND 薪资 > 15000", output_path="beijing_high.xlsx"
     → 直接生成 .xlsx 文件
```

#### 编辑并保存

```
你：把第 3 行的部门从 "工程部" 改成 "研发部"
AI：→ update_cell → 修改单元格

你：保存
AI：→ save → 覆盖原文件
```

---

## --exec 执行模式

`--exec` 允许在命令行中直接执行 MCP 工具，无需启动 MCP 服务器。支持单条命令和多步流水线。

### 单条命令

```bash
# 查看帮助
grep_excel --exec help

# 导入并查看文件
grep_excel data.xlsx --exec '{"tool":"import_file","params":{"file_path":"data.xlsx"}}'

# 搜索
grep_excel data.xlsx --exec '{"tool":"search","params":{"query":"张三","mode":"exact"}}'

# 搜索 + 聚合
grep_excel data.xlsx --exec '{"tool":"search","params":{"query":"工程部","aggregate":"职位"}}'

# 执行 SQL
grep_excel data.xlsx --exec '{"tool":"execute_sql","params":{"sql":"SELECT * FROM sheet_1_0 LIMIT 5"}}'
```

### 多步流水线

使用 JSON 数组，命令按顺序执行，状态共享：

```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"data.xlsx"}},
  {"tool":"get_metadata","params":{}},
  {"tool":"search","params":{"query":"张三","mode":"exact"}},
  {"tool":"update_cell","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"姓名","value":"李四"}},
  {"tool":"save","params":{"file_name":"data.xlsx"}}
]'
```

### 输出格式

```bash
# JSON 格式（默认）
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}'

# Markdown 表格
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}' -f markdown

# 简单 TSV 格式
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}' -f simple
```

---

## --run Shell 命令模式

`--run` 允许对每个匹配行执行外部 shell 命令，使用 `${列名}` 引用单元格值。

### 基本用法

```bash
# 对每个匹配行执行命令
grep_excel <文件> -q <查询> --run '<命令>'
```

命令模板中使用 `${列名}` 占位符，值自动 shell 转义（单引号包裹）。`$$` 表示字面 `$`。

### 示例

```bash
# 对每个匹配行执行外部分析工具
grep_excel data.xlsx -q "ERROR" -c "级别" --run './analyzer "${内容}"'

# 将命令输出写入新列，然后导出
grep_excel data.xlsx -q "TODO" -c "类型" --run './classifier "${标题}"' --run-output-column "分类" -e output.xlsx

# 配合 SQL 查询使用
grep_excel data.xlsx --sql "SELECT 姓名, SQL FROM sheet_1_0 WHERE 类型='旧版'" --run './formatter "${SQL}"'
```

### 相关选项

| 选项 | 说明 |
|------|------|
| `--run-output-column` | 将命令 stdout 写入该列（列不存在则自动创建） |
| `--export` | 处理完成后导出完整 Excel 文件（需要 mcp-server feature） |

> **注意**：`--run` 必须配合 `--query`（`-q`）或 `--sql`（`-x`）使用。命令通过 `sh -c` 执行。

---

## 搜索模式详解

### fulltext（全文搜索）— 默认

不区分大小写的子串匹配。最适合一般性搜索。

```
查询: "john"
匹配: "John Smith", "Johnson", "JOHN", "john@example.com"
不匹配: "Jon"
```

### exact（精确匹配）

区分大小写的完全匹配。整个单元格内容必须完全等于查询文本。

```
查询: "Engineering"
匹配: "Engineering" (完全一致)
不匹配: "engineering", "Engineering Dept"
```

### wildcard（通配符）

SQL LIKE 风格的模式匹配。不区分大小写。

| 通配符 | 含义 |
|--------|------|
| `%` | 匹配任意字符序列（包括空字符串） |
| `_` | 匹配恰好一个字符 |

```
查询: "San%"  → 匹配 "San Francisco", "San Jose", "San"
查询: "A__"   → 匹配 "ABC", "Amy"（恰好 3 个字符）
查询: "%公司"  → 匹配 "科技有限公司", "贸易公司"
```

### regex（正则表达式）

正则表达式匹配。不区分大小写。支持完整 Rust 正则语法。

```
查询: "张三|李四"                    → 匹配包含任一关键词的单元格
查询: "\d{4}-\d{2}-\d{2}"           → 匹配日期格式 2024-01-15
查询: "^[A-Z]{3}-\d{3}$"            → 匹配编号格式 ABC-123
查询: "(error|warning|critical)"    → 匹配日志级别
```

---

## SQL 查询指南

### 表名规则

导入的工作表存储在数据库中，命名规则为：

- **内部表名**：`sheet_{文件ID}_{工作表索引}`（如 `sheet_1_0`、`sheet_2_0`）
- **友好别名**：`文件名.工作表名`（如 `data.Sheet1`、`employees.员工表`）

先使用 `--list-tables` 或 MCP `list_files` 查看可用表名。

### 基础查询

```sql
-- 查看前 10 行
SELECT * FROM sheet_1_0 LIMIT 10

-- 筛选列
SELECT 姓名, 部门 FROM sheet_1_0

-- 条件筛选
SELECT * FROM sheet_1_0 WHERE 年龄 > 30

-- 排序
SELECT * FROM sheet_1_0 ORDER BY 薪资 DESC
```

### 聚合分析

```sql
-- 计数
SELECT 部门, COUNT(*) as 人数 FROM sheet_1_0 GROUP BY 部门

-- 求和
SELECT 部门, SUM(薪资) as 总薪资 FROM sheet_1_0 GROUP BY 部门

-- 平均值
SELECT 部门, AVG(薪资) as 平均薪资 FROM sheet_1_0 GROUP BY 部门
```

### 跨表查询（JOIN）

```sql
-- 先导入两个文件，再用友好别名查询
SELECT e.姓名, e.部门, d.部门负责人
FROM employees.Sheet1 e
JOIN departments.Sheet1 d ON e.部门 = d.部门名称
```

### DuckDB 特有函数

使用 DuckDB 引擎时（`--features engine-duckdb`），可以使用：

```sql
-- 不区分大小写匹配
SELECT * FROM sheet_1_0 WHERE 姓名 ILIKE '%张%'

-- 正则匹配
SELECT * FROM sheet_1_0 WHERE regexp_matches(邮箱, '.*@company\.com')

-- 类型转换
SELECT 姓名, 薪资::INTEGER * 12 as 年薪 FROM sheet_1_0

-- 窗口函数
SELECT 姓名, 薪资, RANK() OVER (PARTITION BY 部门 ORDER BY 薪资 DESC) as 排名
FROM sheet_1_0
```

---

## 文件编辑

grep-excel 支持通过 MCP 或 `--exec` 模式编辑 Excel 文件。

### 单元格编辑

```bash
# 更新单个单元格
grep_excel data.xlsx --exec '{"tool":"update_cell","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","row":0,"column":"姓名","value":"张三"}}'

# 批量更新
grep_excel data.xlsx --exec '{"tool":"update_cells","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","updates":[{"row":0,"column":"姓名","value":"张三"},{"row":1,"column":"部门","value":"研发部"}]}}'
```

### 行列操作

```bash
# 在第 5 行前插入新行
grep_excel data.xlsx --exec '{"tool":"insert_rows","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","start_row":4,"rows":[["新员工","研发部","工程师"]]}}'

# 删除第 3-5 行（共 3 行）
grep_excel data.xlsx --exec '{"tool":"delete_rows","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","start_row":2,"count":3}}'

# 添加新列
grep_excel data.xlsx --exec '{"tool":"add_column","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","column_name":"状态","default_value":"在职"}}'

# 重命名列
grep_excel data.xlsx --exec '{"tool":"rename_column","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","old_name":"部门","new_name":"所属部门"}}'
```

### 保存

```bash
# 覆盖原文件
grep_excel data.xlsx --exec '{"tool":"save","params":{"file_name":"data.xlsx"}}'

# 另存为新文件
grep_excel data.xlsx --exec '{"tool":"save_as","params":{"file_name":"data.xlsx","output_path":"data_modified.xlsx"}}'
```

> **注意**：`save` 会覆盖原始文件，建议先用 `save_as` 另存备份。`save` 功能需要 `rust_xlsxwriter` feature flag。

> **只读格式**：`.docx` 和 `.pptx` 仅支持导入、搜索与查询导出（`export_query` / `--export`），所有编辑与保存类工具（`update_cell`、`update_cells`、`insert_rows`、`delete_rows`、`add_column`、`rename_column`、`save`、`save_as`）会直接报错拒绝。

---

## 输出格式

`--format` / `-f` 选项支持四种输出格式：

### markdown（默认）

```
| 文件 | Sheet | 姓名 | 部门 |
| --- | --- | --- | --- |
| data | Sheet1 | 张三 | 工程部 |
| data | Sheet1 | 李四 | 市场部 |
```

### pretty

带对齐的美化格式，适合终端直接阅读。

### json

完整 JSON 格式，适合程序化处理。

```json
{
  "results": [
    {
      "file_name": "data.xlsx",
      "sheet_name": "Sheet1",
      "row": ["张三", "工程部"],
      "col_names": ["姓名", "部门"]
    }
  ],
  "stats": {
    "total_matches": 2,
    "total_rows_searched": 150,
    "search_duration_ms": 12
  }
}
```

### simple

TSV（Tab-Separated Values）格式，适合导入其他工具处理。

```
文件	Sheet	姓名	部门
data	Sheet1	张三	工程部
```

---

## 常见问题

### Q: 支持哪些文件格式？

A: `.xlsx`、`.xls`、`.xlsm`、`.xlsb`、`.ods`、`.csv`、`.tsv`/`.tab`、`.html`/`.htm`、`.txt`、`.md`/`.markdown`、`.dbf`、`.xml`、`.docx`、`.pptx`，以及 `.zip`、`.tar` 系列归档和 `.zip.001` 分卷压缩包。HTML 与文本文件会自动检测编码；扩展名缺失或误导时可用 `--as` 强制指定格式；详见 [支持的文件格式](#支持的文件格式)。

### Q: 如何加速大文件搜索？

A: 使用 DuckDB 引擎（`--features engine-duckdb` 或 `--features full` 构建）。DuckDB 专为 OLAP 查询优化，处理百万行数据秒级响应。

### Q: MCP 模式下如何编辑文件？

A: 使用 `update_cell` / `update_cells` 修改数据后，必须调用 `save` 或 `save_as` 才能持久化。`save` 依赖 `rust_xlsxwriter` feature flag。

### Q: 如何修复损坏的 xlsx 文件？

A: 使用 `--repair` / `-r` 选项。grep-excel 会尝试在 ZIP/XML 层面恢复数据。如果常规导入失败会自动触发修复模式。

### Q: 中文搜索支持吗？

A: 完全支持。grep-excel 基于 UTF-8，中文搜索与英文搜索同样流畅。TUI 和 CLI 帮助文本会自动检测系统语言并显示中文或英文。

### Q: 如何设置中文界面？

A: 设置环境变量 `LANG=zh_CN.UTF-8` 即可。grep-excel 会在启动时自动检测。

### Q: Windows 上能使用吗？

A: 支持 Windows 7 及以上版本。需要启用 ANSI 转义序列支持的终端（如 Windows Terminal，不支持旧版 cmd.exe）。

### Q: 如何在 TUI 中执行 SQL？

A: 按 `S` 进入 SQL 模式，输入查询语句后按 `Enter`。先按 `o` 查看可用的表名。导入后会自动浏览首个 sheet；用 `Ctrl+←/→` 切换同文件 Sheet，`Ctrl+↑/↓` 切换文件。

### Q: REPL 如何导出查询结果？

A: 使用 `.save <文件> [csv|json|tsv|table]` 保存上次结果，或 `.output <文件>` 将后续查询持续写入 CSV，再用 `.output` 恢复终端输出。

### Q: --exec 和 --mcp 有什么区别？

A: `--exec` 是 CLI 一次性执行模式（执行完退出），`--mcp` 启动持久化 MCP 服务器（等待 AI 助手连接）。

### Q: DuckDB 引擎和 SQLite 引擎怎么选？

A: DuckDB 专为分析查询优化，适合聚合、JOIN、窗口函数等操作。SQLite 更轻量，适合简单查询。默认内存引擎无额外依赖，适合基本搜索。

### Q: Excel 中的日期显示为什么格式？

A: Excel 内部将日期存储为序列号（如 `46188` = 2026-06-15）。grep-excel 会**自动检测日期列**并将序列号转换为 ISO 8601 格式字符串：

- 纯日期列显示为 `2026-06-15`
- 包含时间的列显示为 `2026-06-15 14:30:00`
- 可以直接用日期片段搜索，如 `-q 06-15` 匹配 6 月 15 日，`-q 2026-06` 匹配 2026 年 6 月
- `--repair` 修复模式也会执行同样的日期转换

检测逻辑分两层：① 如果列中已有 `Data::DateTime` 类型单元格（高置信度）；② 如果列名包含日期相关关键词且超过 50% 的值为纯整数且在 Excel 序列号范围内（保守兜底检测）。数值列（如薪资）不受影响。
