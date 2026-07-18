use grep_excel_core::excel::{parse_file, parse_file_metadata};

#[test]
fn test_tsv_basic_import() {
    let dir = std::env::temp_dir();
    let path = dir.join("gre_test_basic.tsv");
    std::fs::write(&path, "Name\tAge\tCity\nAlice\t30\tNYC\nBob\t25\tSF\n").unwrap();

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
    let path = dir.join("gre_test_quoted.tsv");
    std::fs::write(&path, "Col1\tCol2\na\t\"tab\tinside\"\n").unwrap();

    let sheets = parse_file(&path).expect("quoted TSV should parse");
    assert_eq!(sheets[0].rows[0], vec!["a", "tab\tinside"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tsv_metadata() {
    let dir = std::env::temp_dir();
    let path = dir.join("gre_test_meta.tsv");
    std::fs::write(&path, "A\tB\tC\n1\t2\t3\n4\t5\t6\n7\t8\t9\n").unwrap();

    let meta = parse_file_metadata(&path).expect("TSV metadata should work");
    assert_eq!(meta.len(), 1);
    assert_eq!(meta[0].headers, vec!["A", "B", "C"]);
    assert_eq!(meta[0].row_count, 3);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tsv_empty_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("gre_test_empty.tsv");
    std::fs::write(&path, "").unwrap();

    let sheets = parse_file(&path).expect("empty TSV should not error");
    assert!(sheets.is_empty());

    let _ = std::fs::remove_file(&path);
}
