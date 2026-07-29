# Design: OpenGauss Dump File Support

> **Status:** Draft
> **Date:** 2026-07-16

---

## 1. Problem Statement

grep-excel 当前支持 9 种表格文件格式（xlsx/xls/xlsm/xlsb/ods/csv/html/htm/txt/md），但不支持数据库 dump 文件。OpenGauss 的 `gs_dump` 工具生成纯 SQL 文本格式的数据库导出文件（`.sql`），其中包含 CREATE TABLE 和 COPY/INSERT 语句。用户希望能在 grep-excel 中直接搜索和查询这些 dump 文件。

**当前状态：**
- `.sql` / `.dmp` / `.dump` 扩展名不在 `parse_file()` 的分发列表中
- 无 SQL 解析模块
- 项目已有的 OpenGauss 关联仅限 WDR HTML 报告（测试 fixture 在 repo 外）

**目标：** 支持导入 `gs_dump -Fp`（纯 SQL 文本）输出文件，将每个数据库表转为可查询的 Sheet。

**非目标（MVP）：** `gs_dump -Fc` 自定义二进制格式、Oracle DMP 格式、`gs_dumpall` 输出。

---

## 2. Format Analysis

### 2.1 gs_dump -Fp 输出结构

```
-- header comments (数据库版本、工具版本)
SET ...;                          -- 会话配置（忽略）
CREATE SCHEMA ...;                -- Schema 定义（收集 schema 上下文）
CREATE TABLE public.employees (   -- 表定义 → 表名 + 列名
    id integer NOT NULL,
    name character varying(100),
    ...
);
COPY public.employees (...) FROM stdin;  -- 表数据
1	张三	Engineering	15000.00
2	李四	Marketing	12000.00
\.
ALTER TABLE ... ADD CONSTRAINT ...;  -- 约束（忽略）
```

关键特征：
- `--` 开头的是注释行，用作节标记
- `CREATE TABLE` 定义表结构（schema.table → 表名，列定义 → headers）
- `COPY ... FROM stdin;` ... `\.` 之间是 TSV 格式的行数据
- `INSERT INTO` 也可能出现（取决于导出选项）
- `ALTER TABLE`、`CREATE INDEX` 等 DDL 不在解析范围内

### 2.2 格式识别

三层策略（按优先级）：

| 优先级 | 机制 | 说明 |
|--------|------|------|
| 1 | `--file-format` CLI 选项 | 用户显式指定格式，覆盖自动检测 |
| 2 | 扩展名推断 | `.sql` → SQL dump |
| 3 | 内容嗅探（4KB 文件头） | 以 `--` SQL 注释或 `SET ` 开头 → SQL dump |

---

## 3. Architecture

### 3.1 Data Flow

```
.sql 文件
  │
  ▼
sql_dump::extract_tables(content: &str) -> Result<Vec<SqlTable>>
  │  sqlparser 解析 AST
  │  遍历语句:
  │    CREATE TABLE → 记录 (schema, table_name, columns)
  │    COPY ns.table (cols) FROM stdin → 关联到已知表，解析 TSV 行
  │    INSERT INTO ns.table (cols) VALUES (...) → 关联到已知表，解析值
  │
  ▼
Vec<SqlTable { schema, name, headers, rows }>
  │
  ▼
excel.rs::parse_file() → Vec<SheetData>
  │  表名 = "schema.table" 或友好名
  │  headers = Vec<String>
  │  rows = Vec<Vec<String>>
  │
  ▼
Engine 导入 → DuckDB/SQLite 表
```

### 3.2 Module Interface

```rust
// crates/core/src/sql_dump.rs

pub struct SqlTable {
    pub schema: String,
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn extract_tables(content: &str) -> Result<Vec<SqlTable>>;
```

### 3.3 Sheet Naming

为避免跨 dump 文件的表名冲突，使用 `schema.table` 格式作为 sheet 名称：
- `public.employees` → sheet name: `public.employees`
- `hr.staff` → sheet name: `hr.staff`

友好别名由 engine 层自动生成：`{file_stem}.public.employees`

### 3.4 File Format CLI Option

新增 `--file-format` 选项：

```bash
grep_excel export.dmp --file-format opengauss-sql -t
grep_excel data.sql --file-format opengauss-sql -q "张三"
```

实现：在 `parse_file()` 和其他分发函数中，`--file-format` 比扩展名优先。支持的值：`opengauss-sql`（MVP）。

### 3.5 Zero-extension Files

无扩展名文件：内容嗅探 + `--file-format` 兜底。

---

## 4. Goals & Non-Goals

### 4.1 Goals (MVP)

1. 解析 `gs_dump -Fp` 纯 SQL 文本输出中的 CREATE TABLE + COPY 数据
2. 也支持 INSERT INTO 格式（备选数据格式）
3. 通过 `--file-format` 显式指定格式，处理无扩展名或歧义扩展名
4. 每个数据库表成为一个 Sheet，通过 TUI/CLI/MCP 可查
5. 保留 `schema.table` 命名，避免跨 dump 表名冲突
6. 测试 fixture 内置在 repo 中

### 4.2 Non-Goals (MVP)

- `gs_dump -Fc` 自定义二进制格式
- `gs_dumpall` 全局 dump 输出
- `INSERT INTO` 中的表达式或子查询（仅支持 VALUES 字面量）
- `COPY` 的 CSV 格式变体（仅 TSV）
- DDL 语句（ALTER TABLE、CREATE INDEX 等）的解析
- Oracle DMP 格式

### 4.3 Success Criteria

```bash
# 搜索整个 dump
grep_excel tests/fixtures/opengauss/sample_dump.sql -q "张三"
# → 返回 public.employees 中匹配的行

# 跨表 JOIN
grep_excel sample_dump.sql -x "
  SELECT e.name, d.name as dept
  FROM \"public.employees\" e
  JOIN \"public.departments\" d ON e.department = d.name
"

# --file-format 显式指定
grep_excel export_no_ext --file-format opengauss-sql -t

# TUI 浏览
grep_excel sample_dump.sql
# → 自动打开 TUI，Ctrl+箭头在表之间切换
```

---

## 5. Implementation Tasks

### Task 1: Add `sqlparser` dependency

**Files:** `crates/core/Cargo.toml`

```toml
sqlparser = "0.53"
```

### Task 2: Create `sql_dump` module

**Files:**
- New: `crates/core/src/sql_dump.rs`

**Steps:**

1. Define `SqlTable` struct
2. Define `ParseState` struct（track: current schema, known tables with columns, collected rows）
3. Implement `extract_tables(content: &str) -> Result<Vec<SqlTable>>`:
   - Use `sqlparser::parser::Parser::parse_sql()` to parse SQL statements
   - On `Statement::CreateTable`: extract schema + table name, column names → register in state
   - On `Statement::Copy { to: false, target, columns, .. }`: extract table name from `target`, match to known table, parse TSV data lines until `\.`
   - On `Statement::Insert { table_name, columns, source }`: extract values from `source`, append rows
   - Skip: SET, CREATE SCHEMA, ALTER TABLE, CREATE INDEX, comments
4. Handle `public.table` qualified names
5. Unit tests: parse a minimal SQL dump string and verify extracted tables

### Task 3: Handle COPY data in SQL dump

**Files:** `crates/core/src/sql_dump.rs`

COPY 的特殊性：`COPY ... FROM stdin;` 后面的数据不在 SQL 语法内，sqlparser 不会解析。需要特殊处理：

```
COPY public.employees (id, name, salary) FROM stdin;
1	张三	15000.00    ← TSV 格式，不在 SQL 语句内
2	李四	12000.00
\.
```

策略：当检测到 COPY 语句后，切换到"行模式"，逐行读取直到 `\.`，按 TSV 分割。

`parse_file()` 目前一次读取整个文件。对于 COPY 数据，我们需要在 `extract_tables()` 中同时遍历 SQL 语句和原始文本行。

实现方案：双模式解析器
- 先对内容逐行扫描
- 遇到 `COPY ... FROM stdin;` → 后面的行收集为 TSV 数据，直到 `\.`
- 其余行拼接后交给 sqlparser 解析

### Task 4: Register module and wire dispatch

**Files:**
- `crates/core/src/lib.rs` — add `pub mod sql_dump;`
- `crates/core/src/excel.rs` — add SQL branch in 4 dispatch functions

In `parse_file()` (line 77):
```rust
} else if ext == "sql" || ext == "dump" || ext == "dmp" {
    parse_sql_dump(path)
} else if ext == "csv" {
```

Same pattern in `parse_file_metadata()`, `for_each_sheet()`, `parse_file_repair()`.

Implement `parse_sql_dump()`:
```rust
fn parse_sql_dump(path: &Path) -> Result<Vec<SheetData>> {
    let content = read_file_auto_encoding(path)?;
    let tables = sql_dump::extract_tables(&content)
        .map_err(|e| anyhow!("Failed to parse SQL dump: {}", e))?;
    Ok(tables.into_iter().map(|t| SheetData {
        name: format!("{}.{}", t.schema, t.name),
        headers: t.headers,
        rows: t.rows,
        col_widths: Vec::new(),
    }).collect())
}
```

### Task 5: Add `--file-format` CLI option

**Files:**
- `crates/cli/src/main.rs` — add clap argument
- `crates/core/src/excel.rs` — thread format hint through parse functions

```rust
// clap
#[arg(long = "file-format", help = "Force file format: opengauss-sql")]
file_format: Option<String>,
```

Thread through to `parse_file()`, `for_each_sheet()` etc. as an optional parameter. When set to `"opengauss-sql"`, skip extension matching and use `parse_sql_dump()` directly.

### Task 6: Add content sniffing for extensionless files

**Files:** `crates/core/src/sql_dump.rs`

```rust
pub fn looks_like_sql_dump(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    head.trim_start().starts_with("--") 
        || head.contains("SET ")
        || head.contains("CREATE TABLE")
}
```

Used in `parse_file()` when no extension matches and no `--file-format` given.

### Task 7: Add test fixtures and integration tests

**Files:**
- New: `tests/fixtures/opengauss/sample_dump.sql` (already created)
- New: `crates/core/tests/sql_dump_test.rs`

Tests:
1. `test_extract_table_names` — verify 3 tables found
2. `test_extract_headers` — verify column names for employees table
3. `test_extract_row_count` — verify row counts
4. `test_parse_file_end_to_end` — `parse_file()` on the fixture returns correct SheetData
5. `test_cross_table_join` — engine-level test: import dump, run cross-table SQL

### Task 8: Update documentation

**Files:**
- `README.md` — Supported Formats 加 `.sql` (OpenGauss dump)
- `docs/UserGuide.md` — 格式表加 `.sql`，加使用示例
- `Desktop/frontend/api/commands.ts` — file dialog extensions 加 `sql`

### Task 9: Relocate WDR test fixtures into repo

**Files:**
- Move: `/Users/c2j/Projects/Desktop_Projects/DB/WDRProbe/example/*.html` → `tests/fixtures/opengauss/`
- Update: `crates/core/tests/html_wdr_test.rs` — change `WDR_DIR` to relative path

---

## 6. Verification

### 6.1 Unit Tests
- [ ] `sql_dump::extract_tables()` on minimal SQL dump → correct table count, headers, rows
- [ ] Empty file → empty Vec, no panic
- [ ] Dump with no COPY data (DDL only) → tables with headers but 0 rows
- [ ] Dump with INSERT instead of COPY → correct data extraction
- [ ] `looks_like_sql_dump()` on valid SQL, binary garbage, and empty input

### 6.2 Integration Tests
- [ ] `parse_file("sample_dump.sql")` → 3 sheets with correct data
- [ ] Import into DuckDB engine → cross-table JOIN works
- [ ] `--list-tables` shows all 3 tables with schema-qualified names
- [ ] TUI: Ctrl+arrow switches between tables
- [ ] MCP: `import_file` → `search` → returns correct results

### 6.3 Regression
- [ ] All existing tests pass
- [ ] Existing format dispatch unchanged
- [ ] `--file-format` does not affect non-dump files

---

## 7. Open Questions

1. **INSERT INTO 格式细节**：gs_dump 在什么条件下生成 INSERT 而非 COPY？需要验证 `sqlparser` 对 VALUES 子句的解析能力。
2. **大文件性能**：COPY 数据可能非常大（GB 级别）。当前方案先读入内存再解析，是否需要流式处理？
3. **`--file-format` 的实现粒度**：应该作为全局选项还是与文件一一对应（支持 `file1 --file-format x file2 --file-format y`）？

---

## 8. Risk Assessment

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| sqlparser 不支持 OpenGauss 特定语法 | 低 | 中 | OpenGauss SQL 是 PG 超集，sqlparser 对 PG 方言支持良好 |
| COPY 数据格式变体（CSV模式） | 低 | 低 | MVP 仅支持 TSV 默认格式 |
| 大 dump 文件内存溢出 | 中 | 中 | Phase 2 考虑流式解析；当前先限制 |
| 字符编码问题 | 低 | 低 | 已有 `read_file_auto_encoding` 链路 |
