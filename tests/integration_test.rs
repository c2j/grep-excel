use std::path::Path;

#[test]
fn test_parse_excel() {
    let sheets = search_excel::excel::parse_excel(Path::new("test_data.xlsx"))
        .expect("parse_excel should succeed");
    assert_eq!(sheets.len(), 3);
    assert_eq!(sheets[0].name, "Employees");
    assert_eq!(sheets[0].rows.len(), 7);
    assert_eq!(sheets[1].name, "Products");
    assert_eq!(sheets[2].name, "Orders");
}

#[test]
fn test_database_search() {
    let mut db = search_excel::database::Database::new().expect("db new");
    let info = db
        .import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");
    assert_eq!(info.sheets.len(), 3);
    assert_eq!(info.total_rows, 15);

    let query = search_excel::database::SearchQuery {
        text: "Engineering".into(),
        column: None,
        mode: search_excel::database::SearchMode::FullText,
    };
    let (results, stats) = db.search(&query).expect("search");
    assert_eq!(results.len(), 4);
    assert_eq!(stats.total_matches, 4);
}

#[test]
fn test_exact_match() {
    let mut db = search_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = search_excel::database::SearchQuery {
        text: "Engineering".into(),
        column: Some("Department".into()),
        mode: search_excel::database::SearchMode::ExactMatch,
    };
    let (results, _) = db.search(&query).expect("search");
    assert_eq!(results.len(), 4);
}

#[test]
fn test_wildcard_search() {
    let mut db = search_excel::database::Database::new().expect("db new");
    db.import_excel(Path::new("test_data.xlsx"), |_, _| {})
        .expect("import");

    let query = search_excel::database::SearchQuery {
        text: "San%".into(),
        column: Some("City".into()),
        mode: search_excel::database::SearchMode::Wildcard,
    };
    let (results, _) = db.search(&query).expect("search");
    assert_eq!(results.len(), 4);
}
