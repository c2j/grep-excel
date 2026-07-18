use grep_excel_core::excel::parse_file;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn build_docx(document_xml: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("grep_excel_docx_test");
    let _ = std::fs::create_dir_all(&dir);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = dir.join(format!("test_{}_{id}.docx", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default();

    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();
    zip.finish().unwrap();

    path
}

fn wrap_docx_body(inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
{inner}
  </w:body>
</w:document>"#,
    )
}

fn cell(text: &str) -> String {
    format!(
        "<w:tc><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>"
    )
}

fn row(cells: &[&str]) -> String {
    let cells_xml: String = cells.iter().map(|c| cell(c)).collect();
    format!("<w:tr>{cells_xml}</w:tr>")
}

fn table(rows: &[&[&str]]) -> String {
    let rows_xml: String = rows.iter().map(|r| row(r)).collect();
    format!("<w:tbl><w:tblPr/><w:tblGrid/>{rows_xml}</w:tbl>")
}

#[test]
fn parses_simple_docx_table() {
    let body = table(&[
        &["Name", "Age", "City"],
        &["Alice", "30", "Shanghai"],
        &["Bob", "25", "Beijing"],
        &["Carol", "40", "Shenzhen"],
    ]);
    let xml = wrap_docx_body(&body);
    let path = build_docx(&xml);

    let sheets = parse_file(&path).expect("docx parse should succeed");
    assert_eq!(sheets.len(), 1, "should have one sheet");
    let s = &sheets[0];
    assert_eq!(s.name, "Table_1");
    assert_eq!(s.headers, vec!["Name", "Age", "City"]);
    assert_eq!(s.rows.len(), 3);
    assert_eq!(s.rows[0], vec!["Alice", "30", "Shanghai"]);
    assert_eq!(s.rows[1], vec!["Bob", "25", "Beijing"]);
    assert_eq!(s.rows[2], vec!["Carol", "40", "Shenzhen"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn parses_docx_with_no_tables() {
    let xml = wrap_docx_body("<w:p><w:r><w:t>Just a paragraph.</w:t></w:r></w:p>");
    let path = build_docx(&xml);

    let sheets = parse_file(&path).expect("docx with no tables should succeed");
    assert!(sheets.is_empty(), "should return zero sheets");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn parses_docx_with_multiple_tables() {
    let t1 = table(&[&["A1", "B1"], &["a", "b"]]);
    let t2 = table(&[&["X", "Y"], &["1", "2"]]);
    let t3 = table(&[&["P", "Q"], &["p", "q"]]);
    let body = format!("{t1}\n{t2}\n{t3}");
    let xml = wrap_docx_body(&body);
    let path = build_docx(&xml);

    let sheets = parse_file(&path).expect("multi-table docx should parse");
    assert_eq!(sheets.len(), 3);
    assert_eq!(sheets[0].name, "Table_1");
    assert_eq!(sheets[1].name, "Table_2");
    assert_eq!(sheets[2].name, "Table_3");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn skips_docx_table_with_only_header_row() {
    let body = table(&[&["Only", "Headers"]]);
    let xml = wrap_docx_body(&body);
    let path = build_docx(&xml);

    let sheets = parse_file(&path).expect("should parse");
    assert!(sheets.is_empty(), "table with only header row should be skipped");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn docx_horizontal_merge_gridspan() {
    let xml = wrap_docx_body(
        r#"<w:tbl>
  <w:tblPr/><w:tblGrid/>
  <w:tr>
    <w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>Merged</w:t></w:r></w:p></w:tc>
    <w:tc><w:p><w:r><w:t>C</w:t></w:r></w:p></w:tc>
  </w:tr>
  <w:tr>
    <w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
    <w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
    <w:tc><w:p><w:r><w:t>C</w:t></w:r></w:p></w:tc>
  </w:tr>
</w:tbl>"#,
    );
    let path = build_docx(&xml);
    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 1);
    let s = &sheets[0];
    assert_eq!(s.headers, vec!["Merged", "Merged", "C"]);
    assert_eq!(s.rows[0], vec!["A", "B", "C"]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn docx_vertical_merge_vmerge() {
    let xml = wrap_docx_body(
        r#"<w:tbl>
  <w:tblPr/><w:tblGrid/>
  <w:tr>
    <w:tc><w:p><w:r><w:t>Region</w:t></w:r></w:p></w:tc>
    <w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc>
  </w:tr>
  <w:tr>
    <w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>North</w:t></w:r></w:p></w:tc>
    <w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc>
  </w:tr>
  <w:tr>
    <w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p></w:p></w:tc>
    <w:tc><w:p><w:r><w:t>200</w:t></w:r></w:p></w:tc>
  </w:tr>
  <w:tr>
    <w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p></w:p></w:tc>
    <w:tc><w:p><w:r><w:t>300</w:t></w:r></w:p></w:tc>
  </w:tr>
</w:tbl>"#,
    );
    let path = build_docx(&xml);
    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 1);
    let s = &sheets[0];
    assert_eq!(s.headers, vec!["Region", "Value"]);
    assert_eq!(s.rows[0], vec!["North", "100"]);
    assert_eq!(s.rows[1], vec!["North", "200"]);
    assert_eq!(s.rows[2], vec!["North", "300"]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn docx_table_named_from_preceding_heading() {
    let body = r#"<w:p>
  <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
  <w:r><w:t>Quarterly Results</w:t></w:r>
</w:p>"#
        .to_string()
        + &table(&[&["A", "B"], &["1", "2"]]);
    let xml = wrap_docx_body(&body);
    let path = build_docx(&xml);

    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].name, "Quarterly Results");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn docx_table_default_name_when_no_heading() {
    let body = table(&[&["X", "Y"], &["x", "y"]]);
    let xml = wrap_docx_body(&body);
    let path = build_docx(&xml);

    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets[0].name, "Table_1");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn docx_cell_with_multiple_paragraphs_joined_with_newline() {
    let single_cell = "<w:tc><w:p><w:r><w:t>H</w:t></w:r></w:p></w:tc>";
    let multi_para_cell = "<w:tc>\
        <w:p><w:r><w:t>Line 1</w:t></w:r></w:p>\
        <w:p><w:r><w:t>Line 2</w:t></w:r></w:p>\
    </w:tc>";
    let header = format!("<w:tr></w:tr>");
    let data_row = format!("<w:tr>{single_cell}{multi_para_cell}</w:tr>");
    let body = format!("<w:tbl><w:tblPr/><w:tblGrid/>{header}{data_row}</w:tbl>");
    let xml = wrap_docx_body(&body);
    let path = build_docx(&xml);

    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 1);
    let row = &sheets[0].rows[0];
    assert_eq!(row.len(), 2);
    assert_eq!(row[1], "Line 1\nLine 2");

    let _ = std::fs::remove_file(&path);
}
