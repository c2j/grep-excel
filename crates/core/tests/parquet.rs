#![cfg(feature = "parquet-support")]

use grep_excel_core::engine::SearchEngine;
use std::path::PathBuf;

fn workspace_fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

#[test]
fn parse_parquet_sample() {
    let path = workspace_fixture("tests/fixtures/parquet/sample.parquet");
    let sheets = grep_excel_core::excel::parse_file(&path).unwrap();
    assert_eq!(sheets.len(), 1);
    let sheet = &sheets[0];
    assert_eq!(sheet.headers, vec!["id", "name", "score", "active"]);
    assert_eq!(sheet.rows.len(), 3);
    assert_eq!(sheet.rows[0][1], "Alice");
    assert_eq!(sheet.rows[2][1], "Charlie");
}

#[test]
fn search_parquet_via_memory_engine() {
    let path = workspace_fixture("tests/fixtures/parquet/sample.parquet");
    let mut engine = grep_excel_core::engine::DefaultEngine::new().unwrap();
    let info = engine.import_excel(&path, &|_, _| {}).unwrap();
    assert_eq!(info.sheets.len(), 1);
    assert_eq!(info.sheets[0].0, "sample");
    assert_eq!(info.sheets[0].1, 3);
    assert_eq!(info.total_rows, 3);
}

#[test]
fn parquet_file_format_detection() {
    use grep_excel_core::format::FileFormat;
    let path = workspace_fixture("tests/fixtures/parquet/sample.parquet");
    assert_eq!(FileFormat::from_path(&path), Some(FileFormat::Parquet));
    assert_eq!(FileFormat::from_name("parquet"), Some(FileFormat::Parquet));
}
