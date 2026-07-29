# PDF 功能优化与性能优化 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 优化 PDF 表格提取的准确性和速度：合并跨页表格 + 跳过无表格页快速预检 + 多页并行提取。

**Architecture:** 在 `pdf_table.rs` 内完成所有优化，不修改外部接口。使用页边缘（edges）数量作为快速预检信号跳过无表格页面，用 rayon 并行化保留下来的页面处理。跨页表格合并已在 Task 0 实现。

**Tech Stack:** pdfsink-rs 0.2.11, rayon（新增依赖）

---

## 背景分析

### 当前性能瓶颈（17MB / 2500页 PDF 耗时 3分钟+）

**瓶颈 1：pdfsink-rs 急加载**。`PdfDocument::open()` 在打开时就解析全部 2500 页的内容流（parse.rs:363-386），提取每页的 chars/lines/rects/curves。这一步不可绕过——是 pdfsink-rs 的设计决定。

**瓶颈 2：无表格页面也全额付费**。当前代码对每页都跑完整的 `extract_table()` → `TableFinder::new()` 管线（edges → intersections → cells → tables），即使页面根本没有表格。在典型报告类 PDF 中，超过 80% 的页面可能不含表格。

**瓶颈 3：串行处理**。2500 页完全串行。

**瓶颈 4：extract_table 管线昂贵**。`TableFinder::new()` 内部（table.rs:252-276）：
- `get_edges()` — 遍历全部 lines/rects/curves 转 Edge，O(L+R+C)
- `merge_edges()` — 排序聚类合并 edges，O(E log E)
- `edges_to_intersections()` — 垂直×水平 edges 求交点，O(V × H)
- `intersections_to_cells()` — 从交点构建单元格，O(I²)
- `cells_to_tables()` — 分组单元格为表格，O(C²)

### 优化策略

| 策略 | 原理 | 预期提升 |
|------|------|----------|
| **快速预检跳过** | 用 `page.edges()` 快速检查水平和垂直边缘数量，不足则跳过 | 若 80% 页无表 → ~5x |
| **并行化** | rayon `par_iter` 并行处理多页 | ~2-4x（取决于 CPU） |
| **组合** | 预检 + 并行 | 若 80% 页无表 + 8核 → ~10-15x |

**保守估算**（80% 页无表，4 核 CPU）：3 分钟 → ~20-40 秒。

---

## 前置条件检查

开始任何任务前，验证：
- `cargo build -p grep-excel` 编译通过
- `cargo test -p grep-excel-core --features pdf-support` 全部通过
- 无未提交变更

```bash
git status --short && cargo build -p grep-excel 2>&1 | tail -3 && cargo test -p grep-excel-core --features pdf-support 2>&1 | tail -5
```

---

### Task 0: 跨页表格合并（已完成）

**Files:**
- 已修改: `crates/core/src/pdf_table.rs`

已实现 `merge_consecutive_tables()` 和 `first_row_matches_headers()`。两个连续页面上的表格如果列数相同则合并为一个 SheetData。连续页重复表头会被自动跳过。

---

### Task 1: 添加 rayon 依赖

**Files:**
- Modify: `crates/core/Cargo.toml`

**Step 1.1: 添加 rayon 到 core Cargo.toml**

在 `crates/core/Cargo.toml` 的 `[dependencies]` 区域，添加：

```toml
# Parallel PDF page processing
rayon = "1.10"
```

注意：rayon 不加 `optional = true`，因为它是轻量依赖（无系统库），对所有构建都可用。

**Step 1.2: 验证编译**

```bash
cargo build -p grep-excel-core 2>&1 | tail -3
```

预期：编译成功，rayon 依赖解析。

```bash
cargo build -p grep-excel-core --features pdf-support 2>&1 | tail -3
```

预期：pdf-support 下也编译成功。

**Step 1.3: Commit**

```bash
git add crates/core/Cargo.toml
git commit -m "feat(pdf): add rayon dependency for parallel page processing"
```

---

### Task 2: 实现快速预检跳过 + 并行提取

**Files:**
- Modify: `crates/core/src/pdf_table.rs`

**Step 2.1: 重写 parse_pdf 主循环**

将当前串行循环替换为：收集所有页面 → 预检跳过无表页 → 并行提取。

关键设计：
1. 先用 `pdf.pages()` 获取全部页面引用（已是解析好的内存数据，无额外 IO）
2. 对每页做快速预检：计数 edges，水平和垂直各需 >= 2 条才继续
3. 预检通过的页面用 rayon `par_iter` 并行调用 `extract_table`
4. 预检跳过的页面不计入时间的主要部分

```rust
#[cfg(feature = "pdf-support")]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    use pdfsink_rs::{PdfDocument, TableSettings};
    use rayon::prelude::*;

    let pdf = PdfDocument::open(path)
        .map_err(|e| anyhow::anyhow!("failed to open PDF '{}': {}", path.display(), e))?;

    let pages = pdf.pages();
    let page_count = pages.len();

    // Phase 1: Quick pre-check — count edges per page to skip pages
    // unlikely to contain tables (lattice strategy needs both vertical
    // and horizontal edges to form a grid).
    let candidates: Vec<(usize, &Page)> = pages
        .iter()
        .enumerate()
        .filter(|(_, page)| {
            let edges = page.edges();
            let v_edges = edges.iter().filter(|e| e.orientation == Orientation::Vertical).count();
            let h_edges = edges.iter().filter(|e| e.orientation == Orientation::Horizontal).count();
            v_edges >= 2 && h_edges >= 2
        })
        .collect();

    // Phase 2: Parallel table extraction only on candidate pages
    let results: Vec<(usize, Option<SheetData>)> = candidates
        .par_iter()
        .map(|(page_idx, page)| {
            let page_num = page_idx + 1; // 1-based
            let table_result = page.extract_table(TableSettings::default());
            match table_result {
                Ok(Some(table)) => {
                    let string_table = table
                        .into_iter()
                        .map(|row| row.into_iter().map(|c| c.unwrap_or_default()).collect::<Vec<_>>())
                        .collect::<Vec<_>>();

                    if string_table.len() >= 2 && !string_table[0].is_empty() {
                        let name = if page_count == 1 {
                            "Table_1".to_string()
                        } else {
                            format!("Page_{}_Table_{}", page_num, 0)
                        };
                        Some(SheetData {
                            name,
                            headers: string_table[0].clone(),
                            rows: string_table[1..].to_vec(),
                            col_widths: Vec::new(),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
            .map(|sd| (page_num, sd))
        })
        .collect();

    // Collect successfully extracted tables, preserving page order for merge
    let mut page_tables: Vec<(usize, SheetData)> = results
        .into_iter()
        .flatten()
        .collect();
    page_tables.sort_by_key(|(page_num, _)| *page_num);

    // Rename in page order
    for (i, (_, table)) in page_tables.iter_mut().enumerate() {
        table.name = if page_count == 1 {
            "Table_1".to_string()
        } else {
            format!("Page_{}_Table_{}", table.name.split('_').nth(1).unwrap_or("1"), i + 1)
        };
    }

    // Phase 3: Merge consecutive-page tables
    let merged = merge_consecutive_tables(page_tables);

    if merged.is_empty() {
        anyhow::bail!(
            "No tables extracted from PDF '{}'. The PDF may not contain bordered tables, or may be scanned (OCR not supported).",
            path.display()
        );
    }

    Ok(merged)
}
```

**Step 2.2: 需要添加的 imports**

在 `#[cfg(feature = "pdf-support")]` 块内的 `use pdfsink_rs::{PdfDocument, TableSettings};` 后添加：

```rust
use pdfsink_rs::Orientation;
use rayon::prelude::*;
```

移除不再需要的 `use pdfsink_rs::Page;`（现在通过 `pdf.pages()` 获取）。

**Step 2.3: 编译验证**

```bash
cargo build -p grep-excel-core --features pdf-support 2>&1 | tail -3
```

预期：编译成功。

**Step 2.4: 运行现有测试**

```bash
cargo test -p grep-excel-core --test pdf --features pdf-support
```

预期：`parse_simple_pdf` 测试通过。

**Step 2.5: Commit**

```bash
git add crates/core/src/pdf_table.rs
git commit -m "perf(pdf): add edge-count pre-check and rayon parallel extraction"
```

---

### Task 3: 重命名修复

**Files:**
- Modify: `crates/core/src/pdf_table.rs`

**问题：** Task 2 中的重命名逻辑用了临时 name 做中间值，不够清晰。应该在收集完后直接按最终顺序重命名。

**Step 3.1: 简化命名逻辑**

将 Task 2 中收集阶段的 `name` 统一设为占位值，重命名移到最终阶段：

收集时不关注 name：
```rust
let sd = SheetData {
    name: String::new(), // placeholder, renamed later
    headers: string_table[0].clone(),
    rows: string_table[1..].to_vec(),
    col_widths: Vec::new(),
};
```

在 merge 之后、返回之前重命名（已由 `merge_consecutive_tables` 内部处理，无需额外修改）。

**Step 3.2: 验证**

```bash
cargo test -p grep-excel-core --test pdf --features pdf-support
```

**Step 3.3: Commit**

```bash
git add crates/core/src/pdf_table.rs
git commit -m "refactor(pdf): simplify sheet naming in parallel extraction"
```

---

### Task 4: Clippy + 全面测试

**Step 4.1: Clippy**

```bash
cargo clippy -p grep-excel-core --features pdf-support -- -D warnings 2>&1 | grep -v "warning: unused\|error: unused\|error: this \`impl\`"
```

预期：pdf_table.rs 相关零 warning（忽略 archive.rs、source.rs、types.rs 已有的预存 warning）。

**Step 4.2: 全面测试**

```bash
cargo test -p grep-excel-core --features pdf-support
```

预期：全部通过。

**Step 4.3: 完整功能编译**

```bash
cargo build -p grep-excel --features full
```

**Step 4.4: Commit 任何修复**

```bash
git add -A
git commit -m "chore(pdf): clippy and test fixes for parallel extraction"
```

---

### Task 5: 更新设计文档注释

**Files:**
- Modify: `crates/core/src/pdf_table.rs`

**Step 5.1: 更新 parse_pdf 的模块级注释**

在 `parse_pdf` 函数上方添加实现说明：

```rust
/// Parse tables from a PDF file.
///
/// Extraction uses a two-phase approach:
/// 1. Quick pre-check: count vertical and horizontal edges per page;
///    pages with fewer than 2 edges in either direction are skipped,
///    since lattice table detection requires a grid of lines.
/// 2. Parallel extraction: remaining candidate pages are processed
///    in parallel via rayon, then tables on consecutive pages with
///    matching column counts are merged into single sheets.
#[cfg(feature = "pdf-support")]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
```

**Step 5.2: Commit**

```bash
git add crates/core/src/pdf_table.rs
git commit -m "docs(pdf): document two-phase extraction approach"
```

---

## 总结

| Task | 描述 | 文件 | Commits |
|------|------|------|---------|
| 0 | 跨页表格合并 | `pdf_table.rs` | 已提交 |
| 1 | rayon 依赖 | `Cargo.toml` | 1 |
| 2 | 预检+并行提取 | `pdf_table.rs` | 1 |
| 3 | 重命名简化 | `pdf_table.rs` | 1 |
| 4 | Clippy+测试 | `pdf_table.rs` | 0-1 |
| 5 | 文档注释 | `pdf_table.rs` | 1 |

**Total: ~5 commits, 2 files modified.**

**关键设计决策：**
- **预检阈值：** 水平和垂直 edges 各 >= 2 条。这是 lattice 策略的最低要求（至少需要 2 条竖线 + 2 条横线才能形成 1×1 单元格）。太低会导致空页也通过，太高会漏检小表格。2 是合理起点。
- **rayon 不加 feature gate：** rayon 是纯 Rust 轻量依赖（无系统库、无 C 编译），适合放入默认依赖。不像 DuckDB 需要编译 C++。
- **预检使用 `page.edges()` 而非 `page.lines()`：** `edges()` 包含 lines + rects + curves 转的 Edge，覆盖面更广，能检测到用矩形框画的表格。
- **并行粒度：** 页级别并行。每页的 `extract_table` 是独立的，无共享状态。
- **保持 merge_consecutive_tables 不变：** 合并逻辑在并行提取之后执行，仍然串行（O(n)），但 n 是表格数而非页数，影响可忽略。

**优化效果预估**（保守）：

| 场景 | 优化前 | 优化后 | 提升 |
|------|--------|--------|------|
| 2500 页，80% 无表，4 核 | ~180s | ~25-45s | 4-7x |
| 2500 页，20% 无表，4 核 | ~180s | ~50-70s | 2.5-3.6x |
| 10 页，全有表 | ~1s | ~0.5s | ~2x |
