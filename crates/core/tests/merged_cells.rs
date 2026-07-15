use grep_excel_core::excel::{for_each_sheet, for_each_sheet_repair, parse_excel, parse_file_repair};
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_xlsx_path() -> PathBuf {
    let id = TMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("grep_excel_merged_{}.xlsx", id))
}

// Fixture: Region|City with A2:A5 merged ("华北" only stored in A2).
fn write_merged_region_xlsx(path: &std::path::Path) {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>"#,
        )
        .unwrap();

        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
        )
        .unwrap();

        zip.start_file("xl/workbook.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
        )
        .unwrap();

        zip.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
</Relationships>"#,
        )
        .unwrap();

        zip.start_file("xl/sharedStrings.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="7" uniqueCount="7">
  <si><t>Region</t></si>
  <si><t>City</t></si>
  <si><t>&#x534E;&#x5317;</t></si>
  <si><t>&#x5317;&#x4EAC;</t></si>
  <si><t>&#x5929;&#x6D25;</t></si>
  <si><t>&#x77F3;&#x5BB6;&#x5E84;</t></si>
  <si><t>&#x5510;&#x5C71;</t></si>
</sst>"#,
        )
        .unwrap();

        zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B5"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1" t="s"><v>1</v></c>
    </row>
    <row r="2">
      <c r="A2" t="s"><v>2</v></c>
      <c r="B2" t="s"><v>3</v></c>
    </row>
    <row r="3">
      <c r="B3" t="s"><v>4</v></c>
    </row>
    <row r="4">
      <c r="B4" t="s"><v>5</v></c>
    </row>
    <row r="5">
      <c r="B5" t="s"><v>6</v></c>
    </row>
  </sheetData>
  <mergeCells count="1">
    <mergeCell ref="A2:A5"/>
  </mergeCells>
</worksheet>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let bytes = buf.into_inner();
    let mut f = File::create(path).unwrap();
    f.write_all(&bytes).unwrap();
}

fn assert_filled_sheet(sheet: &grep_excel_core::excel::SheetData) {
    assert_eq!(sheet.headers, vec!["Region", "City"]);
    assert_eq!(sheet.rows.len(), 4, "expected 4 data rows");
    assert_eq!(sheet.rows[0][0], "华北");
    assert_eq!(sheet.rows[1][0], "华北", "merged A3 should be filled");
    assert_eq!(sheet.rows[2][0], "华北", "merged A4 should be filled");
    assert_eq!(sheet.rows[3][0], "华北", "merged A5 should be filled");
    assert_eq!(sheet.rows[0][1], "北京");
    assert_eq!(sheet.rows[1][1], "天津");
    assert_eq!(sheet.rows[2][1], "石家庄");
    assert_eq!(sheet.rows[3][1], "唐山");
}

#[test]
fn parse_excel_forward_fills_vertical_merge() {
    let path = temp_xlsx_path();
    write_merged_region_xlsx(&path);
    let sheets = parse_excel(&path).expect("parse_excel");
    std::fs::remove_file(&path).ok();
    assert_eq!(sheets.len(), 1);
    assert_filled_sheet(&sheets[0]);
}

#[test]
fn for_each_sheet_forward_fills_vertical_merge() {
    let path = temp_xlsx_path();
    write_merged_region_xlsx(&path);
    let mut seen = Vec::new();
    for_each_sheet(&path, |sheet, _idx| {
        seen.push(sheet);
        Ok(())
    })
    .expect("for_each_sheet");
    std::fs::remove_file(&path).ok();
    assert_eq!(seen.len(), 1);
    assert_filled_sheet(&seen[0]);
}

#[test]
fn repair_path_forward_fills_vertical_merge() {
    let path = temp_xlsx_path();
    write_merged_region_xlsx(&path);
    let sheets = parse_file_repair(&path).expect("parse_file_repair");
    std::fs::remove_file(&path).ok();
    assert_eq!(sheets.len(), 1);
    assert_filled_sheet(&sheets[0]);
}

#[test]
fn for_each_sheet_repair_forward_fills() {
    let path = temp_xlsx_path();
    write_merged_region_xlsx(&path);
    let mut seen = Vec::new();
    for_each_sheet_repair(&path, |sheet, _idx| {
        seen.push(sheet);
        Ok(())
    })
    .expect("for_each_sheet_repair");
    std::fs::remove_file(&path).ok();
    assert_eq!(seen.len(), 1);
    assert_filled_sheet(&seen[0]);
}

#[test]
fn search_matches_all_merged_rows() {
    let path = temp_xlsx_path();
    write_merged_region_xlsx(&path);
    let sheets = parse_excel(&path).expect("parse_excel");
    std::fs::remove_file(&path).ok();
    let hits: Vec<_> = sheets[0]
        .rows
        .iter()
        .filter(|row| row.iter().any(|c| c.contains("华北")))
        .collect();
    assert_eq!(hits.len(), 4, "all 4 rows in merged region should match");
}
