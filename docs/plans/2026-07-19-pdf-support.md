# PDF Table Extraction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add PDF table extraction support using pdfsink-rs (MIT, pure Rust), feature-gated behind `pdf-support`.

**Architecture:** New `pdf_table` module mirrors `docx_table.rs` pattern (`parse_pdf(path) -> Result<Vec<SheetData>>`). Wired into the `FileFormat` registry and 4 dispatch sites in `excel.rs`. Feature-gated to avoid adding the dependency for all builds — sets a new precedent (no existing format is feature-gated) justified by PDF being a heavyweight, heuristic-dependent format.

**Tech Stack:** pdfsink-rs (MIT, pure Rust, pdfplumber-compatible API), feature gate `pdf-support` in both core and CLI crates.

---

## Pre-condition Checks

Before starting any task, verify:
- `cargo build -p grep-excel` compiles cleanly
- `cargo test -p grep-excel-core` passes all tests
- No uncommitted changes in working tree

```bash
git status --short && cargo build -p grep-excel 2>&1 | tail -3 && cargo test -p grep-excel-core 2>&1 | tail -5
```

---

### Task 0: Add test fixtures

**Files:**
- Create: `tests/fixtures/pdf/` (directory)

**Step 0.1: Create directory and add a real PDF with tables**

Create `tests/fixtures/pdf/` directory. Obtain a small text-based PDF containing at least one table (e.g. a financial report table, a CSV exported as PDF). Name it `simple.pdf`.

Also create `tests/fixtures/pdf/README.md` explaining the source.

```bash
mkdir -p tests/fixtures/pdf
```

**Step 0.2: Commit**

```bash
git add tests/fixtures/pdf/
git commit -m "test(pdf): add PDF test fixture directory"
```

---

### Task 1: Add `pdf-support` feature and dependency

**Files:**
- Modify: `crates/core/Cargo.toml` — add dep + feature
- Modify: `crates/cli/Cargo.toml` — propagate feature

**Step 1.1: Add pdfsink-rs to core Cargo.toml**

In `crates/core/Cargo.toml`, add after `dbase = "0.8"`:

```toml
# Optional: PDF table extraction
pdfsink-rs = { version = "0.2", optional = true }
```

In `[features]`, add after `archive-support = [...]`:

```toml
pdf-support = ["dep:pdfsink-rs"]
```

**Step 1.2: Propagate feature to cli Cargo.toml**

In `crates/cli/Cargo.toml`, add to `[features]`:

```toml
pdf-support = ["grep-excel-core/pdf-support"]
```

Update `full` to include it:

```toml
full = ["engine-memory", "file-dialog", "mcp-server", "share-url", "archive-support", "pdf-support"]
```

**Step 1.3: Verify features compile**

```bash
cargo build -p grep-excel --features full 2>&1 | tail -5
```

Expected: compiles (pdfsink-rs and its deps resolve and build).

**Step 1.4: Commit**

```bash
git add crates/core/Cargo.toml crates/cli/Cargo.toml
git commit -m "feat(pdf): add pdfsink-rs dependency behind pdf-support feature"
```

---

### Task 2: Add `FileFormat::Pdf` variant

**Files:**
- Modify: `crates/core/src/format.rs`

**Step 2.1: Add Pdf variant to enum**

After the `Pptx` variant (line 27), add:

```rust
    /// PDF documents (pdfsink-rs): .pdf — table extraction via lattice/text strategies
    Pdf,
```

**Step 2.2: Add extension mapping in `from_path()`**

Add after `pptx` branch (line 56), before the final `else`:

```rust
        } else if ext.eq_ignore_ascii_case("pdf") {
            Some(Self::Pdf)
```

**Step 2.3: Add name mapping in `from_name()`**

Add to the match body:

```rust
            "pdf" => Some(Self::Pdf),
```

**Step 2.4: Add to `ALL_NAMES`**

Append `"pdf"`:

```rust
    pub const ALL_NAMES: &[&str] = &[
        "csv", "tsv", "html", "txt", "md", "dbf", "xml", "excel", "docx", "pptx", "pdf",
    ];
```

**Step 2.5: Add to `TABLE_EXTENSIONS`**

Append `"pdf"`:

```rust
    pub const TABLE_EXTENSIONS: &[&str] = &[
        "xlsx", "xls", "xlsm", "xlsb", "ods",
        "csv", "tsv", "tab",
        "html", "htm",
        "txt", "md", "markdown",
        "dbf",
        "xml",
        "docx", "pptx",
        "pdf",
    ];
```

**Step 2.6: Update tests**

In `from_name_invalid()` (line 119-123), remove the `"pdf"` assertion line. Add a new test:

```rust
    #[test]
    fn from_name_pdf() {
        assert_eq!(FileFormat::from_name("pdf"), Some(FileFormat::Pdf));
        assert_eq!(FileFormat::from_name("PDF"), Some(FileFormat::Pdf));
    }

    #[test]
    fn from_path_pdf() {
        use std::path::Path;
        assert_eq!(FileFormat::from_path(Path::new("report.pdf")), Some(FileFormat::Pdf));
        assert_eq!(FileFormat::from_path(Path::new("REPORT.PDF")), Some(FileFormat::Pdf));
    }

    #[test]
    fn table_extensions_include_pdf() {
        assert!(FileFormat::TABLE_EXTENSIONS.contains(&"pdf"));
    }
```

**Step 2.7: Build and test**

```bash
cargo test -p grep-excel-core -- format::tests
```

Expected: All format tests pass including new Pdf tests.

**Step 2.8: Commit**

```bash
git add crates/core/src/format.rs
git commit -m "feat(pdf): add FileFormat::Pdf variant with extension/name mapping"
```

---

### Task 3: Create `pdf_table.rs` parser module

**Files:**
- Create: `crates/core/src/pdf_table.rs`

**Step 3.1: Write the module (feature-gated)**

```rust
use std::path::Path;

use crate::excel::SheetData;

#[cfg(feature = "pdf-support")]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    use pdfsink_rs::{PdfDocument, TableSettings};

    let pdf = PdfDocument::open(path)
        .map_err(|e| anyhow::anyhow!("failed to open PDF '{}': {}", path.display(), e))?;

    let page_count = pdf.page_count();
    let mut all_tables: Vec<SheetData> = Vec::new();

    for page_num in 0..page_count {
        let page = pdf
            .page(page_num)
            .map_err(|e| anyhow::anyhow!("failed to read page {} of '{}': {}", page_num + 1, path.display(), e))?;

        // Try lattice strategy first (bordered tables, highest accuracy)
        if let Some(table) = page
            .extract_table(TableSettings::default())
            .map_err(|e| anyhow::anyhow!("table extraction failed on page {}: {}", page_num + 1, e))?
        {
            let string_table = table
                .into_iter()
                .map(|row| row.into_iter().map(|c| c.unwrap_or_default()).collect::<Vec<_>>())
                .collect::<Vec<_>>();

            if string_table.len() >= 2 && !string_table[0].is_empty() {
                let headers = string_table[0].clone();
                let rows = string_table[1..].to_vec();
                let name = if page_count == 1 {
                    format!("Table_1")
                } else {
                    format!("Page_{}_Table_{}", page_num + 1, all_tables.len() + 1)
                };
                all_tables.push(SheetData {
                    name,
                    headers,
                    rows,
                    col_widths: Vec::new(),
                });
            }
        }
    }

    if all_tables.is_empty() {
        anyhow::bail!(
            "No tables extracted from PDF '{}'. The PDF may not contain bordered tables, or may be scanned (OCR not supported).",
            path.display()
        );
    }

    Ok(all_tables)
}

#[cfg(not(feature = "pdf-support"))]
pub fn parse_pdf(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    let _ = path;
    anyhow::bail!(
        "PDF support is not enabled. Rebuild with --features pdf-support to enable PDF table extraction."
    )
}
```

**Step 3.2: Verify compilation with and without feature**

```bash
# With feature
cargo build -p grep-excel-core --features pdf-support 2>&1 | tail -3
# Without feature (should compile the stub)
cargo build -p grep-excel-core 2>&1 | tail -3
```

Expected: Both compile successfully.

**Step 3.3: Commit**

```bash
git add crates/core/src/pdf_table.rs
git commit -m "feat(pdf): add pdf_table parser module with pdfsink-rs integration"
```

---

### Task 4: Register module and wire dispatch

**Files:**
- Modify: `crates/core/src/lib.rs` — add `pub mod pdf_table;`
- Modify: `crates/core/src/excel.rs` — add Pdf arms to 4 dispatch sites

**Step 4.1: Register module in lib.rs**

Add after `pub mod pptx_table;` (line 9):

```rust
pub mod pdf_table;
```

**Step 4.2: Wire `parse_file_as` dispatch (line 87-97)**

Add after the `FileFormat::Pptx` arm:

```rust
        FileFormat::Pdf => parse_pdf(path),
```

Full context — the match block becomes:

```rust
    match fmt {
        FileFormat::Csv => parse_delimited(path, b','),
        FileFormat::Tsv => parse_delimited(path, b'\t'),
        FileFormat::Html => parse_html(path),
        FileFormat::Text | FileFormat::Markdown => parse_text(path),
        FileFormat::Dbf => parse_dbf(path),
        FileFormat::Xml => parse_xml(path),
        FileFormat::Docx => parse_docx(path),
        FileFormat::Pptx => parse_pptx(path),
        FileFormat::Pdf => parse_pdf(path),
        FileFormat::Excel => parse_excel(path),
    }
```

Add the import at the top:

```rust
use crate::pdf_table::parse_pdf;
```

**Step 4.3: Wire `parse_file_metadata` dispatch (line 724-732)**

Add `Pdf` to the `metadata_from_full_parse` group:

```rust
        Some(FileFormat::Dbf) | Some(FileFormat::Xml) | Some(FileFormat::Docx) | Some(FileFormat::Pptx) | Some(FileFormat::Pdf) => metadata_from_full_parse(path),
```

**Step 4.4: Wire `parse_file_repair` dispatch (line 1028-1038)**

Add after `FileFormat::Pptx`:

```rust
        Some(FileFormat::Pdf) => parse_pdf(path),
```

**Step 4.5: Wire `for_each_sheet` dispatch (line 825-857)**

Add `Pdf` to the fallback `_` arm group. The `_` arm at line 847 already covers `Dbf, Xml, Docx, Pptx, Pdf` via the fallback. Confirm the `_` arm handles it — it does, since it calls `parse_file(path)` which now knows about Pdf. No change needed.

**Step 4.6: Wire `for_each_sheet_repair` dispatch (line 1041+)**

Add `Pdf` to the group at line 1046:

```rust
        Some(FileFormat::Csv) | Some(FileFormat::Tsv) | Some(FileFormat::Dbf) | Some(FileFormat::Xml) | Some(FileFormat::Docx) | Some(FileFormat::Pptx) | Some(FileFormat::Pdf) => {
```

**Step 4.7: Build and test**

```bash
cargo build -p grep-excel --features pdf-support 2>&1 | tail -3
cargo test -p grep-excel-core --features pdf-support 2>&1 | tail -10
```

Expected: Compiles, all tests pass.

**Step 4.8: Commit**

```bash
git add crates/core/src/lib.rs crates/core/src/excel.rs
git commit -m "feat(pdf): wire Pdf format into all dispatch sites"
```

---

### Task 5: Update archive support

**Files:**
- Modify: `crates/core/src/archive.rs` — update `is_internally_zip_table_format` test assertion
- Modify: `crates/core/src/archive.rs` — `is_table_entry` now includes pdf via TABLE_EXTENSIONS

**Step 5.1: Verify pdf is NOT in `is_internally_zip_table_format`**

PDF files are NOT internally ZIP format, so no exclusion needed. No change.

**Step 5.2: `is_table_entry` automatically picks up pdf from TABLE_EXTENSIONS**

`is_table_entry` reads from `TABLE_EXTENSIONS` which already includes `"pdf"` since Task 2. No change needed.

**Step 5.3: Update test that asserts pdf is NOT a table entry**

Find and update the test at line ~453:

```rust
// Change from:
// assert!(!is_table_entry("readme.pdf"));
// To:
// assert!(is_table_entry("readme.pdf"));
```

Verify the test location:

```bash
grep -n "readme.pdf" crates/core/src/archive.rs
```

**Step 5.4: Verify tests pass**

```bash
cargo test -p grep-excel-core -- archive::tests --features pdf-support
```

**Step 5.5: Commit**

```bash
git add crates/core/src/archive.rs
git commit -m "fix(archive): update is_table_entry test for pdf support"
```

---

### Task 6: Add regression test

**Files:**
- Create: `crates/core/tests/pdf.rs` (feature-gated)

**Step 6.1: Write the test**

```rust
#[cfg(feature = "pdf-support")]
mod pdf_tests {
    use grep_excel_core::excel;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        PathBuf::from(manifest).join("../..").join("tests/fixtures/pdf").join(name)
    }

    #[test]
    fn parse_simple_pdf() {
        let path = fixture("simple.pdf");
        let sheets = excel::parse_file_as(&path, None).expect("should parse simple.pdf");
        assert!(!sheets.is_empty(), "should extract at least one table");
        for sheet in &sheets {
            assert!(!sheet.headers.is_empty(), "table '{}' should have headers", sheet.name);
            assert!(!sheet.rows.is_empty(), "table '{}' should have rows", sheet.name);
        }
    }
}
```

**Step 6.2: Run the test**

```bash
cargo test -p grep-excel-core --test pdf --features pdf-support
```

Expected: PASS (if fixtures/simple.pdf exists and has tables).

If the fixture PDF doesn't exist yet, create a minimal one or adjust the test to skip gracefully.

**Step 6.3: Commit**

```bash
git add crates/core/tests/pdf.rs
git commit -m "test(pdf): add regression test for PDF table extraction"
```

---

### Task 7: Update i18n error messages

**Files:**
- Modify: `crates/core/src/i18n.rs`

**Step 7.1: Add PDF-specific error message**

Add at end of file (before any closing sections):

```rust
pub fn pdf_no_tables(path: &str) -> String {
    match current() {
        Lang::Zh => format!("PDF 中未提取到表格 '{}'。文件可能不含带边框表格，或为扫描件（暂不支持 OCR）。", path),
        Lang::En => format!("No tables extracted from PDF '{}'. The file may not contain bordered tables, or may be scanned (OCR not supported).", path),
    }
}

pub fn pdf_not_enabled() -> &'static str {
    match current() {
        Lang::Zh => "PDF 支持未启用。请使用 --features pdf-support 重新编译以启用 PDF 表格提取。",
        Lang::En => "PDF support is not enabled. Rebuild with --features pdf-support to enable PDF table extraction.",
    }
}
```

**Step 7.2: Commit**

```bash
git add crates/core/src/i18n.rs
git commit -m "feat(pdf): add i18n messages for PDF extraction errors"
```

---

### Task 8: Update README (bilingual)

**Files:**
- Modify: `README.md`

**Step 8.1: Add `.pdf` to both English and Chinese format lists**

In English "Supported Formats" section, add after `.pptx` line:

```
- `.pdf` — PDF documents (text-based, table extraction via lattice/text strategies; read-only, no OCR)
```

In Chinese "支持的文件格式" section, add after `.pptx` line:

```
- `.pdf` — PDF 文档（文本型，通过线条/对齐策略提取表格；只读，不支持 OCR）
```

**Step 8.2: Update `--as` valid values in both languages**

Add `pdf` to the list. In English CLI help text around line 1153+ and in Chinese around line 1082+, add `pdf` to the `--as` format list description.

Actually, the `--as` help text is generated dynamically from `FileFormat::ALL_NAMES` in `main.rs:41`, which already includes `"pdf"` since Task 2. So the CLI help auto-updates. Only the README needs manual update for the `--as` section.

**Step 8.3: Commit**

```bash
git add README.md
git commit -m "docs(pdf): add PDF to supported formats in README (bilingual)"
```

---

### Task 9: Final verification

**Step 9.1: Build full features**

```bash
cargo build -p grep-excel --features full
```

**Step 9.2: Run all tests**

```bash
cargo test -p grep-excel-core --features pdf-support
cargo test -p grep-excel --features full
```

**Step 9.3: Clippy**

```bash
cargo clippy -p grep-excel --features full -- -D warnings
```

**Step 9.4: Manual smoke test**

```bash
# With a real PDF file containing a table
cargo run -p grep-excel --features full -- tests/fixtures/pdf/simple.pdf -t
```

**Step 9.5: Commit any fixups**

```bash
git add -A
git commit -m "chore(pdf): final verification fixes"
```

---

## Summary

| Task | Description | Files | Commits |
|------|-------------|-------|---------|
| 0 | Test fixture | `tests/fixtures/pdf/` | 1 |
| 1 | Dependency + feature gate | `Cargo.toml` (×2) | 1 |
| 2 | `FileFormat::Pdf` variant | `format.rs` | 1 |
| 3 | `pdf_table.rs` parser | `pdf_table.rs` | 1 |
| 4 | Dispatch wiring | `lib.rs`, `excel.rs` | 1 |
| 5 | Archive support | `archive.rs` | 1 |
| 6 | Regression test | `tests/pdf.rs` | 1 |
| 7 | i18n messages | `i18n.rs` | 1 |
| 8 | README update | `README.md` | 1 |
| 9 | Final verification | — | 0-1 |

**Total: ~10 commits, 8 files modified, 2 files created.**

**Key design decisions:**
- Feature-gated (new precedent, justified: PDF is heavyweight + heuristic)
- Only lattice strategy for MVP (bordered tables, highest accuracy); text strategy deferred
- No OCR support (Phase 3 future work)
- Error on zero tables extracted (to avoid silent failures)
- `pdf-support` included in `full` feature
