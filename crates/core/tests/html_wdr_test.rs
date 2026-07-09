use grep_excel_core::html_table::extract_tables;
use std::path::Path;

const WDR_DIR: &str = "/Users/c2j/Projects/Desktop_Projects/DB/WDRProbe/example";

#[test]
fn test_opengauss_v1_tables() {
    let html = std::fs::read_to_string(Path::new(WDR_DIR).join("opengauss_v1.html"))
        .expect("read v1 file");
    let tables = extract_tables(&html).unwrap();
    assert!(!tables.is_empty(), "should extract at least one table");

    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&"This table displays report type"),
        "should find Report Type table, got: {:?}",
        names
    );

    let load_profile = tables
        .iter()
        .find(|t| t.name == "This table displays Load Profile" && t.headers.len() >= 3);
    assert!(load_profile.is_some(), "should find Load Profile table");
    let load_profile = load_profile.unwrap();
    assert_eq!(
        load_profile.headers,
        vec!["Metric", "Per Second", "Per Transaction", "Per Exec"]
    );
    assert!(!load_profile.rows.is_empty());
    assert!(
        load_profile.rows.iter().any(|r| r[0].contains("DB Time")),
        "Load Profile should contain DB Time row"
    );
}

#[test]
fn test_opengauss_v2_tables() {
    let html = std::fs::read_to_string(Path::new(WDR_DIR).join("opengauss_v2.html"))
        .expect("read v2 file");
    let tables = extract_tables(&html).unwrap();
    assert!(!tables.is_empty(), "v2 should also yield tables");
}

#[test]
fn test_sql_detail_wdr_tables() {
    let html =
        std::fs::read_to_string(Path::new(WDR_DIR).join("test_sql_detail_wdr.html"))
            .expect("read sql detail file");
    let tables = extract_tables(&html).unwrap();
    assert!(!tables.is_empty());

    let sql_table = tables.iter().find(|t| {
        t.name.contains("SQL") || t.headers.iter().any(|h| h.contains("SQL ID"))
    });
    assert!(sql_table.is_some(), "should find a SQL-related table");
    let sql_table = sql_table.unwrap();
    assert!(
        sql_table.headers.contains(&"SQL Text".to_string()),
        "SQL table should have SQL Text column"
    );
    assert!(
        sql_table
            .rows
            .iter()
            .any(|r| r.iter().any(|c| c.contains("SELECT"))),
        "SQL table should contain SELECT statements"
    );
}
