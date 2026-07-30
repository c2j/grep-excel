# Skill: Adding File Format Support

Add a new table file format parser to grep-excel. This guide uses the PDF format support implementation as a real-world example.

## Prerequisites

- Rust knowledge and familiarity with the grep-excel codebase
- Working build: `cargo build -p grep-excel --features full`

## Architecture

The import pipeline dispatches files by extension:

```
main.rs (--as override or extension)
  → excel.rs (parse_file / parse_file_with_format)
    → format-specific parser (html_table.rs, text_table.rs, pdf_table.rs, ...)
      → returns Vec<SheetData>
```

Each parser produces `Vec<SheetData>`, where each `SheetData` contains:
- `name: String` — the sheet/table name
- `columns: Vec<String>` — column headers
- `rows: Vec<Vec<String>>` — data rows

## Step-by-Step

### Step 1: Create the Parser Module

Create `crates/core/src/<format>_table.rs`:

```rust
use crate::excel::SheetData;
use anyhow::Result;
use std::path::Path;

pub fn parse_myformat(path: &Path) -> Result<Vec<SheetData>> {
    // 1. Read file content (use read_file_auto_encoding for text formats)
    let content = crate::excel::read_file_auto_encoding(path)?;

    // 2. Parse tables from content
    let mut sheets = Vec::new();

    // 3. For each table found:
    sheets.push(SheetData {
        name: "Sheet Name".to_string(),
        columns: vec!["Col1".to_string(), "Col2".to_string()],
        rows: vec![
            vec!["val1".to_string(), "val2".to_string()],
        ],
    });

    Ok(sheets)
}
```

**Convention**: Return `Ok(vec![])` for files that contain no tables (not an error).

### Step 2: Register in the Format System

Add a variant to the `FileFormat` enum in the `format` module.

In the format dispatch logic, add the mapping from extension to `FileFormat`.

For PDF, this looked like:

```rust
// Extension → FileFormat mapping
"pdf" => FileFormat::Pdf,
```

### Step 3: Add Dispatch in excel.rs

In `crates/core/src/excel.rs`, add a branch in the format dispatch function:

```rust
FileFormat::Pdf => {
    #[cfg(feature = "pdf-support")]
    {
        crate::pdf_table::parse_pdf(path)
    }
    #[cfg(not(feature = "pdf-support"))]
    {
        Err(anyhow::anyhow!("PDF support not enabled (build with --features pdf-support)"))
    }
}
```

The `#[cfg]` pattern ensures the parser is only compiled when the feature flag is active,
with a clear error message otherwise.

### Step 4: Wire Feature Flags

**`crates/core/Cargo.toml`:**
```toml
[features]
myformat-support = ["dep:myformat-parser"]

[dependencies]
myformat-parser = { version = "1.0", optional = true }
```

**`crates/cli/Cargo.toml`:**
```toml
[features]
myformat-support = ["grep-excel-core/myformat-support"]

# Optionally add to `full`:
full = [..., "myformat-support"]
```

### Step 5: Register `--as` Value

If the format should be selectable via `--as`, add it to the valid values in `main.rs`:

```rust
// In the --as help text or value_parser
"myformat"
```

### Step 6: Add Tests

Create `crates/core/tests/myformat_test.rs`:

```rust
use grep_excel_core::excel;

#[test]
fn test_myformat_basic() {
    let fixture = workspace_fixture("tests/fixtures/myformat/sample.myfmt");
    let sheets = excel::parse_file(&fixture).unwrap();
    assert!(!sheets.is_empty());
    assert_eq!(sheets[0].columns, vec!["Col1", "Col2"]);
}
```

Add a test fixture: `tests/fixtures/myformat/sample.myfmt`.

### Step 7: Update Documentation

- Add the new format to the supported formats list in `README.md` and `docs/UserGuide.md`
- Add the feature flag to the feature flags table

## Real Example: PDF Support

The PDF support implementation demonstrates this pattern:

| Component | File |
|---|---|
| Parser | `crates/core/src/pdf_table.rs` |
| Dependency | `pdfsink-rs` (optional) |
| Feature flag (core) | `pdf-support = ["dep:pdfsink-rs"]` |
| Feature flag (cli) | `pdf-support = ["grep-excel-core/pdf-support"]` |
| Dispatch | `excel.rs` — `FileFormat::Pdf` branch with `#[cfg(feature = "pdf-support")]` |
| Tests | `crates/core/tests/pdf_test.rs` |
| Fixture | `tests/fixtures/pdf/sample.pdf` |

Study the PDF implementation as a template for new format parsers.

## Encoding & Text Formats

For text-based formats, use `read_file_auto_encoding()` from `excel.rs`. It handles:
1. UTF-8 (most common)
2. HTML `<meta charset>` detection
3. CJK fallback (GBK, GB18030, Big5, Shift-JIS, EUC-JP, EUC-KR)
4. Lossy UTF-8 as last resort

## Verification

```bash
# Build with the new feature
cargo build -p grep-excel --features myformat-support

# Run format-specific tests
cargo test -p grep-excel-core --test myformat

# Smoke test with a real file
cargo run -p grep-excel --bin grep_excel -- sample.myfmt -t

# Full regression
cargo test -p grep-excel-core --features full
```

## Code Conventions

- **Return empty Vec for "no tables"**: `Ok(vec![])`, not an error
- **Use `read_file_auto_encoding`**: For text-based formats with unknown encoding
- **Feature-gate with clear errors**: `#[cfg(not(feature = "..."))]` with an informative error message
- **`SheetData` columns and rows**: Column count must match row length for each row

## See Also

- [MCP Tool Development](./mcp-tool-development.md) — extend the API surface
- [Developer Guide: Import Pipeline](../docs/DeveloperGuide.md#architecture-overview)
