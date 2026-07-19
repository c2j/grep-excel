use grep_excel_core::excel::parse_file_as;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(manifest)
        .join("../..")
        .join("tests/fixtures/pdf")
        .join(name)
}

#[test]
fn parse_simple_pdf() {
    let path = fixture("simple.pdf");
    if !path.exists() {
        eprintln!(
            "Skipping test: fixture '{}' not found. Place a text-based PDF with tables there.",
            path.display()
        );
        return;
    }
    let sheets = parse_file_as(&path, None).expect("should parse simple.pdf");
    assert!(
        !sheets.is_empty(),
        "should extract at least one table from '{}'",
        path.display()
    );
    for sheet in &sheets {
        assert!(
            !sheet.headers.is_empty(),
            "table '{}' should have headers",
            sheet.name
        );
        assert!(
            !sheet.rows.is_empty(),
            "table '{}' should have data rows",
            sheet.name
        );
    }
}
