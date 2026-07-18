use grep_excel_core::excel::parse_file;
use std::path::PathBuf;

fn write_tmp_xml(filename: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(filename);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_xml_basic_flat_table() {
    let xml = r#"<?xml version="1.0"?>
<rows>
  <row><Name>Alice</Name><Age>30</Age><City>NYC</City></row>
  <row><Name>Bob</Name><Age>25</Age><City>SF</City></row>
</rows>"#;
    let path = write_tmp_xml("gre_xml_basic.xml", xml);

    let sheets = parse_file(&path).expect("XML parse should succeed");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["Name", "Age", "City"]);
    assert_eq!(sheets[0].rows.len(), 2);
    assert_eq!(sheets[0].rows[0], vec!["Alice", "30", "NYC"]);
    assert_eq!(sheets[0].rows[1], vec!["Bob", "25", "SF"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_xml_missing_fields() {
    // Second row missing <Age> — should be empty string
    let xml = r#"<rows>
  <row><Name>Alice</Name><Age>30</Age></row>
  <row><Name>Bob</Name></row>
</rows>"#;
    let path = write_tmp_xml("gre_xml_partial.xml", xml);

    let sheets = parse_file(&path).expect("partial fields should parse");
    assert_eq!(sheets[0].rows[1], vec!["Bob", ""]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_xml_no_rows() {
    let path = write_tmp_xml("gre_xml_empty.xml", "<root></root>");
    let sheets = parse_file(&path).expect("empty XML should not error");
    assert!(sheets.is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_xml_text_only_rows() {
    // Rows with text content but no children — single column "value"
    let xml = r#"<list>
  <item>apple</item>
  <item>banana</item>
  <item>cherry</item>
</list>"#;
    let path = write_tmp_xml("gre_xml_text.xml", xml);

    let sheets = parse_file(&path).expect("text-only XML should parse");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["value"]);
    assert_eq!(sheets[0].rows.len(), 3);
    assert_eq!(sheets[0].rows[0], vec!["apple"]);
    assert_eq!(sheets[0].rows[2], vec!["cherry"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_xml_with_attributes_ignored() {
    // Attributes on row elements should be ignored (MVP behavior)
    let xml = r#"<data>
  <record id="1"><Name>Alice</Name></record>
  <record id="2"><Name>Bob</Name></record>
</data>"#;
    let path = write_tmp_xml("gre_xml_attrs.xml", xml);

    let sheets = parse_file(&path).expect("XML with attrs should parse");
    assert_eq!(sheets[0].headers, vec!["Name"]);
    assert_eq!(sheets[0].rows[0], vec!["Alice"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_xml_invalid_syntax_errors() {
    let path = write_tmp_xml("gre_xml_bad.xml", "<root><unclosed></root>");
    let result = parse_file(&path);
    assert!(result.is_err(), "malformed XML should error");
    let _ = std::fs::remove_file(&path);
}
