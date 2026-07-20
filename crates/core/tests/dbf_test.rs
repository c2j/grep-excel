use grep_excel_core::excel::parse_file;
use std::convert::TryFrom;

/// Roundtrip test: write a DBF file using dbase writer, then read it back via parse_file.
#[test]
fn test_dbf_parse_roundtrip() {
    let dir = std::env::temp_dir().join("grep_excel_dbf_test");
    let _ = std::fs::create_dir_all(&dir);
    let dbf_path = dir.join("test.dbf");

    // Clean up any previous test file
    let _ = std::fs::remove_file(&dbf_path);

    // Write a DBF file with two fields
    {
        let mut writer = dbase::TableWriterBuilder::new()
            .add_character_field(dbase::FieldName::try_from("Name").unwrap(), 50)
            .add_numeric_field(dbase::FieldName::try_from("Age").unwrap(), 20, 10)
            .build_with_file_dest(&dbf_path)
            .expect("should create DBF writer");

        let mut record1 = dbase::Record::default();
        record1.insert(
            "Name".to_string(),
            dbase::FieldValue::Character(Some("Alice".to_string())),
        );
        record1.insert("Age".to_string(), dbase::FieldValue::Numeric(Some(30.0)));
        writer
            .write_record(&record1)
            .expect("should write record 1");

        let mut record2 = dbase::Record::default();
        record2.insert(
            "Name".to_string(),
            dbase::FieldValue::Character(Some("Bob".to_string())),
        );
        record2.insert("Age".to_string(), dbase::FieldValue::Numeric(Some(25.5)));
        writer
            .write_record(&record2)
            .expect("should write record 2");

        // Drop writer to flush and finalize the file
        drop(writer);
    }

    // Read it back using parse_file
    let sheets = parse_file(&dbf_path).expect("DBF parse should succeed");
    assert_eq!(sheets.len(), 1, "should have one sheet");
    assert_eq!(sheets[0].name, "test", "sheet name should be file stem");
    // Headers come from reader.fields() in file (insertion) order
    assert_eq!(
        sheets[0].headers,
        vec!["Name", "Age"],
        "should have correct headers (file order)"
    );
    assert_eq!(sheets[0].rows.len(), 2, "should have 2 data rows");

    // Verify data
    assert_eq!(
        sheets[0].rows[0],
        vec!["Alice", "30"],
        "first row should have correct values"
    );
    assert_eq!(
        sheets[0].rows[1],
        vec!["Bob", "25.5"],
        "second row should have correct values"
    );

    // Clean up
    let _ = std::fs::remove_file(&dbf_path);
}
