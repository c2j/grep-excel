use std::path::Path;

#[test]
fn test_parse_excel() {
    let sheets = grep_excel::excel::parse_excel(Path::new("test_data.xlsx"))
        .expect("parse_excel should succeed");
    assert_eq!(sheets.len(), 3);
    assert_eq!(sheets[0].name, "Employees");
    assert_eq!(sheets[0].rows.len(), 7);
    assert_eq!(sheets[1].name, "Products");
    assert_eq!(sheets[2].name, "Orders");
}

#[test]
fn test_database_search() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    let info = db
        .import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");
    assert_eq!(info.sheets.len(), 3);
    assert_eq!(info.total_rows, 15);

    let query = grep_excel::database::SearchQuery {
        text: "Engineering".into(),
        column: None,
        mode: grep_excel::database::SearchMode::FullText,
        limit: 100,
    };
    let (results, stats) = db.search(&query).expect("search");
    assert_eq!(results.len(), 4);
    assert_eq!(stats.total_matches, 4);
}

#[test]
fn test_exact_match() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = grep_excel::database::SearchQuery {
        text: "Engineering".into(),
        column: Some("Department".into()),
        mode: grep_excel::database::SearchMode::ExactMatch,
        limit: 100,
    };
    let (results, _) = db.search(&query).expect("search");
    assert_eq!(results.len(), 4);
}

#[test]
fn test_wildcard_search() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = grep_excel::database::SearchQuery {
        text: "San%".into(),
        column: Some("City".into()),
        mode: grep_excel::database::SearchMode::Wildcard,
        limit: 100,
    };
    let (results, _) = db.search(&query).expect("search");
    assert_eq!(results.len(), 4);
}

#[test]
fn test_regex_search() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = grep_excel::database::SearchQuery {
        text: "Engineering|Marketing".into(),
        column: None,
        mode: grep_excel::database::SearchMode::Regex,
        limit: 100,
    };
    let (results, _) = db.search(&query).expect("search");
    assert!(!results.is_empty());
    for result in &results {
        assert!(!result.matched_columns.is_empty());
    }
}

#[test]
fn test_search_no_results() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = grep_excel::database::SearchQuery {
        text: "ZZZNONEXISTENT".into(),
        column: None,
        mode: grep_excel::database::SearchMode::FullText,
        limit: 100,
    };
    let (results, stats) = db.search(&query).expect("search");
    assert!(results.is_empty());
    assert_eq!(stats.total_matches, 0);
}

#[test]
fn test_matched_columns_highlight() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = grep_excel::database::SearchQuery {
        text: "Engineering".into(),
        column: Some("Department".into()),
        mode: grep_excel::database::SearchMode::ExactMatch,
        limit: 100,
    };
    let (results, _) = db.search(&query).expect("search");
    assert!(!results.is_empty());
    for result in &results {
        let dept_col = result.col_names.iter().position(|c| c == "Department");
        if let Some(idx) = dept_col {
            assert!(result.matched_columns.contains(&idx));
        }
    }
}

#[test]
fn test_multi_file_import() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    let info1 = db
        .import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import1");
    let info2 = db
        .import_excel(Path::new("test_data2.xlsx"), |_, _| {})
        .expect("import2");

    let files = db.list_files();
    assert_eq!(files.len(), 2);

    let query = grep_excel::database::SearchQuery {
        text: "a".into(),
        column: None,
        mode: grep_excel::database::SearchMode::FullText,
        limit: 1000,
    };
    let (results, _) = db.search(&query).expect("search");
    let from_file1 = results.iter().filter(|r| r.file_name == info1.name).count();
    let from_file2 = results.iter().filter(|r| r.file_name == info2.name).count();
    assert!(from_file1 > 0 || from_file2 > 0);
}

#[test]
fn test_import_file_info() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    let info = db
        .import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    assert!(!info.name.is_empty());
    assert_eq!(info.sheets.len(), 3);
    assert!(info.total_rows > 0);
    assert!(info.sample.is_some());

    let sample = info.sample.unwrap();
    assert!(!sample.headers.is_empty());
    assert_eq!(sample.rows.len(), 3);
}

#[test]
fn test_clear_database() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");
    assert!(!db.list_files().is_empty());

    db.clear().expect("clear");
    assert!(db.list_files().is_empty());
}

#[test]
fn test_result_limit() {
    let mut db = grep_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = grep_excel::database::SearchQuery {
        text: "a".into(),
        column: None,
        mode: grep_excel::database::SearchMode::FullText,
        limit: 2,
    };
    let (results, stats) = db.search(&query).expect("search");
    assert!(results.len() <= 2);
    assert!(stats.truncated);
}

#[test]
fn test_for_each_sheet_streaming() {
    let sheet_info =
        grep_excel::excel::for_each_sheet(Path::new("test_data.xlsx"), |_sheet_data, _idx| Ok(()))
            .expect("for_each_sheet");
    assert_eq!(sheet_info.len(), 3);
    assert_eq!(sheet_info[0].0, "Employees");
    assert_eq!(sheet_info[0].1, 7);
}
