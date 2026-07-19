#![cfg(any(feature = "engine-duckdb", feature = "engine-sqlite"))]

use grep_excel_core::engine::{DefaultEngine, SearchEngine};
use grep_excel_core::excel::SheetData;
use grep_excel_core::types::TableKind;

fn make_engine_with_data() -> DefaultEngine {
    let mut db = DefaultEngine::new().unwrap();
    let sheet = SheetData {
        name: "Sheet1".to_string(),
        headers: vec!["name".to_string(), "value".to_string()],
        rows: vec![
            vec!["Alice".to_string(), "100".to_string()],
            vec!["Bob".to_string(), "200".to_string()],
            vec!["Charlie".to_string(), "300".to_string()],
        ],
        col_widths: vec![],
    };
    db.import_sheets("data.csv", vec![sheet], &|_, _| {})
        .unwrap();
    db
}

// ── Test 1: materialize_query then execute_sql ──────────────────────────────

#[test]
fn test_materialize_and_select() {
    let mut db = make_engine_with_data();
    let info = db
        .materialize_query("my_temp", "SELECT * FROM sheet_1_0", false, None)
        .unwrap();

    assert_eq!(info.name, "my_temp");
    assert_eq!(info.alias, "temp.my_temp");
    assert_eq!(info.row_count, 3);
    assert!(!info.replaced);
    assert_eq!(info.columns.len(), 2);

    let result = db
        .execute_sql("SELECT * FROM my_temp ORDER BY name", 100)
        .unwrap();
    assert_eq!(result.row_count, 3);
    assert_eq!(result.rows[0][0], "Alice");
    assert_eq!(result.rows[0][1], "100");
    assert_eq!(result.rows[1][0], "Bob");
    assert_eq!(result.rows[2][0], "Charlie");
}

// ── Test 2: replace semantics ───────────────────────────────────────────────

#[test]
fn test_replace_true_overwrites() {
    let mut db = make_engine_with_data();
    db.materialize_query("my_temp", "SELECT * FROM sheet_1_0", false, None)
        .unwrap();

    let err = db
        .materialize_query(
            "my_temp",
            "SELECT * FROM sheet_1_0 WHERE value = '100'",
            false,
            None,
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("already exists"),
        "expected 'already exists' error, got: {}",
        err
    );

    let info = db
        .materialize_query(
            "my_temp",
            "SELECT * FROM sheet_1_0 WHERE value = '100'",
            true,
            None,
        )
        .unwrap();
    assert!(info.replaced);
    assert_eq!(info.row_count, 1);

    let result = db.execute_sql("SELECT * FROM my_temp", 100).unwrap();
    assert_eq!(result.row_count, 1);
    assert_eq!(result.rows[0][0], "Alice");
}

// ── Test 3: drop_temp_table lifecycle ───────────────────────────────────────

#[test]
fn test_drop_temp_table_then_select_fails() {
    let mut db = make_engine_with_data();
    db.materialize_query("my_temp", "SELECT * FROM sheet_1_0", false, None)
        .unwrap();

    db.drop_temp_table("my_temp").unwrap();

    let err = db.execute_sql("SELECT * FROM my_temp", 100).unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("exist") || msg.contains("not found") || msg.contains("no such"),
        "expected table-not-found error, got: {}",
        err
    );

    let aliases = db.list_table_aliases();
    assert!(
        aliases.iter().all(|a| a.alias != "temp.my_temp"),
        "temp.my_temp should not appear after drop"
    );
}

// ── Test 4: drop_temp_table on import table name → error ────────────────────

#[test]
fn test_drop_import_table_name_fails() {
    let mut db = make_engine_with_data();

    let err = db.drop_temp_table("sheet_1_0").unwrap_err();
    assert!(
        err.to_string().contains("sheet_"),
        "expected sheet_ prefix error, got: {}",
        err
    );

    let result = db.execute_sql("SELECT * FROM sheet_1_0", 100).unwrap();
    assert_eq!(result.row_count, 3);
}

// ── Test 5: validate_sql still rejects CREATE TABLE via execute_sql ──────────

#[test]
fn test_execute_sql_rejects_ddl() {
    let db = make_engine_with_data();
    let err = db
        .execute_sql("CREATE TABLE foo (x TEXT)", 100)
        .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("read-only") || msg.contains("forbidden"),
        "expected DDL rejection, got: {}",
        err
    );
}

// ── Test 6: Bad names rejected ──────────────────────────────────────────────

#[test]
fn test_materialize_with_bad_names_rejected() {
    let mut db = make_engine_with_data();

    let err = db
        .materialize_query("", "SELECT 1", false, None)
        .unwrap_err();
    assert!(err.to_string().contains("empty"));

    let err = db
        .materialize_query("1abc", "SELECT 1", false, None)
        .unwrap_err();
    assert!(err.to_string().contains("letter"));

    let err = db
        .materialize_query("a-b", "SELECT 1", false, None)
        .unwrap_err();
    assert!(err.to_string().contains("letters"));

    let long = "a".repeat(65);
    let err = db
        .materialize_query(&long, "SELECT 1", false, None)
        .unwrap_err();
    assert!(err.to_string().contains("64"));

    let err = db
        .materialize_query("sheet_my_temp", "SELECT 1", false, None)
        .unwrap_err();
    assert!(err.to_string().contains("sheet_"));
}

// ── Test 7: list_table_aliases includes temp entries ────────────────────────

#[test]
fn test_list_table_aliases_includes_temps() {
    let mut db = make_engine_with_data();
    db.materialize_query("my_temp", "SELECT * FROM sheet_1_0", false, None)
        .unwrap();

    let aliases = db.list_table_aliases();
    let temp_entry = aliases.iter().find(|a| a.alias == "temp.my_temp");
    assert!(
        temp_entry.is_some(),
        "temp.my_temp should appear in list_table_aliases"
    );
    let entry = temp_entry.unwrap();
    assert_eq!(entry.kind, TableKind::Temp);
    assert_eq!(entry.row_count, 3);
    assert_eq!(entry.table_name, "my_temp");
    assert_eq!(entry.file_name, "<temp>");

    let file_count = aliases.iter().filter(|a| a.kind == TableKind::File).count();
    assert!(file_count > 0, "imported file aliases should still be present");
}

// ── Test 8: clear() removes temps ───────────────────────────────────────────

#[test]
fn test_clear_removes_temp_tables() {
    let mut db = make_engine_with_data();
    db.materialize_query("my_temp", "SELECT * FROM sheet_1_0", false, None)
        .unwrap();

    db.clear().unwrap();

    let err = db.execute_sql("SELECT * FROM my_temp", 100).unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("exist") || msg.contains("not found") || msg.contains("no such"),
        "expected table-not-found after clear, got: {}",
        err
    );

    let aliases = db.list_table_aliases();
    assert!(
        aliases.is_empty(),
        "list_table_aliases should be empty after clear"
    );
}

// ── Test 9: max_rows truncation ─────────────────────────────────────────────

#[test]
fn test_max_rows_truncates_materialization() {
    let mut db = make_engine_with_data();
    let info = db
        .materialize_query("my_temp", "SELECT * FROM sheet_1_0", false, Some(1))
        .unwrap();
    assert_eq!(
        info.row_count, 1,
        "max_rows: Some(1) should limit to 1 row"
    );
    assert_eq!(info.columns.len(), 2);

    let result = db.execute_sql("SELECT * FROM my_temp", 100).unwrap();
    assert_eq!(result.row_count, 1);
}

// ── Test 10: Forbidden SQL in source is rejected before any DDL ─────────────

#[test]
fn test_forbidden_sql_in_materialize_source_rejected() {
    let mut db = make_engine_with_data();

    let err = db
        .materialize_query("t", "INSERT INTO sheet_1_0 VALUES ('x','y')", false, None)
        .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("read-only") || msg.contains("forbidden"),
        "expected INSERT rejection, got: {}",
        err
    );
    let err2 = db.execute_sql("SELECT * FROM t", 100);
    assert!(err2.is_err(), "table 't' should not exist after rejection");

    let err = db
        .materialize_query("t2", "UPDATE sheet_1_0 SET name='x'", false, None)
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("read-only")
            || err.to_string().to_lowercase().contains("forbidden")
    );

    let err = db
        .materialize_query("t3", "DELETE FROM sheet_1_0", false, None)
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("read-only")
            || err.to_string().to_lowercase().contains("forbidden")
    );
}

// ── Bugfix: casing mismatch between create and drop ─────────────────────────

#[test]
fn test_drop_with_different_casing() {
    let mut db = make_engine_with_data();
    db.materialize_query("MyTemp", "SELECT * FROM sheet_1_0", false, None)
        .unwrap();

    db.drop_temp_table("mytemp").unwrap();

    let err = db.execute_sql("SELECT * FROM \"MyTemp\"", 100).unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("exist") || msg.contains("not found") || msg.contains("no such"),
        "expected table-not-found after casing-mismatched drop, got: {}",
        err
    );
}

#[test]
fn test_replace_with_different_casing() {
    let mut db = make_engine_with_data();
    db.materialize_query("MyTemp", "SELECT * FROM sheet_1_0", false, None)
        .unwrap();

    let info = db
        .materialize_query(
            "mytemp",
            "SELECT * FROM sheet_1_0 WHERE value = '100'",
            true,
            None,
        )
        .unwrap();
    assert!(
        info.replaced,
        "should report replaced: true after overwrite"
    );
    assert_eq!(info.row_count, 1);
    assert_eq!(info.name, "mytemp");

    let result = db.execute_sql("SELECT * FROM mytemp", 100).unwrap();
    assert_eq!(result.row_count, 1);
    assert_eq!(result.rows[0][0], "Alice");
}
