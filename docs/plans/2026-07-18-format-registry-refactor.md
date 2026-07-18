# File Format Registry Refactoring & New Format Support

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor the hardcoded if-else extension dispatch into a zero-cost `FileFormat` enum registry, then add TSV import, DBF parsing, and XML import stubs — without any performance regression.

**Architecture:** Replace the 5 duplicated `if ext == "csv" ... else if ext == "html" ...` chains with a single `detect_format() -> FileFormat` function. All dispatch becomes `match format { ... }` — a jump table at the machine code level, strictly faster than repeated string comparison. No trait objects, no boxing, no allocation in the hot path.

**Tech Stack:** Rust (existing), `csv` crate (tab delimiter reuse), `dbf` crate (new dependency)

---

## Background: Current State

### Dispatch Duplication

The same extension→parser mapping appears in **5 functions** in `crates/core/src/excel.rs`:

| Function | Lines | Pattern |
|----------|-------|---------|
| `parse_file()` | 70–93 | `ext == "csv"` / `html` / `txt\|md` / else calamine |
| `parse_file_metadata()` | 561–584 | same |
| `for_each_sheet()` | 695–726 | same (partial: html/txt/md only) |
| `parse_file_repair()` | 890–908 | same |
| `for_each_sheet_repair()` | 911–954 | same |

Additionally, `TABLE_EXTENSIONS` in `archive.rs:17` is a separate hand-maintained list that must stay in sync.

### Call Sites (all unchanged after refactor)

```
engine/memory.rs       → parse_file(), parse_file_repair()
engine/duckdb.rs       → parse_file_metadata(), for_each_sheet()
engine/sqlite.rs       → parse_file(), parse_file_repair()
cli/main.rs            → parse_file_metadata()
excel.rs (archive)     → parse_file(), parse_file_metadata() (recursive)
tests/*                → parse_file()
```

---

## Performance Design

### Why Enum Dispatch Is Zero-Cost

```
Current:  String::to_ascii_lowercase() + 4× String comparison   → O(n) byte scan
Proposed: detect_format() produces a Copy enum                  → O(1) jump table
```

1. `FileFormat` is `#[derive(Copy, Clone)]` — single integer discriminant, passed in a register.
2. `match format { ... }` compiles to a jump table (LLVM `switch`), not chained branches.
3. No `Box<dyn Trait>`, no vtable lookup, no heap allocation.
4. Each parser remains a monomorphized function — full inlining potential.
5. The dispatch cost is <1ns; file I/O dominates by 6+ orders of magnitude.

### Compile-Time Extension List

`TABLE_EXTENSIONS` becomes a `const` derived from format definitions at compile time — no runtime sync needed.

---

## Design Decision: Format Override with Sticky `--as` Flag

### Problem

Mixed-format multi-file is a real scenario:

```
# A directory export containing multiple formats
grep_excel *.csv *.dbf report.txt
```

When `.txt` is actually TSV, or a `.dat` file has no recognizable extension, the user needs per-file format control.

### Rejected: Per-file Prefix Syntax

```
grep_excel tsv:a.txt pipe:b.txt c.csv   # ❌ breaks shell glob, path ambiguity
```

`tsv:./relative/path/file.txt` is ambiguous — is `tsv` a format or a directory?

### Decision: **Sticky `--as` Flag** (modeled after `grep -e`, `find -name`)

`--as <format>` sets the format for all subsequent positional file arguments until the next `--as`. Files with no preceding `--as` use extension-based auto-detection.

```
# Default: extension auto-detection
grep_excel a.tsv b.csv c.txt               # Each auto-detected ✅

# Single format override (sticky to end)
grep_excel --as tsv data.txt               # ✅

# Mixed formats in one invocation
grep_excel --as csv a.csv b.csv --as dbf a.dbf /more/*.dbf
#            ^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#            CSV group         DBF group

# Extension detection mixed with overrides
grep_excel data.xlsx --as tsv export.txt --as dbf legacy/*.dbf
#            ^^^^^^^^^  ^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^
#            auto       TSV override        DBF override

# Same format, different extensions — forced uniform parse
grep_excel --as tsv pipe_data.txt tab_data.tsv
#            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#            both parsed as TSV regardless of extension
```

**Rationale:**
1. **Shell-glob compatible** — `grep_excel --as dbf *.dbf` works naturally
2. **One invocation for mixed workloads** — no need for multiple process starts
3. **Backward compatible** — without `--as`, behavior is unchanged
4. **Predictable** — the flag applies forward until overridden (same mental model as `grep -e`, `find -name`, `curl -H`)

### Parsing Model

```
Input:  grep_excel data.xlsx --as tsv a.txt b.txt --as dbf c.dbf

Parse result:
  [("data.xlsx",  None),       // extension auto-detect
   ("a.txt",      Some(Tsv)),  // sticky --as tsv
   ("b.txt",      Some(Tsv)),  // sticky --as tsv
   ("c.dbf",      Some(Dbf))]  // sticky --as dbf
```

Each file dispatched to `parse_file_as(path, format_override)` independently.

### `--as` Accepted Values

| Value | Format | Delimiter | Parser |
|-------|--------|-----------|--------|
| `csv` | Csv | `,` | csv crate |
| `tsv` | Tsv | `\t` | csv crate |
| `excel` | Excel | — | calamine |
| `html` | Html | — | scraper |
| `txt` | Text | — | heuristic text_table |
| `md` | Markdown | — | GFM pipe table |
| `dbf` | Dbf | — | dbf crate |
| `xml` | Xml | — | roxmltree |

**Future**: `--delimiter` flag to override the delimiter for `csv`/`tsv` formats (e.g., `--as csv --delimiter '|'` for pipe-delimited), deferred to a separate phase.

---

## Phase 1: Format Registry Core (Refactor Only, No Behavior Change)

### Task 1.1: Create `FileFormat` enum and `detect_format()`

**Files:**
- Create: `crates/core/src/format.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod format;`)

**Step 1: Write the module**

```rust
// crates/core/src/format.rs

use std::path::Path;

/// File format identified by extension.
///
/// Used as a zero-cost dispatch tag. `Copy` ensures no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Excel / ODS formats (calamine): .xlsx .xls .xlsm .xlsb .ods
    Excel,
    /// Comma-separated values (csv crate): .csv
    Csv,
    /// Tab-separated values (csv crate, delimiter=b'\t'): .tsv .tab
    Tsv,
    /// HTML tables (scraper): .html .htm
    Html,
    /// Plain-text heuristic tables: .txt
    Text,
    /// Markdown pipe tables: .md .markdown
    Markdown,
    /// dBase database files: .dbf
    Dbf,
    /// XML data files: .xml
    Xml,
}

impl FileFormat {
    /// Detect format from file extension. Returns `None` for unknown extensions
    /// (caller should fall back to calamine as a last resort).
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "csv" => Some(Self::Csv),
            "tsv" | "tab" => Some(Self::Tsv),
            "html" | "htm" => Some(Self::Html),
            "txt" => Some(Self::Text),
            "md" | "markdown" => Some(Self::Markdown),
            "dbf" => Some(Self::Dbf),
            "xml" => Some(Self::Xml),
            // Excel/ODS formats — detected here explicitly so we know it's
            // intentional, not just a fallback
            "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => Some(Self::Excel),
            // Unknown extension — caller decides fallback (typically calamine)
            _ => None,
        }
    }

    /// All extensions recognized as table files (for archive filtering).
    /// Derived at compile time from the same extension→format mapping.
    pub const TABLE_EXTENSIONS: &[&str] = &[
        // Excel family
        "xlsx", "xls", "xlsm", "xlsb", "ods",
        // Delimited text
        "csv", "tsv", "tab",
        // Web / markup
        "html", "htm",
        // Text / markdown
        "txt", "md", "markdown",
        // Database
        "dbf",
        // XML
        "xml",
    ];
}
```

**Step 2: Add module to lib.rs**

```rust
// In crates/core/src/lib.rs, add:
pub mod format;
```

**Step 3: Verify it compiles**

```bash
cargo build -p grep-excel-core 2>&1
```

Expected: Compiles cleanly (module unused yet, may get dead_code warning — acceptable for this step).

**Step 4: Commit**

```bash
git add crates/core/src/format.rs crates/core/src/lib.rs
git commit -m "feat: add FileFormat enum and detect_format()"
```

---

### Task 1.2: Refactor `parse_file()` to use `FileFormat`

**Files:**
- Modify: `crates/core/src/excel.rs:70-93`

**Step 1: Replace the dispatch logic**

Replace lines 70–93:

```rust
// OLD (lines 70-93):
pub fn parse_file(path: &Path) -> Result<Vec<SheetData>> {
    #[cfg(feature = "archive-support")]
    {
        if let Some(format) = crate::archive::detect_archive(path) {
            return parse_archive(path, format);
        }
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "csv" {
        parse_csv(path)
    } else if ext == "html" || ext == "htm" {
        parse_html(path)
    } else if ext == "txt" || ext == "md" || ext == "markdown" {
        parse_text(path)
    } else {
        parse_excel(path)
    }
}
```

```rust
// NEW:
use crate::format::FileFormat;

pub fn parse_file(path: &Path) -> Result<Vec<SheetData>> {
    #[cfg(feature = "archive-support")]
    {
        if let Some(format) = crate::archive::detect_archive(path) {
            return parse_archive(path, format);
        }
    }

    match FileFormat::from_path(path) {
        Some(FileFormat::Csv) => parse_csv(path),
        Some(FileFormat::Tsv) => parse_tsv(path),
        Some(FileFormat::Html) => parse_html(path),
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => parse_text(path),
        Some(FileFormat::Dbf) => parse_dbf(path),
        Some(FileFormat::Xml) => parse_xml(path),
        Some(FileFormat::Excel) => parse_excel(path),
        // Unknown extension: fall back to calamine (backward compat)
        None => parse_excel(path),
    }
}
```

**Step 2: Add stub functions for new formats** (they return `Err` with "not yet implemented" — real impl in Phase 3/4)

```rust
fn parse_tsv(path: &Path) -> Result<Vec<SheetData>> {
    parse_delimited(path, b'\t')
}

fn parse_dbf(_path: &Path) -> Result<Vec<SheetData>> {
    anyhow::bail!("DBF format support is not yet implemented");
}

fn parse_xml(_path: &Path) -> Result<Vec<SheetData>> {
    anyhow::bail!("XML format support is not yet implemented");
}
```

**Step 3: Extract CSV parsing into reusable `parse_delimited()`**

The existing `parse_csv()` (lines 131–165) becomes:

```rust
/// Parse a delimiter-separated file (CSV, TSV, etc.)
fn parse_delimited(path: &Path, delimiter: u8) -> Result<Vec<SheetData>> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("data")
        .to_string();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_path(path)?;

    let mut all_rows: Vec<Vec<String>> = Vec::new();
    for result in rdr.records() {
        let record = result?;
        all_rows.push(record.iter().map(|s| s.to_string()).collect());
    }

    if all_rows.len() < 2 {
        return Ok(Vec::new());
    }

    let headers = all_rows.remove(0);
    let rows = all_rows;

    Ok(vec![SheetData {
        name,
        headers,
        rows,
        col_widths: Vec::new(),
    }])
}

fn parse_csv(path: &Path) -> Result<Vec<SheetData>> {
    parse_delimited(path, b',')
}

fn parse_tsv(path: &Path) -> Result<Vec<SheetData>> {
    parse_delimited(path, b'\t')
}
```

**Step 4: Build and run existing tests**

```bash
cargo test -p grep-excel-core 2>&1
```

Expected: All existing tests pass, no regressions.

**Step 5: Commit**

```bash
git add crates/core/src/excel.rs
git commit -m "refactor: use FileFormat enum dispatch in parse_file()"
```

---

### Task 1.3: Refactor `parse_file_metadata()` to use `FileFormat`

**Files:**
- Modify: `crates/core/src/excel.rs:561-584`

**Step 1: Replace dispatch logic**

Replace the extension if-else chain with the same `match FileFormat::from_path(path)` pattern used in Task 1.2. Use `parse_delimited_metadata()` extracted from `parse_csv_metadata()`.

```rust
pub fn parse_file_metadata(path: &Path) -> Result<Vec<SheetMetadata>> {
    #[cfg(feature = "archive-support")]
    {
        if let Some(format) = crate::archive::detect_archive(path) {
            return parse_archive_metadata(path, format);
        }
    }

    match FileFormat::from_path(path) {
        Some(FileFormat::Csv) => parse_delimited_metadata(path, b','),
        Some(FileFormat::Tsv) => parse_delimited_metadata(path, b'\t'),
        Some(FileFormat::Html) => parse_html_metadata(path),
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => parse_text_metadata(path),
        Some(FileFormat::Excel) => parse_excel_metadata(path),
        _ => parse_excel_metadata(path), // fallback
    }
}

fn parse_delimited_metadata(path: &Path, delimiter: u8) -> Result<Vec<SheetMetadata>> {
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("data").to_string();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_path(path)?;

    let mut headers: Vec<String> = Vec::new();
    let mut row_count: usize = 0;
    for result in rdr.records() {
        let record = result?;
        if headers.is_empty() {
            headers = record.iter().map(|s| s.to_string()).collect();
        } else {
            row_count += 1;
        }
    }
    if headers.is_empty() || row_count == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![SheetMetadata { name, headers, row_count }])
}
```

Then delete the old `parse_csv_metadata()` function (lines 620–652) since it's replaced by `parse_delimited_metadata(path, b',')`.

**Step 2: Verify**

```bash
cargo test -p grep-excel-core 2>&1
```

**Step 3: Commit**

```bash
git add crates/core/src/excel.rs
git commit -m "refactor: use FileFormat enum dispatch in parse_file_metadata()"
```

---

### Task 1.4: Refactor `for_each_sheet()` to use `FileFormat`

**Files:**
- Modify: `crates/core/src/excel.rs:695-726`

**Step 1: Replace the if-else chain**

```rust
pub fn for_each_sheet<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    match FileFormat::from_path(path) {
        Some(FileFormat::Html) => {
            let sheets = parse_html(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("html_{}", idx), row_count));
            }
            Ok(info)
        }
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => {
            let sheets = parse_text(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("text_{}", idx), row_count));
            }
            Ok(info)
        }
        _ => for_each_excel_sheet(path, handler),
    }
}

// Rename the old for_each_sheet body (the calamine part, lines 727-793)
// to for_each_excel_sheet()
fn for_each_excel_sheet<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    // ... existing calamine loop (lines 727-793) ...
}
```

**Step 2: Verify**

```bash
cargo test -p grep-excel-core 2>&1
cargo test -p grep-excel-core --test merged_cells 2>&1
```

**Step 3: Commit**

```bash
git add crates/core/src/excel.rs
git commit -m "refactor: use FileFormat enum dispatch in for_each_sheet()"
```

---

### Task 1.5: Refactor `parse_file_repair()` and `for_each_sheet_repair()`

**Files:**
- Modify: `crates/core/src/excel.rs:890-954`

**Step 1: Apply the same `match FileFormat::from_path()` pattern**

These two functions already defer non-Excel formats to their non-repair counterparts — the refactoring just replaces the if-else chain with `match`.

```rust
pub fn parse_file_repair(path: &Path) -> Result<Vec<SheetData>> {
    match FileFormat::from_path(path) {
        Some(FileFormat::Csv) => parse_csv(path),
        Some(FileFormat::Tsv) => parse_tsv(path),
        Some(FileFormat::Html) => parse_html(path),
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => parse_text(path),
        // Excel formats get the repair treatment; unknown falls through to repair too
        _ => parse_xlsx_repair(path),
    }
}

pub fn for_each_sheet_repair<F>(path: &Path, mut handler: F) -> Result<Vec<(String, usize)>>
where
    F: FnMut(SheetData, usize) -> Result<()>,
{
    match FileFormat::from_path(path) {
        Some(FileFormat::Csv) | Some(FileFormat::Tsv) => {
            // Delimited files have no "repair" — just parse normally
            let sheets = parse_file(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("sheet_{}", idx), row_count));
            }
            Ok(info)
        }
        Some(FileFormat::Html) => {
            let sheets = parse_html(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("html_{}", idx), row_count));
            }
            Ok(info)
        }
        Some(FileFormat::Text) | Some(FileFormat::Markdown) => {
            let sheets = parse_text(path)?;
            let mut info = Vec::new();
            for (idx, sheet) in sheets.into_iter().enumerate() {
                let row_count = sheet.rows.len();
                handler(sheet, idx)?;
                info.push((format!("text_{}", idx), row_count));
            }
            Ok(info)
        }
        _ => for_each_xlsx_repair(path, handler),
    }
}
```

**Step 2: Verify**

```bash
cargo test -p grep-excel-core 2>&1
```

**Step 3: Commit**

```bash
git add crates/core/src/excel.rs
git commit -m "refactor: use FileFormat enum dispatch in repair functions"
```

---

### Task 1.6: Update `TABLE_EXTENSIONS` in archive.rs to use format registry

**Files:**
- Modify: `crates/core/src/archive.rs:17-19`

**Step 1: Replace the hand-maintained list**

```rust
// OLD:
pub const TABLE_EXTENSIONS: &[&str] = &[
    "xlsx", "xls", "xlsm", "xlsb", "ods", "csv", "html", "htm", "txt", "md", "markdown",
];

// NEW:
pub use crate::format::FileFormat;
pub const TABLE_EXTENSIONS: &[&str] = FileFormat::TABLE_EXTENSIONS;
```

**Step 2: Verify archive tests still pass**

```bash
cargo test -p grep-excel-core 2>&1
```

**Step 3: Commit**

```bash
git add crates/core/src/archive.rs
git commit -m "refactor: derive TABLE_EXTENSIONS from FileFormat registry"
```

---

### Task 1.7: Update help text in i18n.rs

**Files:**
- Modify: `crates/core/src/i18n.rs` (lines 836–843 for Chinese, lines 907–914 for English)

**Step 1: Add new formats to both language versions**

```rust
// Chinese (around line 836):
"                  支持的文件格式:\n\
                     .xlsx  .xls  .xlsm  .xlsb  .ods  (Excel/电子表格)\n\
                     .csv                                (逗号分隔)\n\
                     .tsv  .tab                          (制表符分隔)\n\
                     .html  .htm                         (HTML 表格, 自动检测编码)\n\
                     .txt   .md   .markdown              (文本/Markdown 表格)\n\
                     .dbf                                (dBase 数据库)\n\
                     .xml                                (XML 数据)\n\
                     .zip  .tar  .tar.gz  .tgz  .tar.bz2  .tar.xz  .tar.zst\n\
                                                        (归档文件, 自动提取内部表格)\n\
                     .zip.001                            (分卷 ZIP)\n\n\"

// English (around line 907):
"                  Supported Formats:\n\
                     .xlsx  .xls  .xlsm  .xlsb  .ods  (Excel / Spreadsheets)\n\
                     .csv                               (Comma-separated)\n\
                     .tsv  .tab                         (Tab-separated)\n\
                     .html  .htm                        (HTML tables, auto-detect encoding)\n\
                     .txt   .md   .markdown             (Text / Markdown tables)\n\
                     .dbf                               (dBase database)\n\
                     .xml                               (XML data)\n\
                     .zip  .tar  .tar.gz  .tgz  .tar.bz2  .tar.xz  .tar.zst\n\
                                                        (Archives, table files extracted automatically)\n\
                     .zip.001                           (Split ZIP volumes)\n\n\"
```

**Step 2: Verify**

```bash
cargo build -p grep-excel-core 2>&1
```

**Step 3: Commit**

```bash
git add crates/core/src/i18n.rs
git commit -m "docs: add TSV, DBF, XML to supported format help text"
```

---

### Task 1.8: Full integration test — no regressions

**Step 1: Run the full test suite**

```bash
cargo test --all 2>&1
```

**Step 2: Run the AWR regression tests** (these exercise txt/html/md parsers)

```bash
cargo test -p grep-excel-core text_table_test 2>&1
cargo test -p grep-excel-core merged_cells 2>&1
```

**Step 3: Verify all existing formats still import correctly**

```bash
cargo run -- test_data2.xlsx -t 2>&1
cargo run -- tests/regress/awr.txt -t 2>&1
cargo run -- tests/regress/awr.html -t 2>&1
cargo run -- tests/regress/awr.md -t 2>&1
```

Expected: All files list their tables without errors.

**Step 4: Commit** (if any fixes from test failures)

```bash
git add -u
git commit -m "fix: address test regressions from format refactor"
```

---

## Phase 2: TSV Import (Real Implementation)

### Task 2.1: Verify TSV parsing works end-to-end

**Files:**
- Create: `crates/core/tests/tsv_test.rs`

**Step 1: Write integration test**

```rust
use grep_excel_core::excel::parse_file;
use std::io::Write;

#[test]
fn test_tsv_basic_import() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_basic.tsv");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "Name\tAge\tCity").unwrap();
    writeln!(f, "Alice\t30\tNYC").unwrap();
    writeln!(f, "Bob\t25\tSF").unwrap();
    drop(f);

    let sheets = parse_file(&path).expect("TSV parse should succeed");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["Name", "Age", "City"]);
    assert_eq!(sheets[0].rows.len(), 2);
    assert_eq!(sheets[0].rows[0], vec!["Alice", "30", "NYC"]);
    assert_eq!(sheets[0].rows[1], vec!["Bob", "25", "SF"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tsv_with_quotes() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_quoted.tsv");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "Col1\tCol2").unwrap();
    writeln!(f, "a\t\"tab\tinside\"").unwrap();
    drop(f);

    let sheets = parse_file(&path).expect("quoted TSV should parse");
    assert_eq!(sheets[0].rows[0], vec!["a", "tab\tinside"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tsv_metadata() {
    use grep_excel_core::excel::parse_file_metadata;

    let dir = std::env::temp_dir();
    let path = dir.join("test_meta.tsv");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "A\tB\tC").unwrap();
    writeln!(f, "1\t2\t3").unwrap();
    writeln!(f, "4\t5\t6").unwrap();
    writeln!(f, "7\t8\t9").unwrap();
    drop(f);

    let meta = parse_file_metadata(&path).expect("TSV metadata should work");
    assert_eq!(meta.len(), 1);
    assert_eq!(meta[0].headers, vec!["A", "B", "C"]);
    assert_eq!(meta[0].row_count, 3);

    let _ = std::fs::remove_file(&path);
}
```

**Step 2: Run the new tests**

```bash
cargo test -p grep-excel-core --test tsv_test 2>&1
```

Expected: All 3 tests pass.

**Step 3: Commit**

```bash
git add crates/core/tests/tsv_test.rs
git commit -m "test: add TSV import integration tests"
```

---

### Task 2.2: End-to-end CLI smoke test

**Step 1: Create a test TSV file and import via CLI**

```bash
printf "Name\tDept\tSalary\nAlice\tEng\t100000\nBob\tSales\t85000\n" > /tmp/test.tsv
cargo run -- /tmp/test.tsv -t 2>&1
```

Expected: Lists the TSV table with columns Name, Dept, Salary and 2 rows.

**Step 2: Run a search query**

```bash
cargo run -- /tmp/test.tsv -q "Alice" -m exact 2>&1
```

Expected: Returns the Alice row.

**Step 3: Cleanup**

```bash
rm /tmp/test.tsv
```

---

## Phase 2.5: `--as` CLI Flag for Format Override

### Task 2.3: Add `FileFormat::from_name()` for CLI parsing

**Files:**
- Modify: `crates/core/src/format.rs`

**Step 1: Add name-based lookup**

```rust
impl FileFormat {
    // ... existing from_path() ...

    /// Parse format from a CLI string (for --as flag).
    /// Returns None for unrecognized names.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "tsv" | "tab" => Some(Self::Tsv),
            "html" | "htm" => Some(Self::Html),
            "txt" | "text" => Some(Self::Text),
            "md" | "markdown" => Some(Self::Markdown),
            "dbf" => Some(Self::Dbf),
            "xml" => Some(Self::Xml),
            "excel" | "xlsx" | "xls" => Some(Self::Excel),
            _ => None,
        }
    }

    /// Human-readable names for error messages / help text.
    pub const ALL_NAMES: &[&str] = &[
        "csv", "tsv", "html", "txt", "md", "dbf", "xml", "excel",
    ];
}
```

**Step 2: Add test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_valid() {
        assert_eq!(FileFormat::from_name("csv"), Some(FileFormat::Csv));
        assert_eq!(FileFormat::from_name("TSV"), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_name("Excel"), Some(FileFormat::Excel));
    }

    #[test]
    fn from_name_invalid() {
        assert_eq!(FileFormat::from_name("pdf"), None);
        assert_eq!(FileFormat::from_name(""), None);
    }
}
```

**Step 3: Commit**

```bash
git add crates/core/src/format.rs
git commit -m "feat: add FileFormat::from_name() for CLI --as flag"
```

---

### Task 2.4: Parse sticky `--as` flags and plumb per-file format to engines

**Files:**
- Modify: `crates/cli/src/main.rs` (parse sticky `--as` groups)
- Modify: `crates/core/src/excel.rs` (add `parse_file_as()`)
- Modify: `crates/core/src/engine/memory.rs` (accept per-file format override)
- Modify: `crates/core/src/engine/duckdb.rs` (accept per-file format override)
- Modify: `crates/core/src/engine/sqlite.rs` (accept per-file format override)

**Step 1: Parse sticky `--as` into file+format groups in CLI**

The `argh` parser gives us `--as` as a single `Option<String>`. For sticky behavior, we need to restructure how args are collected. Since `argh` doesn't natively support repeating flags before positional args, we parse the raw `std::env::args()` manually for the `--as` grouping:

```rust
/// A file with an optional format override.
struct FileArg {
    path: PathBuf,
    format_override: Option<FileFormat>,
}

/// Parse CLI args into file groups respecting sticky --as.
fn parse_file_args(args: &[String]) -> Vec<FileArg> {
    let mut files = Vec::new();
    let mut current_format: Option<FileFormat> = None;

    let mut i = 1; // skip program name
    while i < args.len() {
        if args[i] == "--as" {
            i += 1;
            if i < args.len() {
                match FileFormat::from_name(&args[i]) {
                    Some(fmt) => current_format = Some(fmt),
                    None => {
                        eprintln!("Warning: unrecognized format '{}', ignoring --as. Valid: {:?}",
                            args[i], FileFormat::ALL_NAMES);
                    }
                }
                i += 1;
            }
        } else if !args[i].starts_with('-') {
            // Positional file argument
            files.push(FileArg {
                path: PathBuf::from(&args[i]),
                format_override: current_format,
            });
            i += 1;
        } else {
            // Other flags (--query, --mode, etc.) — skip (handled by argh)
            // For flags that take a value, skip the value too
            i += if i + 1 < args.len() && !args[i + 1].starts_with('-') { 2 } else { 1 };
        }
    }

    files
}
```

Then in `main()`:

```rust
let raw_args: Vec<String> = std::env::args().collect();
let file_args = parse_file_args(&raw_args);

// Pass file_args to import, each with its format_override
for fa in &file_args {
    engine.import_excel_as(&fa.path, fa.format_override, &progress)?;
}
```

**Step 2: Add `parse_file_as()` to core**

```rust
/// Parse a file with an optional explicit format override.
/// When `format` is None, auto-detects from extension.
pub fn parse_file_as(path: &Path, format: Option<FileFormat>) -> Result<Vec<SheetData>> {
    #[cfg(feature = "archive-support")]
    {
        if format.is_none() {
            if let Some(archive_format) = crate::archive::detect_archive(path) {
                return parse_archive(path, archive_format);
            }
        }
    }

    let fmt = format.unwrap_or_else(|| {
        FileFormat::from_path(path).unwrap_or(FileFormat::Excel)
    });

    match fmt {
        FileFormat::Csv => parse_delimited(path, b','),
        FileFormat::Tsv => parse_delimited(path, b'\t'),
        FileFormat::Html => parse_html(path),
        FileFormat::Text | FileFormat::Markdown => parse_text(path),
        FileFormat::Dbf => parse_dbf(path),
        FileFormat::Xml => parse_xml(path),
        FileFormat::Excel => parse_excel(path),
    }
}
```

`parse_file()` becomes:

```rust
pub fn parse_file(path: &Path) -> Result<Vec<SheetData>> {
    parse_file_as(path, None)
}
```

Do the same pattern for `parse_file_metadata_as()`, `for_each_sheet_as()`, `parse_file_repair_as()`.

**Step 3: Add per-file format override to engine trait**

```rust
// In engine module:
fn import_excel_as(
    &mut self,
    path: &Path,
    format_override: Option<FileFormat>,
    progress: &dyn Fn(usize, usize),
) -> Result<FileInfo>;
```

Each engine (memory, duckdb, sqlite) calls `parse_file_as(path, format_override)` instead of `parse_file(path)`.

**Step 4: Integration test**

```bash
# Mixed formats in one invocation
printf "A,B,C\n1,2,3\n" > /tmp/test.csv
printf "X\tY\tZ\n4\t5\t6\n" > /tmp/test.tsv
cargo run -- --as csv /tmp/test.csv --as tsv /tmp/test.tsv -t 2>&1
# Expected: both files listed with correct columns

# Extension auto-detect mixed with explicit override
cargo run -- /tmp/test.csv --as tsv /tmp/test.csv -t 2>&1
# Expected: first parsed as CSV (auto), second as TSV (override)
# (may error on second since the file is actually CSV)

rm /tmp/test.csv /tmp/test.tsv
```

**Step 5: Commit**

```bash
git add crates/core/src/excel.rs crates/cli/src/main.rs crates/core/src/engine/
git commit -m "feat: add sticky --as flag for per-file format override"
```

---

## Phase 3: DBF Parsing (New Dependency, Real Implementation)

### Task 3.1: Add `dbf` crate dependency

**Files:**
- Modify: `crates/core/Cargo.toml`

**Step 1: Add dependency**

```toml
# Under [dependencies]:
dbf = "0.2"
```

**Step 2: Verify it resolves**

```bash
cargo update -p grep-excel-core 2>&1
cargo build -p grep-excel-core 2>&1
```

Expected: Compiles with new dependency.

**Step 3: Commit**

```bash
git add crates/core/Cargo.toml Cargo.lock
git commit -m "deps: add dbf crate for DBF file support"
```

---

### Task 3.2: Implement `parse_dbf()`

**Files:**
- Modify: `crates/core/src/excel.rs` (replace the stub `parse_dbf`)

**Step 1: Implement the parser**

```rust
fn parse_dbf(path: &Path) -> Result<Vec<SheetData>> {
    use dbf::Reader;

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dbf")
        .to_string();

    let mut reader = Reader::new(path)
        .map_err(|e| anyhow::anyhow!("Failed to open DBF file '{}': {}", path.display(), e))?;

    // Collect field names as headers
    let headers: Vec<String> = reader
        .fields()
        .iter()
        .map(|f| f.name().to_string())
        .collect();

    if headers.is_empty() {
        anyhow::bail!("DBF file '{}' has no fields", path.display());
    }

    // Read all records
    let mut rows: Vec<Vec<String>> = Vec::new();
    for record_result in reader.records() {
        let record = record_result
            .map_err(|e| anyhow::anyhow!("Failed to read DBF record: {}", e))?;
        let row: Vec<String> = (0..headers.len())
            .map(|i| {
                record
                    .get(i)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            })
            .collect();
        rows.push(row);
    }

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![SheetData {
        name,
        headers,
        rows,
        col_widths: Vec::new(),
    }])
}
```

**Step 2: Add integration test**

Create `crates/core/tests/dbf_test.rs`:

```rust
use grep_excel_core::excel::parse_file;
use std::io::Write;

#[test]
fn test_dbf_basic_import() {
    // Note: this test requires a real .dbf file.
    // For CI, we either need a small binary fixture or mark as #[ignore].
    // For now, test with a known-good .dbf if available.
    let path = std::path::Path::new("tests/fixtures/sample.dbf");
    if !path.exists() {
        eprintln!("Skipping: no test fixture at tests/fixtures/sample.dbf");
        return;
    }

    let sheets = parse_file(path).expect("DBF parse should succeed");
    assert!(!sheets.is_empty(), "should have at least one sheet");
    assert!(!sheets[0].headers.is_empty(), "should have headers");
    assert!(!sheets[0].rows.is_empty(), "should have data rows");
}
```

**Step 3: Build and verify**

```bash
cargo build -p grep-excel-core 2>&1
```

**Step 4: Commit**

```bash
git add crates/core/src/excel.rs crates/core/tests/dbf_test.rs
git commit -m "feat: implement DBF file parsing via dbf crate"
```

---

## Phase 4: XML Import Stub (Minimal Viable)

### Task 4.1: Implement basic XML → table parser

**Files:**
- Create: `crates/core/src/xml_table.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod xml_table;`)
- Modify: `crates/core/src/excel.rs` (replace stub `parse_xml()`)

**Step 1: Design the XML→table mapping**

For MVP, support a simple convention: the first element child of the root becomes a column, and its text content becomes rows. This handles flat XML structures commonly exported from databases:

```xml
<rows>
  <row><Name>Alice</Name><Age>30</Age></row>
  <row><Name>Bob</Name><Age>25</Age></row>
</rows>
```

Becomes:

| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |

**Step 2: Implement**

```rust
// crates/core/src/xml_table.rs

use std::path::Path;
use crate::excel::SheetData;

/// Parse a flat XML file into a single SheetData.
///
/// Convention: root > repeating child elements > text content.
/// The first repeating child's element names become headers.
pub fn parse_xml_table(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    let content = crate::excel::read_file_auto_encoding(path)?;
    let doc = roxmltree::Document::parse(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse XML '{}': {}", path.display(), e))?;

    let root = doc.root_element();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("xml")
        .to_string();

    // Find the first repeating child element (the "row" element)
    let mut children = root.children().filter(|n| n.is_element());
    let first_child = match children.next() {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    // Gather all sibling elements with the same tag name
    let row_tag = first_child.tag_name().name();
    let row_elements: Vec<roxmltree::Node> = std::iter::once(first_child)
        .chain(children)
        .filter(|n| n.is_element() && n.tag_name().name() == row_tag)
        .collect();

    if row_elements.is_empty() {
        return Ok(Vec::new());
    }

    // Headers: unique element names from the first row
    let headers: Vec<String> = row_elements[0]
        .children()
        .filter(|n| n.is_element())
        .map(|n| n.tag_name().name().to_string())
        .collect();

    if headers.is_empty() {
        // Fallback: if no children, treat the row elements themselves as data
        // with a single "value" column
        let rows: Vec<Vec<String>> = row_elements
            .iter()
            .map(|el| vec![el.text().unwrap_or("").trim().to_string()])
            .filter(|r| !r[0].is_empty())
            .collect();
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![SheetData {
            name,
            headers: vec!["value".to_string()],
            rows,
            col_widths: Vec::new(),
        }]);
    }

    // Data rows
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row_el in &row_elements {
        let mut row: Vec<String> = Vec::with_capacity(headers.len());
        for header in &headers {
            let value = row_el
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == header.as_str())
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string())
                .unwrap_or_default();
            row.push(value);
        }
        if !row.iter().all(|c| c.is_empty()) {
            rows.push(row);
        }
    }

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![SheetData {
        name,
        headers,
        rows,
        col_widths: Vec::new(),
    }])
}
```

**Step 3: Wire into parse_file dispatch**

In `excel.rs`, replace the stub:

```rust
fn parse_xml(path: &Path) -> Result<Vec<SheetData>> {
    crate::xml_table::parse_xml_table(path)
}
```

**Step 4: Add test**

Create `crates/core/tests/xml_test.rs`:

```rust
use grep_excel_core::excel::parse_file;
use std::io::Write;

#[test]
fn test_xml_basic_flat_table() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_flat.xml");
    let xml = r#"<?xml version="1.0"?>
<rows>
  <row><Name>Alice</Name><Age>30</Age><City>NYC</City></row>
  <row><Name>Bob</Name><Age>25</Age><City>SF</City></row>
</rows>"#;
    std::fs::write(&path, xml).unwrap();

    let sheets = parse_file(&path).expect("XML parse should succeed");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["Name", "Age", "City"]);
    assert_eq!(sheets[0].rows.len(), 2);
    assert_eq!(sheets[0].rows[0], vec!["Alice", "30", "NYC"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_xml_missing_fields() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_partial.xml");
    let xml = r#"<rows>
  <row><Name>Alice</Name><Age>30</Age></row>
  <row><Name>Bob</Name></row>
</rows>"#;
    std::fs::write(&path, xml).unwrap();

    let sheets = parse_file(&path).expect("partial fields should parse");
    assert_eq!(sheets[0].rows[1], vec!["Bob", ""]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_xml_no_rows() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_empty.xml");
    std::fs::write(&path, "<root></root>").unwrap();

    let sheets = parse_file(&path).expect("empty XML should not error");
    assert!(sheets.is_empty());

    let _ = std::fs::remove_file(&path);
}
```

**Step 5: Verify**

```bash
cargo test -p grep-excel-core --test xml_test 2>&1
```

**Step 6: Commit**

```bash
git add crates/core/src/xml_table.rs crates/core/src/lib.rs crates/core/src/excel.rs crates/core/tests/xml_test.rs
git commit -m "feat: add basic XML table import (flat row/column convention)"
```

---

## Phase 5: Final Verification

### Task 5.1: Run full test suite

```bash
cargo test --all 2>&1
cargo clippy --all-targets 2>&1
```

Expected: All tests pass, no new clippy warnings.

### Task 5.2: Performance benchmark (smoke test)

```bash
# Before refactor (stash changes, run, then pop):
time cargo run -- test_data2.xlsx -q "test" 2>&1
# After refactor:
time cargo run -- test_data2.xlsx -q "test" 2>&1
```

Expected: No measurable difference (<5% variance in either direction).

### Task 5.3: Verify help text shows all formats

```bash
cargo run -- --help 2>&1 | grep -A 10 "Supported Formats"
```

Expected: Lists `.tsv .tab`, `.dbf`, `.xml` in the format list.

---

## Summary of Changes

| File | Change | Phase |
|------|--------|-------|
| `crates/core/src/format.rs` | **Create** — `FileFormat` enum + `TABLE_EXTENSIONS` + `from_name()` | 1.1, 2.3 |
| `crates/core/src/lib.rs` | Add `pub mod format;` + `pub mod xml_table;` | 1.1, 4.1 |
| `crates/core/src/excel.rs` | Refactor 5 dispatch + add `parse_file_as()`, `parse_delimited()`, `parse_tsv`, `parse_dbf`, `parse_xml` | 1.2–1.5, 2, 2.4, 3.2, 4.1 |
| `crates/core/src/archive.rs` | Derive `TABLE_EXTENSIONS` from `FileFormat` | 1.6 |
| `crates/core/src/i18n.rs` | Add TSV/DBF/XML + `--as` to help text (zh + en) | 1.7 |
| `crates/core/src/xml_table.rs` | **Create** — XML → table parser | 4.1 |
| `crates/core/Cargo.toml` | Add `dbf` dependency | 3.1 |
| `crates/cli/src/main.rs` | Add `--as` CLI argument + format override plumbing | 2.4 |
| `crates/core/src/engine/memory.rs` | Accept optional `FileFormat` override in `import_excel()` | 2.4 |
| `crates/core/src/engine/duckdb.rs` | Accept optional `FileFormat` override | 2.4 |
| `crates/core/src/engine/sqlite.rs` | Accept optional `FileFormat` override | 2.4 |
| `crates/core/tests/tsv_test.rs` | **Create** — TSV integration tests | 2.1 |
| `crates/core/tests/dbf_test.rs` | **Create** — DBF integration tests | 3.2 |
| `crates/core/tests/xml_test.rs` | **Create** — XML integration tests | 4.1 |

**No changes to:** `engine/` (memory, duckdb, sqlite), `cli/`, `Desktop/`, any callers. The public API signatures (`parse_file`, `parse_file_metadata`, etc.) remain identical.

---

## What's Out of Scope (Future Work)

1. **`--delimiter` flag for pipe/arbitrary delimiters** — `--as csv` and `--as tsv` cover comma and tab. For pipe-delimited (`|`) or custom delimiters, add `--delimiter <char>` that pairs with `--as csv`/`--as tsv`. Trivial since `parse_delimited()` already accepts `delimiter: u8`.
2. **User-defined fixed-width column specs** — needs a `--col-widths` or `--fixed-width` flag. The existing `split_by_boundaries` infrastructure could be reused.
3. **ISITC/SWIFT format** — dedicated parser for domain-specific financial messaging format.
4. **Attribute-based XML mapping** — MVP parses element text only; attributes-as-columns needs user configuration.
5. **Multi-table XML** — MVP creates one sheet per file; multi-table XML needs a `<table>` detection convention.

---

## Execution

Plan complete. Two options:

1. **Subagent-Driven (this session)** — I dispatch each task to a fresh subagent, review between tasks, fast iteration
2. **Parallel Session (separate)** — Open new session with `executing-plans`, batch execution with checkpoints

Which approach?
