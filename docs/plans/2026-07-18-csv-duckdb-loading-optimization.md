# CSV DuckDB 加载性能优化

**日期**: 2026-07-18
**状态**: Momus 审核通过，开始实现

## 背景

用户场景：3800 万行、20 列、11GB 的 CSV 文件，列上有重复数据。需要优化 grep-excel 在 CLI 一次性查询（`-q`/`--sql`/`--exec`）和 TUI 交互模式（`-i`）下对大型 CSV 的加载和搜索速度。

## 问题分析

### 调用链

CSV 加载有两个入口，选择取决于文件扩展名（`ext == "csv"`）：

| 方法 | 位置 | 行为 |
|------|------|------|
| `import_csv_direct` | duckdb.rs:1435 | `CREATE TABLE AS SELECT * FROM read_csv_auto(...)` — 直接物化 |
| `register_csv_virtual` | duckdb.rs:1056 | `CREATE VIEW AS SELECT * FROM read_csv_auto(...)` — 延迟视图 |

CLI/TUI 的上层路由由 `quick_register`（main.rs:297）控制：

```rust
fn quick_register(db, file, repair) {
    if ext == "csv" && !repair {
        db.register_virtual(file, ...)   // VIEW
    } else {
        import_file_with_repair(...)      // TABLE
    }
}
```

`quick_register` 有 10+ 个调用点（lines 364, 392, 572, 600, 1013, 1042, 1124, 1152, 1449, 1481），覆盖 CLI 一次性模式和 `-i` 交互模式。另有一处直接 `register_virtual`（line 852，`--list-tables` 路径，只需要行数不需要物化——这里不动）。

### 瓶颈 1：CLI 一次性模式 CSV 走 VIEW

`run_cli`（`-q`, `--sql`, `--exec`）通过 `quick_register` → `register_csv_virtual` 创建 VIEW，搜索时每查询 DuckDB 重新解析 11GB CSV。`import_csv_direct` 作为 TABLE 一次读入后在内存中搜索，速度快 100-1000 倍。

`quick_register` 设计意图是"快速返回"，但对大文件适得其反。

### 瓶颈 2：DuckDB 未设全局性能参数

`init_schema` 只设了 `preserve_insertion_order=false`，未设 `threads` 和 `memory_limit`。CSV 解析无法并行。默认线程数为逻辑核数，默认内存为物理内存的 80%——但不稳定，需显式控制。

### 瓶颈 3：物化时创建无用索引

`materialize_csv_sheet` 为每列创建 B-tree 索引，但对 FullText 搜索（`ILIKE '%keyword%'` 前导通配符）B-tree 完全无法利用。ExactMatch（`=`）和 Wildcard 前缀（`LIKE 'Jo%'`）理论上可用 B-tree，但 DuckDB 列存引擎的自动 zone map 和向量化扫描已足够高效，索引收益远小于其构建和维护成本。20 列 × 3800 万行的索引白占存储、拖慢写入。

### 瓶颈 4：`all_varchar=true` 存储效率（本轮不处理）

两处 CSV 路径都使用 `all_varchar=true`，关闭了 DuckDB 的类型推断和字典编码优化。数值列无法用紧凑格式存储。但改为自动类型推断会影响用户 SQL 的 CAST 行为，风险较高。**本轮接受此风险，不做修改。**

## 设计方案

### P0-1: CLI 一次性模式 CSV 走 TABLE

**文件**: `crates/cli/src/main.rs`

**采用修改 `quick_register` 本身**：

```rust
// CSV → eager TABLE (import_excel → import_csv_direct)
// 其他 → import_file_with_repair (保留 --repair)
if ext == "csv" {
    db.import_excel(file, &|_, _| {})
} else {
    import_file_with_repair(db, file, repair)
}
```

所有调用 `quick_register` 的路径（`run_cli` / `run_sql_cli` / `-i` / `--exec`）自动受益。`--repair` 对非 CSV 文件继续生效。

**TUI 同步改为 eager TABLE**：VIEW 无原生 `rowid`，搜索 `SELECT rowid, ...` 在物化完成前必然失败。TUI 的 VIEW + 后台物化路径已移除，统一走 `import_excel`。

`--list-tables` (`-t`) 路径保持直接 `register_virtual`，不改——它只需要 `COUNT(*)`，VIEW 已足够。

**代价**: CLI 一次性查询的首次导入多等几秒到几十秒（取决于磁盘 I/O，11GB CSV 约 10-30 秒）。
**收益**: 搜索从全表扫描 11GB CSV 文件变为查询内存表，速度提升 100-1000 倍。

### P0-2: DuckDB 全局性能配置

**文件**: `crates/core/src/engine/duckdb.rs`，`init_schema` 函数

添加：
```sql
SET threads = <min(available_parallelism, 8)>;
SET memory_limit = '<env_or_default>GB';
```

- **线程数**: `std::thread::available_parallelism()`，上限 8。
- **内存**: 环境变量 `GREP_EXCEL_DUCKDB_MEMORY` 控制；不设时默认 `'4GB'`（适合大多数场景）。不用 `sysinfo` crate（避免额外依赖）。
- **注意**: `memory_limit` 是硬上限。对于 11GB CSV，`read_csv_auto` 是流式解析——DuckDB 边读边物化，中间结果写临时文件。所以 `4GB` 够用，但设大一点（如 `16GB`）能减少磁盘 I/O。

**收益**: 多线程并行解析 CSV，11GB 文件扫描从 ~60s 降到 ~10s（取决于磁盘速度和线程数）。

### P1: 移除物化时的全列索引

**文件**: `crates/core/src/engine/duckdb.rs`

删除两处索引创建：
- `materialize_csv_sheet` line 1354-1372
- `import_excel_sheets` line 1674-1686

**搜索性能影响**: FullText（`ILIKE '%...%'`）和 Regex 不受影响。ExactMatch（`=`）和 Wildcard 前缀（`LIKE 'Jo%'`）可能小幅回退，但 DuckDB 的 zone map + 向量化扫描在列存格式上已足够高效。以基准测试为准——如果回退明显，可后续按需加回索引。

**收益**: 物化时间减少 30-50%，存储减半。

### P2: 定长格式文件支持（后续独立迭代）

**文件**: 新增 `crates/core/src/fixed_width.rs`

- Rust 侧按字节位置切分 → `Vec<Vec<String>>`
- 列宽来源：`--col-widths` 参数或 header 自动检测
- 复用 DuckDB Appender 路径
- 扩展名检测：`.fix`, `.fwf`

## 改动范围

| 文件 | 改动 | 行数 |
|------|------|------|
| `crates/cli/src/main.rs` | CLI CSV 路由：跳过 `quick_register`，直调 `import_excel` | ~10 |
| `crates/core/src/engine/duckdb.rs` | threads/memory 配置 + 删除索引 | +15 / -40 |

不涉及 API 变更，不破坏现有测试。

## 验证计划

### 测试数据

`/tmp/grep_excel_test_data.csv` — 100 万行 × 20 列，163MB，含高重复列（类别、城市、产品）和数值列。

### 验证步骤

1. **搜索延迟基准测试** — 优化前后对比：
   ```bash
   time grep_excel /tmp/grep_excel_test_data.csv -q "北京"           # FullText
   time grep_excel /tmp/grep_excel_test_data.csv -q "北京" -m exact  # ExactMatch
   time grep_excel /tmp/grep_excel_test_data.csv -q "备注%" -m wildcard  # Wildcard前缀
   ```
   目标：FullText 搜索延迟 < 2s（优化前扫描 163MB CSV 约 2-5s）。

2. **SQL 查询** — 验证 `--sql` 路径：
   ```bash
   grep_excel /tmp/grep_excel_test_data.csv -x "SELECT COUNT(*) FROM sheet_1_0"
   ```

3. **`--list-tables`** — 验证 `-t` 路径不受影响：
   ```bash
   grep_excel /tmp/grep_excel_test_data.csv -t
   ```

4. **现有测试套件**:
   ```bash
   cargo test
   ```

5. **`-m exact` 回归检查** — P1 移除索引后，对比 ExactMatch 在优化前后的耗时。如果 >2x 回退，考虑按需创建索引。
