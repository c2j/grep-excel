use grep_excel_core::excel::parse_file;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn build_pptx(slide_xmls: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join("grep_excel_pptx_test");
    let _ = std::fs::create_dir_all(&dir);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = dir.join(format!("test_{}_{id}.pptx", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default();

    zip.start_file("[Content_Types].xml", opts).unwrap();
    zip.write_all(br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#).unwrap();

    for (i, xml) in slide_xmls.iter().enumerate() {
        let name = format!("ppt/slides/slide{}.xml", i + 1);
        zip.start_file(&name, opts).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }

    zip.finish().unwrap();
    path
}

fn wrap_slide(inner: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
{inner}
    </p:spTree>
  </p:cSld>
</p:sld>"#,
    )
}

fn pptx_row(cells: &[&str]) -> String {
    let cells_xml: String = cells
        .iter()
        .map(|t| {
            format!(
                "<a:tc><a:txBody><a:p><a:r><a:t>{t}</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc>"
            )
        })
        .collect();
    format!("<a:tr h=\"370\">{cells_xml}</a:tr>")
}

fn pptx_table(rows: &[&[&str]]) -> String {
    let rows_xml: String = rows.iter().map(|r| pptx_row(r)).collect();
    format!(
        "<p:graphicFrame><a:graphic><a:graphicData><a:tbl><a:tblPr/><a:tblGrid/>{rows_xml}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"
    )
}

#[test]
fn parses_simple_pptx_table() {
    let tbl = pptx_table(&[
        &["Name", "Age"],
        &["Alice", "30"],
        &["Bob", "25"],
    ]);
    let slide = wrap_slide(&tbl);
    let path = build_pptx(&[&slide]);

    let sheets = parse_file(&path).expect("pptx parse should succeed");
    assert_eq!(sheets.len(), 1);
    let s = &sheets[0];
    assert_eq!(s.name, "Slide_1_Table_1");
    assert_eq!(s.headers, vec!["Name", "Age"]);
    assert_eq!(s.rows.len(), 2);
    assert_eq!(s.rows[0], vec!["Alice", "30"]);
    assert_eq!(s.rows[1], vec!["Bob", "25"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn parses_pptx_multi_slide() {
    let t1 = pptx_table(&[&["A", "B"], &["a", "b"]]);
    let t2 = pptx_table(&[&["X", "Y"], &["x", "y"]]);
    let slide1 = wrap_slide(&t1);
    let slide2 = wrap_slide(&t2);
    let path = build_pptx(&[&slide1, &slide2]);

    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets[0].name, "Slide_1_Table_1");
    assert_eq!(sheets[1].name, "Slide_2_Table_1");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn pptx_skips_slide_without_tables() {
    let slide_empty = wrap_slide("");
    let slide_with = wrap_slide(&pptx_table(&[&["H"], &["d"]]));
    let path = build_pptx(&[&slide_empty, &slide_with]);

    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].name, "Slide_2_Table_1");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn pptx_gridspan_merge() {
    let xml = wrap_slide(
        r#"<p:graphicFrame><a:graphic><a:graphicData><a:tbl><a:tblPr/><a:tblGrid/>
  <a:tr><a:tc gridSpan="2"><a:txBody><a:p><a:r><a:t>Merged</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc><a:tc><a:txBody><a:p><a:r><a:t>C</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr>
  <a:tr><a:tc><a:txBody><a:p><a:r><a:t>A</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc><a:tc><a:txBody><a:p><a:r><a:t>B</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc><a:tc><a:txBody><a:p><a:r><a:t>C</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr>
</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
    );
    let path = build_pptx(&[&xml]);
    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["Merged", "Merged", "C"]);
    assert_eq!(sheets[0].rows[0], vec!["A", "B", "C"]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn pptx_rowspan_merge() {
    let xml = wrap_slide(
        r#"<p:graphicFrame><a:graphic><a:graphicData><a:tbl><a:tblPr/><a:tblGrid/>
  <a:tr><a:tc><a:txBody><a:p><a:r><a:t>Region</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc><a:tc><a:txBody><a:p><a:r><a:t>Value</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr>
  <a:tr><a:tc rowSpan="2"><a:txBody><a:p><a:r><a:t>North</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc><a:tc><a:txBody><a:p><a:r><a:t>100</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr>
  <a:tr><a:tc><a:txBody><a:p><a:r><a:t>200</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr>
</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
    );
    let path = build_pptx(&[&xml]);
    let sheets = parse_file(&path).expect("parse");
    assert_eq!(sheets.len(), 1);
    assert_eq!(sheets[0].headers, vec!["Region", "Value"]);
    assert_eq!(sheets[0].rows[0], vec!["North", "100"]);
    assert_eq!(sheets[0].rows[1], vec!["North", "200"]);
    let _ = std::fs::remove_file(&path);
}