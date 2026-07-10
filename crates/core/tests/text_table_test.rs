//! Regression tests for text/markdown table extraction (`text_table` module).
//!
//! These tests exercise GFM pipe table parsing and TXT alignment-based
//! table detection.
//!
//! Test fixtures:
//!   - tests/regress/sample_tables.md (workspace root)
//!   - tests/regress/awr.txt (workspace root)

use grep_excel_core::excel::parse_file;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Resolve a fixture path relative to the workspace root.
/// When `cargo test -p grep-excel-core` runs, CWD is `crates/core/`,
/// so relative paths like `tests/regress/awr.txt` don't resolve.
/// This helper uses CARGO_MANIFEST_DIR to navigate to the workspace root.
fn workspace_fixture(relative: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR is crates/core/, workspace root is two levels up
    let workspace = manifest.parent().unwrap().parent().unwrap();
    workspace.join(relative)
}

// ── Helper: write content to a unique temp file, parse, return sheets ─────────
// Use an atomic counter so parallel tests don't collide on the same filename.

static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

fn parse_text_content(content: &str, ext: &str) -> Vec<grep_excel_core::excel::SheetData> {
    let dir = std::env::temp_dir();
    let id = TMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = dir.join(format!("test_text_table_{}.{}", id, ext));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    let result = parse_file(&path);
    std::fs::remove_file(&path).ok();
    result.expect(&format!("parsing .{} should succeed", ext))
}

// ═══════════════════════════════════════════════════════════════════════════════
// MD: GFM Pipe Table Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]

fn md_basic_pipe_table() {
    let md = r#"| Name | Age | City |
|---|---|---|
| Alice | 30 | NYC |
| Bob | 25 | SF |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["Name", "Age", "City"]);
    assert_eq!(sheets[0].rows.len(), 2);
    assert_eq!(sheets[0].rows[0], vec!["Alice", "30", "NYC"]);
    assert_eq!(sheets[0].rows[1], vec!["Bob", "25", "SF"]);
}

#[test]

fn md_alignment_colons() {
    let md = r#"| Left | Center | Right |
|:-----|:------:|------:|
| A | B | C |
| D | E | F |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["Left", "Center", "Right"]);
    assert_eq!(sheets[0].rows.len(), 2);
}

#[test]

fn md_no_separator_line() {
    let md = r#"| H1 | H2 |
| A | B |
| C | D |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["H1", "H2"]);
    assert_eq!(sheets[0].rows.len(), 2);
}

#[test]

fn md_empty_cells() {
    let md = r#"| Col1 | Col2 | Col3 |
|---|---|---|
| a | | c |
| | b | |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].rows[0], vec!["a", "", "c"]);
    assert_eq!(sheets[0].rows[1], vec!["", "b", ""]);
}

#[test]

fn md_single_column() {
    let md = r#"| X |
|---|
| 1 |
| 2 |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["X"]);
    assert_eq!(sheets[0].rows.len(), 2);
}

#[test]

fn md_multiple_tables_no_gap() {
    let md = r#"| A | B |
|---|---|
| 1 | 2 |

| C | D |
|---|---|
| 3 | 4 |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets[0].headers, vec!["A", "B"]);
    assert_eq!(sheets[1].headers, vec!["C", "D"]);
}

#[test]

fn md_section_heading_as_name() {
    let md = r#"## Performance Metrics

| Metric | Value |
|--------|-------|
| CPU | 45% |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].name, "Performance Metrics");
}

#[test]

fn md_multiple_sections_multiple_tables() {
    let md = r#"## Section One

| A | B |
|---|---|
| 1 | 2 |

## Section Two

| C | D |
|---|---|
| 3 | 4 |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets[0].name, "Section One");
    assert_eq!(sheets[1].name, "Section Two");
}

#[test]

fn md_code_block_skips_tables() {
    let md = r#"```markdown
| This | Should | Not |
|---|---|---|
| be | extracted | ! |
```

| Real | Table |
|---|---|
| foo | bar |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1, "tables inside code blocks should be skipped");
    assert_eq!(sheets[0].headers, vec!["Real", "Table"]);
}

#[test]

fn md_ragged_columns() {
    let md = r#"| H1 | H2 | H3 |
|---|---|---|
| a | b |
| c | d | e | f |
| g |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    // Jagged rows: each row has its actual cell count
    assert_eq!(sheets[0].rows[0].len(), 2, "row 0 has 2 cells");
    assert_eq!(sheets[0].rows[1].len(), 4, "row 1 has 4 cells");
    assert_eq!(sheets[0].rows[2].len(), 1, "row 2 has 1 cell");
}

#[test]

fn md_no_tables() {
    let md = r#"This is just a paragraph.
No tables here at all.
"#;
    let sheets = parse_text_content(md, "md");
    assert!(sheets.is_empty(), "no pipe tables -> empty result");
}

#[test]

fn md_empty_file() {
    let sheets = parse_text_content("", "md");
    assert!(sheets.is_empty(), "empty file -> empty result");
}

#[test]

fn md_pipe_trims() {
    let md = r#"| X | Y | Z |
|---|---|---|
| 1 | 2 | 3 |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].rows[0], vec!["1", "2", "3"]);
}

#[test]

fn md_data_rows_without_separator() {
    let md = r#"| a | b |
| c | d |
"#;
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["a", "b"]);
    assert_eq!(sheets[0].rows[0], vec!["c", "d"]);
}

#[test]

fn md_no_trailing_newline() {
    let md = "| H |\n|---|\n| v |";
    let sheets = parse_text_content(md, "md");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["H"]);
    assert_eq!(sheets[0].rows[0], vec!["v"]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// TXT: Text Table Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]

fn txt_standard_section() {
    let txt = r#"Load Profile
~~~~~~~~~~~~
Metric         Value
------         -----
DB Time(s)     4.1
CPU Time(s)    2.3
"#;
    let sheets = parse_text_content(txt, "txt");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].name, "Load Profile");
    assert_eq!(sheets[0].headers, vec!["Metric", "Value"]);
    assert_eq!(sheets[0].rows.len(), 2);
    assert!(sheets[0].rows[0][0].contains("DB Time"));
}

#[test]

fn txt_multi_section_multi_table() {
    let txt = r#"Section One
~~~~~~~~~~~
ColA    ColB
----    ----
1       2
3       4

Section Two
~~~~~~~~~~~
X        Y        Z
-        -        -
a        b        c
"#;
    let sheets = parse_text_content(txt, "txt");
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets[0].name, "Section One");
    assert_eq!(sheets[1].name, "Section Two");
}

#[test]

fn txt_no_dash_separator() {
    let txt = r#"Host CPU
~~~~~~~~
%User  %System  %Idle
 45.2    12.3    32.1
 67.8     8.9    23.3
"#;
    let sheets = parse_text_content(txt, "txt");
    assert_eq!(sheets.len(), 1, "should detect table without dash separator");
    assert_eq!(sheets[0].name, "Host CPU");
    // At minimum, should find 2 data rows
    assert_eq!(sheets[0].rows.len(), 2);
}

#[test]

fn txt_mixed_prose_and_tables() {
    let txt = r#"Some introductory text that is not a table.

First Table
~~~~~~~~~~~~
K    V
-    -
a    1

Here is some explanation between tables.

Second Table
~~~~~~~~~~~~
X    Y
-    -
b    2
"#;
    let sheets = parse_text_content(txt, "txt");
    assert_eq!(sheets.len(), 2, "prose between sections should be skipped");
    assert_eq!(sheets[0].name, "First Table");
    assert_eq!(sheets[1].name, "Second Table");
}

#[test]

fn txt_no_tables() {
    let txt = r#"This is a plain text file.
There are no tables here.
Just paragraphs of text.
"#;
    let sheets = parse_text_content(txt, "txt");
    assert!(sheets.is_empty(), "no tables in text -> empty result");
}

#[test]

fn txt_empty_file() {
    let sheets = parse_text_content("", "txt");
    assert!(sheets.is_empty(), "empty file -> empty result");
}

// ═══════════════════════════════════════════════════════════════════════════════
// REGRESSION: Real File Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]

fn regress_awr_txt() {
    let path = workspace_fixture("tests/regress/awr.txt");
    let sheets = parse_file(&path).expect("awr.txt should parse");
    assert!(!sheets.is_empty(), "awr.txt should yield at least one table");
    // Expect 15+ tables from the AWR report
    assert!(sheets.len() >= 15, "expected >=15 tables from awr.txt, got {}", sheets.len());
    // First table should have a name and headers
    assert!(!sheets[0].name.is_empty());
    assert!(!sheets[0].headers.is_empty());
    // Verify some known table names.
    // Note: "Load Profile" uses a hybrid TXT format (title + headers on same
    // line) not handled by the parser, so it won't appear in .txt output.
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Instance Activity Stats"),
        "should find 'Instance Activity Stats', got: {:?}", names
    );
}

#[test]

fn regress_awr_md() {
    let path = workspace_fixture("tests/regress/awr.md");
    let sheets = parse_file(&path).expect("awr.md should parse");
    assert!(!sheets.is_empty(), "awr.md should yield at least one table");
    // Expect 18+ pipe tables from the MD AWR report
    assert!(sheets.len() >= 18, "expected >=18 tables from awr.md, got {}", sheets.len());
    // First table should have a name and headers
    assert!(!sheets[0].name.is_empty());
    assert!(!sheets[0].headers.is_empty());
    // Verify pipe tables parsed correctly — section headings become names.
    // "Load Profile" IS found in .md output because pipe tables are parsed
    // via heading tracking, not tilde-section segmentation.
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Load Profile"),
        "should find 'Load Profile' in MD output, got: {:?}", names
    );
    assert!(
        names.contains(&"Top 5 Timed Foreground Events"),
        "should find 'Top 5 Timed Foreground Events', got: {:?}", names
    );
    assert!(
        names.contains(&"Wait Event Histogram"),
        "should find 'Wait Event Histogram', got: {:?}", names
    );
}

#[test]

fn regress_sample_md() {
    let path = workspace_fixture("tests/regress/sample_tables.md");
    let sheets = parse_file(&path).expect("sample_tables.md should parse");
    assert!(!sheets.is_empty(), "sample_tables.md should yield tables");
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Basic Table"),
        "should find 'Basic Table', got: {:?}", names
    );
    assert!(
        names.contains(&"Performance Metrics"),
        "should find 'Performance Metrics', got: {:?}", names
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// FORMAT: Extension Recognition
// ═══════════════════════════════════════════════════════════════════════════════

#[test]

fn txt_extension_supported() {
    let content = "Title\n~~~~~\nK    V\n-    -\na    1\n";
    let sheets = parse_text_content(content, "txt");
    assert_eq!(sheets.len(), 1, ".txt files should be recognized");
}

#[test]

fn md_extension_supported() {
    let content = "| H |\n|---|---|\n| v |\n";
    let sheets = parse_text_content(content, "md");
    assert_eq!(sheets.len(), 1, ".md files should be recognized");
}
