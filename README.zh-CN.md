[English](README.md)

# grep-excel

**给表格文件用的 grep。** 把 Excel、CSV、HTML 报告、Word 文档、ZIP 压缩包当成表格文本搜。需要的时候，直接上 SQL。

我平时经常要看多个sheet的excel文件、数据库诊断报告（AWR、WDR 之类的 HTML 文件），一份文件/报告里几十张表，想搜个关键词得一个个点开。Excel 搜多了也烦。后来干脆写了这个工具：不管什么格式，丢进去就能搜，还能用 SQL 做分析——DuckDB 在底下扛着，几百万行也不卡。

## 能干什么

- **搜表格像搜文本一样简单。** 全文搜索、精确匹配、通配符、正则都行。`grep_excel data.xlsx -q "关键词"` 就完了。
- **SQL 直接查。** 导入的表格就是数据库表，JOIN、GROUP BY、窗口函数随便用。DuckDB 引擎，分析型的 SQL 比 SQLite 强不少。
- **格式通吃。** Excel / CSV / TSV / HTML / Markdown / Word / PowerPoint / DBF / XML / PDF / Parquet 全都认。ZIP 和 tar.gz 压缩包直接丢进来，里面的表格文件自动解出来。
- **TUI 终端界面。** 不习惯命令行的可以用键盘操作，导入自动浏览、Tab 分页、详情面板、平铺/表格视图切换都有。
- **MCP 接入 AI 助手。** 起个 `--mcp` 就能让 Claude、Cursor 直接操作表格——搜索、统计、改数据、导出都行。
- **中文友好。** 根据系统语言自动切中英文。编码检测做了 CJK 回退，GBK 的文件不用手动转码。

## 不是什么

- 不是 Excel 代替品。你如果要做图表、数据透视表、条件格式，老老实实用 WPS 或 Office。
- 不是 OCR 工具。PDF 只能提取文本型表格，扫描件不行。
- 不是数据库。数据量上了亿行，还是导入到正经数据库里处理更合适。
- 性能看你用什么引擎。默认内存引擎简单够用，DuckDB 引擎更快但编译麻烦（下面会说）。

## 长什么样

进 TUI 模式直接输 `grep_excel`，不传任何参数：

![TUI 截图](https://github.com/c2j/grep-excel/raw/main/docs/tui.png)

## 怎么装

### 下载二进制

去 [Releases](https://github.com/c2j/grep-excel/releases) 页面找对应平台的包：

- **Windows** — `grep_excel-windows-x86_64.zip`
- **macOS Intel** — `grep_excel-macos-x86_64.zip`
- **macOS Apple Silicon** — `grep_excel-macos-aarch64.zip`
- **Linux x86_64** — `grep_excel-linux-x86_64.zip`
- **Linux ARM64** — `grep_excel-linux-aarch64.zip`

解压就能用。

### 从源码编译

需要 Rust 1.70+ 和 Cargo。

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# 默认编译（内存引擎，快）
cargo build --release

# 推荐：全功能编译（DuckDB + MCP + 归档支持等）
cargo build --release --features full
```

> DuckDB 引擎编译比较慢，设置环境变量 `DUCKDB_DOWNLOAD_LIB=1` 能下载预编译库，快很多。

其他 feature 组合（SQLite 引擎、无头编译等）看 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 用法

### 最简单的

```bash
# 搜一个文件
grep_excel data.xlsx -q "张三"

# 搜多个文件，只查 Name 列
grep_excel a.xlsx b.xlsx -c Name -m exact

# 反向搜：找不包含 "正常" 的行
grep_excel data.xlsx -q "正常" -v
```

### 实际场景

```bash
# 场景1：老板扔了个 sales-2026.zip，里面 50 个 Excel。
# 找出所有提到"退款"的行，按部门统计：
grep_excel sales-2026.zip -q 退款 -g 部门

# 场景2：看 AWR 报告的 CPU 使用情况
grep_excel awr_report.html -x "SELECT * FROM \"CPU Load\" WHERE \"%Busy\" > 80"

# 场景3：导出一批数据的统计结果
grep_excel 全国销售.xlsx -q 华东 -g 城市 -e 华东统计.csv

# 场景4：对匹配行执行外部脚本
grep_excel data.xlsx -q ERROR -c Level --run './analyzer "${Message}"'
```

### TUI 模式

不带参数直接启动：

```bash
grep_excel
```

导入文件后自动浏览第一个 sheet。主要快捷键：

| 按键 | 干什么 |
|------|--------|
| `/` 或 `e` | 输入搜索关键词 |
| `S` | SQL 查询模式 |
| `Tab` | 切换搜索模式 |
| `v` | 平铺/表格视图切换 |
| `Ctrl+←` / `Ctrl+→` | 切换 Sheet |
| `Ctrl+↑` / `Ctrl+↓` | 切换文件 |
| `?` | 帮助 |

完整快捷键列表见 [UserGuide](docs/UserGuide.zh-CN.md)。

### SQL REPL

```bash
grep_excel data.xlsx employees.xlsx -i
```

进去后就是 SQL 交互环境，支持 `.tables`、`.files`、`.output`、`.save` 等点命令，历史记录会自动保存。

### MCP 模式

```bash
grep_excel --mcp
```

在 AI 助手的 MCP 配置里加上：

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

之后 Claude / Cursor 就能直接搜表格、统计数据、修改单元格、导出文件了。[UserGuide](docs/UserGuide.zh-CN.md) 里有完整工作流示例。

## 支持的文件格式

| 格式 | 扩展名 | 能改吗 |
|------|--------|--------|
| Excel 2007+ | `.xlsx` | ✅ |
| Excel 97-2004 | `.xls` | ✅ |
| CSV / TSV | `.csv` `.tsv` `.tab` | ✅ |
| HTML 表格 | `.html` `.htm` | ✅ |
| Markdown 管道表 | `.md` | ✅ |
| 纯文本表格 | `.txt` | ✅ |
| Word 文档 | `.docx` | ❌ 只读 |
| PowerPoint | `.pptx` | ❌ 只读 |
| dBase 数据库 | `.dbf` | ✅ |
| XML 数据 | `.xml` | ✅ |
| PDF | `.pdf` | ❌ 只读 |
| Parquet | `.parquet` | ❌ 只读 |
| ZIP / TAR 压缩包 | `.zip` `.tar.gz` 等 | — 自动解压 |

## 更多

- [UserGuide.zh-CN.md](docs/UserGuide.zh-CN.md) — 完整使用手册
- [DeveloperGuide.md](docs/DeveloperGuide.md) — 架构说明，SearchEngine trait，如何加新引擎
- [CONTRIBUTING.md](CONTRIBUTING.md) — 贡献指南，PR checklist，feature flags 说明

## 协议

MIT License — 详见 [LICENSE](LICENSE)
