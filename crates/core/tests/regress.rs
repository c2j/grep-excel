//! Regression tests for HTML table extraction (`html_table::extract_tables`).
//!
//! These tests exercise edge cases that real-world HTML files often present,
//! ensuring the extractor remains robust across variations in structure,
//! formatting, and malformed markup.
//!
//! Test fixtures are documented in `tests/regress/README.md` at workspace root.

use grep_excel_core::html_table::extract_tables;

// ── 1. HTML Fragment (bare `<table>` without `<html>/<body>`) ────────────────

#[test]
fn regress_html_fragment() {
    let html = r#"<table>
        <tr><th>Key</th><th>Val</th></tr>
        <tr><td>A</td><td>1</td></tr>
        <tr><td>B</td><td>2</td></tr>
    </table>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1, "should extract from bare table fragment");
    assert_eq!(tables[0].headers, ["Key", "Val"]);
    assert_eq!(tables[0].rows.len(), 2);
}

#[test]
fn regress_html_fragment_no_root() {
    // Multiple tables with no <html>, <body>, or any wrapper
    let html = r#"<table><tr><th>X</th></tr><tr><td>1</td></tr></table>
        <hr/>
        <table><tr><th>Y</th></tr><tr><td>2</td></tr></table>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 2, "multiple bare tables should both be found");
    assert_eq!(tables[0].name, "Table_1");
    assert_eq!(tables[1].name, "Table_2");
}

// ── 2. Semantic Sections (`<thead>`, `<tbody>`, `<tfoot>`) ──────────────────

#[test]
fn regress_thead_tbody_tfoot() {
    let html = r#"<html><body>
        <table summary="Sales">
            <thead>
                <tr><th>Product</th><th>Qty</th></tr>
            </thead>
            <tbody>
                <tr><td>Widget</td><td>10</td></tr>
                <tr><td>Gadget</td><td>20</td></tr>
            </tbody>
            <tfoot>
                <tr><td>Total</td><td>30</td></tr>
            </tfoot>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Product", "Qty"]);
    // <tfoot> row should also be extracted as a data row
    assert!(
        tables[0].rows.iter().any(|r| r[0] == "Total"),
        "tfoot row should be present: {:?}",
        tables[0].rows
    );
}

#[test]
fn regress_multiple_tbody() {
    // Tables with multiple <tbody> sections
    let html = r#"<html><body>
        <table>
            <thead><tr><th>ID</th><th>Name</th></tr></thead>
            <tbody><tr><td>1</td><td>Alice</td></tr></tbody>
            <tbody><tr><td>2</td><td>Bob</td></tr></tbody>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows.len(), 2, "both tbody rows merged");
}

// ── 3. `colspan` in headers and data ────────────────────────────────────────

#[test]
fn regress_colspan_header() {
    // scraper's text() concatenates cell content but colspan isn't reflected;
    // this test verifies we don't crash on colspan and still extract correctly
    let html = r#"<html><body>
        <table summary="colspan table">
            <tr>
                <th colspan="2">Full Name</th>
                <th>Age</th>
            </tr>
            <tr>
                <td>John</td><td>Doe</td><td>30</td>
            </tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper doesn't expand colspan; it treats <th colspan="2"> as a single cell
    assert_eq!(
        tables[0].headers.len(),
        2,
        "colspan header: 2 headers expected (colspan collapsed), got {:?}",
        tables[0].headers
    );
    assert_eq!(tables[0].rows[0], ["John", "Doe", "30"]);
}

#[test]
fn regress_colspan_data_cell() {
    let html = r#"<html><body>
        <table>
            <tr><th>A</th><th>B</th><th>C</th></tr>
            <tr><td colspan="2">merged</td><td>single</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows.len(), 1);
    // colspan cell yields a single text, so we get 2 data cells vs 3 headers
    assert_eq!(
        tables[0].rows[0].len(),
        2,
        "colspan data row should reflect actual <td> count"
    );
}

// ── 4. `rowspan` in data ───────────────────────────────────────────────────

#[test]
fn regress_rowspan() {
    let html = r#"<html><body>
        <table summary="rowspan test">
            <tr><th>Category</th><th>Item</th></tr>
            <tr><td rowspan="2">Fruit</td><td>Apple</td></tr>
            <tr><td>Banana</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // rowspan is not visually expanded; each <td> is independent
    assert_eq!(tables[0].rows.len(), 2);
    // rowspan is not visually expanded; each <td> is independent in its own <tr>
    // row 0: <td rowspan="2">Fruit</td><td>Apple</td> → 2 cells
    // row 1: <td>Banana</td> → 1 cell
    assert_eq!(tables[0].rows[0].len(), 2, "row 0: 2 tds (Fruit + Apple)");
    assert_eq!(tables[0].rows[1].len(), 1, "row 1: 1 td (Banana)");
    assert_eq!(tables[0].rows[0][0], "Fruit");
    assert_eq!(tables[0].rows[0][1], "Apple");
    assert_eq!(tables[0].rows[1][0], "Banana");
}

// ── 5. Nested Tables ────────────────────────────────────────────────────────

#[test]
fn regress_nested_table() {
    let html = r#"<html><body>
        <table summary="Outer">
            <tr><th>Name</th><th>Details</th></tr>
            <tr>
                <td>OuterItem</td>
                <td>
                    <table summary="Inner">
                        <tr><th>InnerKey</th><th>InnerVal</th></tr>
                        <tr><td>X</td><td>42</td></tr>
                    </table>
                </td>
            </tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    // Both tables should be extracted; the inner one is a descendant of the outer <td>
    assert_eq!(tables.len(), 2, "nested tables should both be extracted");
    let outer = tables
        .iter()
        .find(|t| t.name == "Outer")
        .expect("outer table");
    let inner = tables
        .iter()
        .find(|t| t.name == "Inner")
        .expect("inner table");
    assert_eq!(outer.headers, ["Name", "Details"]);
    assert_eq!(inner.headers, ["InnerKey", "InnerVal"]);
    assert_eq!(inner.rows[0], ["X", "42"]);
}

#[test]
fn regress_deeply_nested() {
    // 3 levels of nesting: table > td > table > td > table
    let html = r#"<html><body>
        <table summary="L1">
            <tr><th>L1_H</th></tr>
            <tr><td>
                <table summary="L2">
                    <tr><th>L2_H</th></tr>
                    <tr><td>
                        <table summary="L3">
                            <tr><th>L3_H</th></tr>
                            <tr><td>deep</td></tr>
                        </table>
                    </td></tr>
                </table>
            </td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 3, "all 3 nesting levels should be extracted");
    assert!(tables.iter().any(|t| t.name == "L1"));
    assert!(tables.iter().any(|t| t.name == "L2"));
    assert!(tables.iter().any(|t| t.name == "L3"));
    assert_eq!(
        tables.iter().find(|t| t.name == "L3").unwrap().rows[0][0],
        "deep"
    );
}

// ── 6. `<caption>` as Table Name ────────────────────────────────────────────

#[test]
fn regress_caption_as_name() {
    // Note: current implementation uses summary attr -> h3 text -> auto.
    // <caption> is not yet used as a name source. This test documents that
    // gap and ensures we don't crash on <caption>.
    let html = r#"<html><body>
        <table>
            <caption>My Caption</caption>
            <tr><th>H1</th><th>H2</th></tr>
            <tr><td>A</td><td>B</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // Name currently falls back to auto-generated because no summary, no h3
    assert_eq!(
        tables[0].name, "Table_1",
        "caption not yet used as name source; falls back to auto"
    );
    assert_eq!(tables[0].headers, ["H1", "H2"]);
    assert_eq!(tables[0].rows[0], ["A", "B"]);
}

// ── 7. Empty / Whitespace / &nbsp; Cells ─────────────────────────────────────

#[test]
fn regress_empty_cells() {
    let html = r#"<html><body>
        <table>
            <tr><th>Col1</th><th>Col2</th><th>Col3</th></tr>
            <tr><td></td><td> </td><td>val</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows[0].len(), 3, "empty cells should be present");
    assert_eq!(tables[0].rows[0][0], "", "empty td -> empty string");
    // scraper's text() + .trim() reduces whitespace-only cells to empty
    assert_eq!(
        tables[0].rows[0][1], "",
        "whitespace-only td -> trimmed to empty"
    );
    assert_eq!(tables[0].rows[0][2], "val");
}

#[test]
fn regress_nbsp_cells() {
    let html = r#"<html><body>
        <table>
            <tr><th>Item</th><th>Note</th></tr>
            <tr><td>Alpha</td><td>&nbsp;</td></tr>
            <tr><td>Beta</td><td>&nbsp;&nbsp;</td></tr>
            <tr><td>Gamma</td><td></td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows.len(), 3);
    // scraper trims &nbsp; (NBSP is whitespace in Rust), so cell is empty
    assert_eq!(tables[0].rows[0][1], "", "&nbsp; -> trimmed to empty");
    assert_eq!(tables[0].rows[2][1], "", "empty td -> empty string");
}

#[test]
fn regress_all_empty_row() {
    let html = r#"<html><body>
        <table>
            <tr><th>H</th></tr>
            <tr><td></td></tr>
            <tr><td></td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0].rows.len(),
        2,
        "empty data rows should still be extracted"
    );
    assert_eq!(tables[0].rows[0][0], "");
}

// ── 8. `<br>` Inside Cells ─────────────────────────────────────────────────

#[test]
fn regress_br_in_cell() {
    let html = r#"<html><body>
        <table summary="br test">
            <tr><th>Address</th></tr>
            <tr><td>123 Main St.<br/>Apt 4B<br />City, State</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper's text() concatenates text nodes; <br> doesn't produce text
    let cell = &tables[0].rows[0][0];
    assert!(
        cell.contains("123 Main St."),
        "cell should contain address: '{}'",
        cell
    );
    assert!(
        cell.contains("City, State"),
        "cell should contain city: '{}'",
        cell
    );
}

// ── 9. `<a>` Links Inside Cells ──────────────────────────────────────────────

#[test]
fn regress_links_in_cells() {
    let html = r#"<html><body>
        <table>
            <tr><th>Name</th><th>URL</th></tr>
            <tr><td><a href="https://example.com">Example</a></td><td>https://example.com</td></tr>
            <tr><td><a href="/relative">Relative Link</a></td><td>/relative</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0].rows[0][0], "Example",
        "link text should be extracted"
    );
    assert_eq!(tables[0].rows[1][0], "Relative Link");
}

// ── 10. Embedded CSS / JavaScript ──────────────────────────────────────────

#[test]
fn regress_embedded_css_js() {
    let html = r#"<!DOCTYPE html>
    <html><head>
        <style>table { border: 1px solid; } td { color: red; }</style>
        <script>document.getElementById('x').innerHTML = 'evil';</script>
    </head><body>
        <table summary="clean">
            <tr><th>Data</th></tr>
            <tr><td>real content</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // CSS/JS text should NOT leak into table content
    assert!(
        !tables[0].headers.iter().any(|h| h.contains("border")),
        "CSS should not leak into headers: {:?}",
        tables[0].headers
    );
    assert!(
        !tables[0].rows[0][0].contains("getElementById"),
        "JS should not leak into rows"
    );
    assert_eq!(tables[0].rows[0][0], "real content");
}

// ── 11. `<th>` Rows Mid-Table (Section Headers) ────────────────────────────

#[test]
fn regress_mid_table_th_row() {
    let html = r#"<html><body>
        <table summary="Sectioned">
            <tr><th>Name</th><th>Value</th></tr>
            <tr><td>Item1</td><td>100</td></tr>
            <tr><th colspan="2">Sub Section</th></tr>
            <tr><td>Item2</td><td>200</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // The second <th> row should be treated as data since headers already seen
    // It has 1 th cell (colspan=2) -> goes into th_cells, falls through to
    // `th_cells.len() == 1` branch -> row = [th_cell] + td_cells (empty) = [th_cell]
    // The mid-table <th> row has th_cells=["Sub Section"] but no td_cells,
    // so `!cells.is_empty()` is false → row is skipped
    assert_eq!(
        tables[0].rows.len(),
        2,
        "mid-table th row is skipped (no td cells)"
    );
    assert_eq!(tables[0].rows[0][0], "Item1");
    assert_eq!(
        tables[0].rows[1][0], "Item2",
        "sub-section marker row dropped, Item2 is second row"
    );
}

// ── 12. No `<th>` at All (Infer Header from First Row) ─────────────────────

#[test]
fn regress_no_th_infer_header() {
    let html = r#"<html><body>
        <table>
            <tr><td>Name</td><td>Age</td><td>City</td></tr>
            <tr><td>Alice</td><td>30</td><td>NYC</td></tr>
            <tr><td>Bob</td><td>25</td><td>SF</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // First row becomes header (the `if headers.is_empty() && !rows.is_empty()` branch)
    assert_eq!(tables[0].headers, ["Name", "Age", "City"]);
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].rows[0][0], "Alice");
}

#[test]
fn regress_no_th_two_rows() {
    let html = r#"<table>
        <tr><td>A</td><td>B</td></tr>
        <tr><td>C</td><td>D</td></tr>
    </table>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // First data row promoted to header
    assert_eq!(tables[0].headers, ["A", "B"]);
    assert_eq!(tables[0].rows[0], ["C", "D"]);
}

#[test]
fn regress_no_th_single_row() {
    // Single data row means header is the row, no data rows
    let html = r#"<table>
        <tr><td>Only</td><td>Row</td></tr>
    </table>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Only", "Row"]);
    assert!(
        tables[0].rows.is_empty(),
        "single row: header promoted, no data rows left"
    );
}

// ── 13. Jagged Rows (Varying Column Counts) ─────────────────────────────────

#[test]
fn regress_jagged_rows() {
    let html = r#"<html><body>
        <table summary="jagged">
            <tr><th>Col1</th><th>Col2</th><th>Col3</th></tr>
            <tr><td>a</td><td>b</td></tr>
            <tr><td>c</td><td>d</td><td>e</td><td>f</td></tr>
            <tr><td>g</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Col1", "Col2", "Col3"]);
    // Each row's length = number of <td> in that row (jagged preserved)
    assert_eq!(tables[0].rows[0].len(), 2, "row 0: 2 cells");
    assert_eq!(tables[0].rows[1].len(), 4, "row 1: 4 cells");
    assert_eq!(tables[0].rows[2].len(), 1, "row 2: 1 cell");
}

// ── 14. Malformed: Missing `</table>` ───────────────────────────────────────

#[test]
fn regress_missing_table_close() {
    let html = r#"<html><body>
        <table summary="oops">
            <tr><th>H</th></tr>
            <tr><td>X</td></tr>
        </div>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    // scraper is forgiving — missing </table> still parses
    assert_eq!(tables.len(), 1, "missing </table> should still extract");
    assert_eq!(tables[0].rows[0][0], "X");
}

#[test]
fn regress_unclosed_tr() {
    let html = r#"<html><body>
        <table summary="unclosed">
            <tr><th>A</th><th>B</th>
            <tr><td>1</td><td>2</td>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1, "unclosed <tr> should still extract");
    assert_eq!(tables[0].headers, ["A", "B"]);
    assert_eq!(tables[0].rows[0], ["1", "2"]);
}

#[test]
fn regress_malformed_attr_quotes() {
    // Missing closing quotes on attributes
    let html = r#"<html><body>
        <table summary=broken>
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1, "malformed attr should still parse");
    // summary=broken might not be parsed; name falls to auto
    assert_eq!(tables[0].rows[0][0], "data");
}

// ── 15. Multiple Tables with No Names ───────────────────────────────────────

#[test]
fn regress_multi_auto_named() {
    let html = r#"<html><body>
        <table><tr><th>A</th></tr><tr><td>1</td></tr></table>
        <p>separator</p>
        <table><tr><th>B</th></tr><tr><td>2</td></tr></table>
        <table><tr><th>C</th></tr><tr><td>3</td></tr></table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 3);
    assert_eq!(tables[0].name, "Table_1");
    assert_eq!(tables[1].name, "Table_2");
    assert_eq!(tables[2].name, "Table_3");
}

// ── 16. Name Source Prioritization ──────────────────────────────────────────

#[test]
fn regress_h3_with_summary() {
    // summary attr should take precedence over h3
    let html = r#"<html><body>
        <h3>Ignore Me</h3>
        <table summary="Use Me">
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables[0].name, "Use Me", "summary should win over h3");
}

#[test]
fn regress_h3_fallback() {
    // No summary -> use h3
    let html = r#"<html><body>
        <h3>Section Title</h3>
        <table>
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables[0].name, "Section Title");
}

#[test]
fn regress_h3_empty_skipped() {
    // Empty h3 should not set current_heading
    let html = r#"<html><body>
        <h3>   </h3>
        <table>
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(
        tables[0].name, "Table_1",
        "empty h3 shouldn't set heading, got: '{}'",
        tables[0].name
    );
}

#[test]
fn regress_h3_not_immediately_preceding() {
    // h3 that precedes another element before table should still be the most recent heading
    let html = r#"<html><body>
        <h3>Target</h3>
        <p>some paragraph between h3 and table</p>
        <table>
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(
        tables[0].name, "Target",
        "h3 before intermediary elements should still apply"
    );
}

// ── 17. Mixed `<th>` and `<td>` in Same Row (WDR Style) ───────────────────

#[test]
fn regress_mixed_th_td_row() {
    let html = r#"<html><body>
        <table summary="Mixed">
            <tr><th>Metric</th><th>Value</th><th>Unit</th></tr>
            <tr><th class="wdrnc">DB Time</th><td>5709</td><td>us</td></tr>
            <tr><td>CPU</td><td>23</td><td>%</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Metric", "Value", "Unit"]);
    // Row with th+td: th_cells = ["DB Time"], td_cells = ["5709", "us"]
    // th_cells.len() == 1 -> merged row = ["DB Time", "5709", "us"]
    assert_eq!(
        tables[0].rows[0],
        ["DB Time", "5709", "us"],
        "mixed th/td row should combine: got {:?}",
        tables[0].rows[0]
    );
    // Normal td-only row
    assert_eq!(tables[0].rows[1], ["CPU", "23", "%"]);
}

// ── 18. HTML Comments Inside Table ──────────────────────────────────────────

#[test]
fn regress_comments_in_table() {
    let html = r#"<html><body>
        <table summary="comments">
            <!-- header row -->
            <tr><th>Name</th><th>Score</th></tr>
            <!-- data rows -->
            <tr><td>Alice</td><td>95</td></tr>
            <!-- <tr><td>Bob</td><td>87</td></tr> (commented out) -->
            <tr><td>Charlie</td><td>92</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // Commented-out tr should NOT appear
    assert_eq!(
        tables[0].rows.len(),
        2,
        "commented-out row should not appear"
    );
    assert_eq!(tables[0].rows[0][0], "Alice");
    assert_eq!(tables[0].rows[1][0], "Charlie");
}

// ── 19. `<img>` Inside Cells ────────────────────────────────────────────────

#[test]
fn regress_img_in_cell() {
    let html = r#"<html><body>
        <table summary="images">
            <tr><th>Icon</th><th>Label</th></tr>
            <tr><td><img src="check.png" alt="✓"/></td><td>Done</td></tr>
            <tr><td><img src="cross.png" alt="✗"/></td><td>Failed</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // <img> elements don't produce text children, so cell is empty
    assert_eq!(
        tables[0].rows[0][0], "",
        "img alt text not extracted (img is self-closing)"
    );
    assert_eq!(tables[0].rows[0][1], "Done");
}

// ── 20. UTF-8 BOM ───────────────────────────────────────────────────────────

#[test]
fn regress_utf8_bom() {
    // Prepend UTF-8 BOM bytes before the HTML string
    let bom = "\u{feff}";
    let html = format!(
        r#"{}<html><body>
        <table summary="BOM Test">
            <tr><th>Col</th></tr>
            <tr><td>value</td></tr>
        </table>
    </body></html>"#,
        bom
    );
    let tables = extract_tables(&html).unwrap();
    assert_eq!(tables.len(), 1, "BOM prefix should not break parsing");
    assert_eq!(tables[0].name, "BOM Test");
    assert_eq!(tables[0].rows[0][0], "value");
}

// ── 21. Chinese / CJK Content ───────────────────────────────────────────────

#[test]
fn regress_chinese_content() {
    let html = r#"<html><body>
        <table summary="中文表">
            <tr><th>姓名</th><th>年龄</th><th>城市</th></tr>
            <tr><td>张三</td><td>28</td><td>北京</td></tr>
            <tr><td>李四</td><td>35</td><td>上海</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "中文表");
    assert_eq!(tables[0].headers, ["姓名", "年龄", "城市"]);
    assert_eq!(tables[0].rows[0], ["张三", "28", "北京"]);
    assert_eq!(tables[0].rows[1], ["李四", "35", "上海"]);
}

#[test]
fn regress_chinese_no_th() {
    // Chinese HTML table without <th> tags (common in some HTML exports)
    let html = r#"<html><body>
        <table>
            <tr><td>商品</td><td>价格</td></tr>
            <tr><td>苹果</td><td>5.00</td></tr>
            <tr><td>香蕉</td><td>3.50</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0].headers,
        ["商品", "价格"],
        "No-th Chinese table: first row promoted to headers"
    );
    assert_eq!(tables[0].rows[0], ["苹果", "5.00"]);
}

// ── 22. Mixed `<h2>` / `<h4>` (not just `<h3>`) ──────────────────────────

#[test]
fn regress_h2_not_tracked() {
    // Only <h3> is tracked; <h2> should NOT set current_heading
    let html = r#"<html><body>
        <h2>Section H2</h2>
        <table summary="">
            <tr><th>X</th></tr>
            <tr><td>1</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(
        tables[0].name, "Table_1",
        "h2 should not be used as table name, got: '{}'",
        tables[0].name
    );
}

#[test]
fn regress_h4_not_tracked() {
    let html = r#"<html><body>
        <h4>Sub Section</h4>
        <table>
            <tr><th>X</th></tr>
            <tr><td>1</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(
        tables[0].name, "Table_1",
        "h4 should not be used as table name, got: '{}'",
        tables[0].name
    );
}

// ── 23. Attributes with Special Characters ──────────────────────────────────

#[test]
fn regress_summary_with_special_chars() {
    let html = r#"<html><body>
        <table summary="Load Profile (Per Second &amp; Per Transaction)">
            <tr><th>M</th><th>V</th></tr>
            <tr><td>A</td><td>1</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper decodes HTML entities in attributes
    assert_eq!(
        tables[0].name, "Load Profile (Per Second & Per Transaction)",
        "HTML entities in summary should be decoded"
    );
}

// ── 24. `<th>` in First Row Only, Data Rows Have Mixed Content ─────────────

#[test]
fn regress_first_row_th_only() {
    let html = r#"<html><body>
        <table summary="First Row Th">
            <tr><th>ID</th><th>Label</th></tr>
            <tr><td>1</td><td>First</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["ID", "Label"]);
    assert_eq!(tables[0].rows.len(), 1);
}

// ── 25. Table with Only Headers, No Data Rows ──────────────────────────────

#[test]
fn regress_headers_only() {
    let html = r#"<html><body>
        <table summary="Empty">
            <tr><th>ColA</th><th>ColB</th></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["ColA", "ColB"]);
    assert!(
        tables[0].rows.is_empty(),
        "headers-only table should have no rows"
    );
}

// ── 26. `<tr>` with No `<td>` or `<th>` (Empty Row) ────────────────────────

#[test]
fn regress_empty_tr_tags() {
    let html = r#"<html><body>
        <table summary="empty tr">
            <tr><th>H</th></tr>
            <tr></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // <tr></tr> has no td/th, so it's skipped entirely
    assert_eq!(tables[0].rows.len(), 1, "empty <tr> should be skipped");
    assert_eq!(tables[0].rows[0][0], "data");
}

// ── 27. Self-Closing Tags Inside Table HTML5 Style ─────────────────────────

#[test]
fn regress_self_closing_tags() {
    let html = r#"<html><body>
        <table summary="self-close">
            <tr>
                <th>Item<br/></th>
                <th>Count<hr/></th>
            </tr>
            <tr>
                <td>Foo<br/></td>
                <td>42<br/></td>
            </tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Item", "Count"]);
    assert_eq!(tables[0].rows[0], ["Foo", "42"]);
}

// ── 28. Multiple `<h3>` with Only Some Preceding Tables ────────────────────

#[test]
fn regress_h3_with_gap() {
    let html = r#"<html><body>
        <h3>Section One</h3>
        <table summary="T1"><tr><th>H</th></tr><tr><td>1</td></tr></table>
        <h3>Section Two</h3>
        <!-- no table after h3 Section Two -->
        <h3>Section Three</h3>
        <table><tr><th>H</th></tr><tr><td>3</td></tr></table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].name, "T1", "summary takes precedence");
    assert_eq!(
        tables[1].name, "Section Three",
        "table without summary should pick up preceding h3"
    );
}

// ── 29. `scraper` Error Tolerance: HTML with Extra Attributes on Table ─────

#[test]
fn regress_table_extra_attrs() {
    let html = r#"<html><body>
        <table summary="Extra" data-id="123" class="results" style="width:100%">
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "Extra");
    assert_eq!(tables[0].rows[0][0], "data");
}

// ── 30. `<td>` with Nested Inline Elements ──────────────────────────────────

#[test]
fn regress_nested_inline_in_td() {
    let html = r#"<html><body>
        <table summary="inline">
            <tr><th>Description</th></tr>
            <tr><td>This is <b>bold</b> and <i>italic</i> text</td></tr>
            <tr><td><span class="highlight">Highlighted</span> cell</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper's text() concatenates all descendant text nodes
    assert_eq!(tables[0].rows[0][0], "This is bold and italic text");
    assert_eq!(tables[0].rows[1][0], "Highlighted cell");
}

// ── 31. `<table>` Without Any `<tr>` ───────────────────────────────────────

#[test]
fn regress_table_no_tr() {
    let html = r#"<html><body>
        <table summary="NoRows">
            <th>Orphan</th>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scaper still finds <th> as descendant of <table>, no <tr> needed
    assert_eq!(
        tables[0].headers,
        ["Orphan"],
        "<th> found even without <tr>"
    );
    assert!(tables[0].rows.is_empty(), "no tr -> no data rows");
}

// ── 32. HTML Entities in Cell Text ──────────────────────────────────────────

#[test]
fn regress_html_entities_in_cells() {
    let html = r#"<html><body>
        <table summary="entities">
            <tr><th>Symbol</th><th>Name</th></tr>
            <tr><td>&amp;</td><td>Ampersand</td></tr>
            <tr><td>&lt;</td><td>Less Than</td></tr>
            <tr><td>&gt;</td><td>Greater Than</td></tr>
            <tr><td>&quot;</td><td>Double Quote</td></tr>
            <tr><td>&#39;</td><td>Single Quote</td></tr>
            <tr><td>&copy;</td><td>Copyright</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows[0][0], "&", "&amp; -> &");
    assert_eq!(tables[0].rows[1][0], "<", "&lt; -> <");
    assert_eq!(tables[0].rows[2][0], ">", "&gt; -> >");
    assert_eq!(tables[0].rows[3][0], "\"", "&quot; -> \"");
    assert_eq!(tables[0].rows[4][0], "'", "&#39; -> '");
    assert_eq!(tables[0].rows[5][0], "\u{a9}", "&copy; -> ©");
}

// ── 33. `summary` Attribute that is Empty String ────────────────────────────

#[test]
fn regress_empty_summary_attr() {
    // summary="" should be treated as no summary, falling back to h3 or auto
    let html = r#"<html><body>
        <h3>Fallback Name</h3>
        <table summary="">
            <tr><th>H</th></tr>
            <tr><td>d</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(
        tables[0].name, "Fallback Name",
        "empty summary attr should fallback to h3"
    );
}

#[test]
fn regress_summary_whitespace_only() {
    let html = r#"<html><body>
        <h3>Real Name</h3>
        <table summary="   ">
            <tr><th>H</th></tr>
            <tr><td>d</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    // scraper preserves whitespace attr value; the code only filters empty strings
    assert_eq!(
        tables[0].name, "   ",
        "whitespace-only summary is NOT filtered (only empty string is)"
    );
}

// ── 34. Real-World WDR: Table with `class` and Complex Structure ───────────

#[test]
fn regress_wdr_complex_table() {
    // Simulating a WDR "SQL Statistics" table with multiple column groups
    let html = r#"<html><body>
        <h3 id="sqlstat">SQL Statistics</h3>
        <table class="table table-bordered" summary="This table displays SQL Statistics">
            <thead>
                <tr>
                    <th>SQL ID</th>
                    <th>SQL Text</th>
                    <th>Elapsed(us)</th>
                    <th>CPU(us)</th>
                </tr>
            </thead>
            <tbody>
                <tr>
                    <td>abc123</td>
                    <td>SELECT * FROM users WHERE id = ?</td>
                    <td>1500</td>
                    <td>1200</td>
                </tr>
                <tr>
                    <td>def456</td>
                    <td>INSERT INTO logs VALUES (?, ?)</td>
                    <td>500</td>
                    <td>400</td>
                </tr>
            </tbody>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "This table displays SQL Statistics");
    assert_eq!(
        tables[0].headers,
        ["SQL ID", "SQL Text", "Elapsed(us)", "CPU(us)"]
    );
    assert_eq!(tables[0].rows.len(), 2);
    assert!(tables[0].rows[0][1].contains("SELECT * FROM users"));
}

// ── 35. Input Validation Edge Cases ─────────────────────────────────────────

#[test]
fn regress_empty_input() {
    let tables = extract_tables("").unwrap();
    assert!(tables.is_empty(), "empty input -> no tables");
}

#[test]
fn regress_whitespace_only_input() {
    let tables = extract_tables("   \n  \t  ").unwrap();
    assert!(tables.is_empty(), "whitespace-only -> no tables");
}

#[test]
fn regress_no_table_tags() {
    let html = r#"<html><body><p>No tables here</p><div>Some content</div></body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert!(tables.is_empty(), "no <table> tags -> no tables");
}

#[test]
fn regress_only_comment() {
    let html = r#"<!-- just a comment, no table -->"#;
    let tables = extract_tables(html).unwrap();
    assert!(tables.is_empty());
}

// ── 36. H3 After Table (Should Not Affect Current Table) ───────────────────

#[test]
fn regress_h3_after_table() {
    let html = r#"<html><body>
        <table summary="First">
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
        <h3>Unrelated</h3>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0].name, "First",
        "h3 after table should not override its name"
    );
}

// ── 37. DOCTYPE Declaration ─────────────────────────────────────────────────

#[test]
fn regress_doctype_html() {
    let html = r#"<!DOCTYPE html>
    <html lang="en">
    <head><meta charset="UTF-8"><title>Test</title></head>
    <body>
        <table summary="DocType Test">
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body>
    </html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "DocType Test");
    assert_eq!(tables[0].rows[0][0], "data");
}

// ── 38. Tables Inside `<div>` / Other Container Elements ────────────────────

#[test]
fn regress_table_in_div() {
    let html = r#"<html><body>
        <div class="container">
            <div class="inner">
                <table summary="Nested in div">
                    <tr><th>K</th><th>V</th></tr>
                    <tr><td>key1</td><td>val1</td></tr>
                </table>
            </div>
        </div>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "Nested in div");
    assert_eq!(tables[0].rows[0][0], "key1");
}

// ── 39. `colgroup` / `col` Elements ─────────────────────────────────────────

#[test]
fn regress_colgroup_elements() {
    let html = r#"<html><body>
        <table summary="colgroup test">
            <colgroup>
                <col style="width:100px"/>
                <col style="width:200px"/>
            </colgroup>
            <tr><th>A</th><th>B</th></tr>
            <tr><td>1</td><td>2</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["A", "B"]);
    assert_eq!(tables[0].rows[0], ["1", "2"]);
}

// ── 40. Table with Only One Cell ────────────────────────────────────────────

#[test]
fn regress_single_cell_table() {
    let html = r#"<html><body>
        <table summary="Single">
            <tr><td>only cell</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // No <th>, first (and only) row becomes header
    assert_eq!(tables[0].headers, ["only cell"]);
    assert!(
        tables[0].rows.is_empty(),
        "single cell: promoted to header, no data rows left"
    );
}

// ── 41. `<th>` with `scope` Attribute ───────────────────────────────────────

#[test]
fn regress_th_with_scope() {
    let html = r#"<html><body>
        <table summary="scope test">
            <tr>
                <th scope="col">Name</th>
                <th scope="col">Value</th>
            </tr>
            <tr>
                <th scope="row">Row Label</th>
                <td>42</td>
            </tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Name", "Value"]);
    // Row with th[scope=row] + td: th_cells=["Row Label"], td_cells=["42"]
    // th_cells.len() == 1 -> merged
    assert_eq!(tables[0].rows[0], ["Row Label", "42"]);
}

// ── 42. Very Large Number of Columns (Stress Test for Allocation) ──────────

#[test]
fn regress_wide_table() {
    let html_content: String = (0..50)
        .map(|i| format!("<th>H{}</th>", i))
        .collect::<Vec<_>>()
        .join("");
    let row_content: String = (0..50)
        .map(|i| format!("<td>V{}</td>", i))
        .collect::<Vec<_>>()
        .join("");
    let html = format!(
        r#"<html><body>
        <table summary="Wide"><tr>{}</tr><tr>{}</tr></table>
    </body></html>"#,
        html_content, row_content
    );
    let tables = extract_tables(&html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers.len(), 50, "50-column table");
    assert_eq!(tables[0].rows[0].len(), 50);
    assert_eq!(tables[0].rows[0][49], "V49");
}

// ── 43. Very Large Number of Rows (Stress) ─────────────────────────────────

#[test]
fn regress_long_table() {
    let header = "<tr><th>ID</th><th>Val</th></tr>";
    let rows: String = (0..500)
        .map(|i| format!("<tr><td>{}</td><td>x</td></tr>", i))
        .collect::<Vec<_>>()
        .join("");
    let html = format!(
        r#"<html><body>
        <table summary="Long">{}{}</table>
    </body></html>"#,
        header, rows
    );
    let tables = extract_tables(&html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows.len(), 500, "500-row table");
    assert_eq!(tables[0].rows[499][0], "499");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Irregular Tables (browser-renderable but non-compliant HTML structures)
// ═══════════════════════════════════════════════════════════════════════════════

// ── 44. `<td>` / `<th>` Without `<tr>` Wrapper ──────────────────────────────
// Browsers auto-insert implicit <tr> when <td>/<th> are direct children of <table>

#[test]
fn regress_td_without_tr() {
    // HTML5 parser wraps all bare <th>/<td> in a single implicit <tr>.
    // The extraction code uses `continue` after setting headers from that row,
    // so the <td> cells in the same implicit <tr> are skipped.
    let html = r#"<html><body>
        <table summary="no tr">
            <th>Name</th>
            <th>Value</th>
            <td>Alpha</td>
            <td>100</td>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Name", "Value"]);
    // td cells in the same implicit <tr> as th are skipped due to `continue`
    assert!(
        tables[0].rows.is_empty(),
        "td in same implicit tr as th are skipped by current logic"
    );
}

// ── 45. `<th>` Only Without `<tr>` ──────────────────────────────────────────

#[test]
fn regress_th_only_no_tr() {
    let html = r#"<html><body>
        <table summary="th only">
            <th>ColA</th>
            <th>ColB</th>
            <th>ColC</th>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["ColA", "ColB", "ColC"]);
    assert!(
        tables[0].rows.is_empty(),
        "only th elements -> headers only"
    );
}

// ── 46. Uppercase Tag Names ─────────────────────────────────────────────────
// Some HTML generators produce <TABLE>, <TR>, <TD>, <TH>

#[test]
fn regress_uppercase_tags() {
    let html = r#"<html><body>
        <TABLE SUMMARY="Uppercase">
            <TR><TH>Name</TH><TH>Age</TH></TR>
            <TR><TD>Alice</TD><TD>30</TD></TR>
            <TR><TD>Bob</TD><TD>25</TD></TR>
        </TABLE>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "Uppercase");
    assert_eq!(tables[0].headers, ["Name", "Age"]);
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].rows[0][0], "Alice");
}

// ── 47. Mixed Case Tag Names ────────────────────────────────────────────────
// e.g., <TABLE>, <tR>, <Td>, <tH>

#[test]
fn regress_mixed_case_tags() {
    let html = r#"<html><body>
        <table summary="MixedCase">
            <Tr><Th>Key</Th><Th>Value</Th></Tr>
            <Tr><Td>x</Td><Td>42</Td></Tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "MixedCase");
    assert_eq!(tables[0].headers, ["Key", "Value"]);
    assert_eq!(tables[0].rows[0], ["x", "42"]);
}

// ── 48. Single-Quoted Attributes ────────────────────────────────────────────
// Browsers accept single quotes: summary='My Table'

#[test]
fn regress_single_quoted_attr() {
    let html = r#"<html><body>
        <table summary='Single Quoted'>
            <tr><th>A</th><th>B</th></tr>
            <tr><td>1</td><td>2</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0].name, "Single Quoted",
        "single-quoted summary attr should be parsed"
    );
    assert_eq!(tables[0].headers, ["A", "B"]);
}

// ── 49. Unquoted Attribute Values ───────────────────────────────────────────
// Browsers accept unquoted attr values: summary=MyTable

#[test]
fn regress_unquoted_attr() {
    let html = r#"<html><body>
        <table summary=Simple>
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper may or may not parse unquoted attr
    assert_eq!(
        tables[0].headers,
        ["H"],
        "table content should be extracted"
    );
    assert_eq!(tables[0].rows[0][0], "data");
}

// ── 50. `<td>` Without Closing Tag (Browser Auto-Closes) ────────────────────
// `<td>cell1<td>cell2` — browser treats second `<td>` as closing the first

#[test]
fn regress_td_no_close() {
    let html = r#"<html><body>
        <table summary="no close">
            <tr><th>A<th>B<th>C</tr>
            <tr><td>1<td>2<td>3</tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper's html5 parser auto-closes unclosed td/th
    assert_eq!(
        tables[0].headers,
        ["A", "B", "C"],
        "unclosed <th> should still parse: got {:?}",
        tables[0].headers
    );
    assert_eq!(
        tables[0].rows[0],
        ["1", "2", "3"],
        "unclosed <td> should still parse: got {:?}",
        tables[0].rows[0]
    );
}

// ── 51. Missing `<th>` Closing Tag, Mixed With `<td>` ──────────────────────

#[test]
fn regress_th_no_close_mixed() {
    let html = r#"<html><body>
        <table summary="mixed close">
            <tr><th>Name<th>Age<td>extra</tr>
            <tr><td>Alice<td>30</tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // The first row has 2 th + 1 td; th_cells=["Name","Age"], td_cells=["extra"]
    // header_row_seen is false so headers become ["Name","Age"]
    // Then th_cells.len() == 1? No, th_cells.len() == 2 -> td-only row cells=["extra"]
    assert_eq!(
        tables[0].headers,
        ["Name", "Age"],
        "first two th become headers: got {:?}",
        tables[0].headers
    );
}

// ── 52. Layout Table With No `<th>` — Pure `<td>` Grid ──────────────────────
// Many real-world layout tables have no headers at all

#[test]
fn regress_layout_table_td_only() {
    let html = r#"<html><body>
        <table>
            <tr><td>Product</td><td>Price</td><td>Qty</td></tr>
            <tr><td>Widget</td><td>9.99</td><td>100</td></tr>
            <tr><td>Gadget</td><td>24.99</td><td>50</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // First row promoted to header (no <th> found)
    assert_eq!(tables[0].headers, ["Product", "Price", "Qty"]);
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].rows[0][0], "Widget");
}

// ── 53. Layout Table With Nested Rows (Grid Inside Grid) ────────────────────
// Tables nested as layout cells (common in old-school HTML)

#[test]
fn regress_layout_nested_grid() {
    let html = r#"<html><body>
        <table summary="Layout">
            <tr>
                <td valign="top">
                    <table summary="Nav">
                        <tr><td>Link1</td></tr>
                        <tr><td>Link2</td></tr>
                    </table>
                </td>
                <td>
                    <table summary="Content">
                        <tr><td>Main content here</td></tr>
                    </table>
                </td>
            </tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 3, "outer + 2 inner layout tables");
    assert!(tables.iter().any(|t| t.name == "Layout"));
    assert!(tables.iter().any(|t| t.name == "Nav"));
    assert!(tables.iter().any(|t| t.name == "Content"));
}

// ── 54. `border` Attribute Without Summary ──────────────────────────────────
// Classic: <table border="1"> with no summary

#[test]
fn regress_table_border_no_summary() {
    let html = r#"<html><body>
        <table border="1">
            <tr><th>H</th></tr>
            <tr><td>data</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "Table_1", "no summary -> auto name");
    assert_eq!(tables[0].rows[0][0], "data");
}

// ── 55. `<tr>` With Deprecated Attributes (`align`, `bgcolor`) ─────────────

#[test]
fn regress_tr_deprecated_attrs() {
    let html = r##"<html><body>
        <table summary="deprecated">
            <tr align="center"><th>Item</th><th>Count</th></tr>
            <tr bgcolor="#ffffff"><td>X</td><td>1</td></tr>
        </table>
    </body></html>"##;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Item", "Count"]);
    assert_eq!(tables[0].rows[0], ["X", "1"]);
}

// ── 56. `<table>` Inside `<form>` ──────────────────────────────────────────
// Very common in real-world HTML

#[test]
fn regress_table_in_form() {
    let html = r#"<html><body>
        <form action="/submit" method="post">
            <table summary="Form Table">
                <tr><th>Field</th><th>Input</th></tr>
                <tr><td>Name:</td><td><input type="text" name="name"/></td></tr>
                <tr><td>Email:</td><td><input type="email" name="email"/></td></tr>
            </table>
        </form>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "Form Table");
    assert_eq!(tables[0].headers, ["Field", "Input"]);
    // Input elements are self-closing — no text content in <td>
    assert_eq!(tables[0].rows[0][0], "Name:");
    // The second cell has <input> which has no text -> empty string
    assert_eq!(
        tables[0].rows[0][1], "",
        "input element has no text content"
    );
}

// ── 57. `<table>` With `cellpadding` / `cellspacing` / `width` ─────────────
// Common legacy attributes that don't affect extraction

#[test]
fn regress_table_cell_attrs() {
    let html = r#"<html><body>
        <table summary="cell attrs" cellpadding="5" cellspacing="0" width="100%">
            <tr><th>A</th><th>B</th></tr>
            <tr><td>1</td><td>2</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "cell attrs");
    assert_eq!(tables[0].rows[0], ["1", "2"]);
}

// ── 58. `<col>` With `span` Attribute ──────────────────────────────────────

#[test]
fn regress_col_span_attr() {
    let html = r#"<html><body>
        <table summary="col span">
            <colgroup>
                <col span="2" style="background:red"/>
                <col style="background:blue"/>
            </colgroup>
            <tr><th>A</th><th>B</th><th>C</th></tr>
            <tr><td>1</td><td>2</td><td>3</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["A", "B", "C"]);
    assert_eq!(tables[0].rows[0], ["1", "2", "3"]);
}

// ── 59. `<tr>` With Text Nodes Between `<td>` Elements ─────────────────────
// Some generators produce: <tr>desc<td>cell</td>desc<td>cell</td></tr>

#[test]
fn regress_tr_with_text_nodes() {
    let html = r#"<html><body>
        <table summary="text in tr">
            <tr>Header<tr><th>Item</th><th>Price</th></tr>
            <tr>Data<tr><td>A</td><td>$1</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // The extra "Header" and "Data" text nodes inside <tr> are ignored by scraper
    assert_eq!(
        tables[0].headers,
        ["Item", "Price"],
        "text nodes in tr should not interfere: got {:?}",
        tables[0].headers
    );
    assert_eq!(tables[0].rows[0][0], "A");
}

// ── 60. Consecutive `<br>` Tags as Row Separators ──────────────────────────
// Some generators put multiple rows of data inside a single <td> separated by <br>

#[test]
fn regress_br_row_separator() {
    let html = r#"<html><body>
        <table summary="br rows">
            <tr><th>Data</th></tr>
            <tr><td>Row1<br/>Row2<br/>Row3</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper's text() concatenates all text, <br> inserts no text
    let cell = &tables[0].rows[0][0];
    assert!(cell.contains("Row1"), "cell contains Row1: '{}'", cell);
    assert!(cell.contains("Row2"), "cell contains Row2: '{}'", cell);
    assert!(cell.contains("Row3"), "cell contains Row3: '{}'", cell);
    // All text is concatenated into one cell (no row splitting)
    assert!(
        !cell.contains('\n'),
        "br-separated text concats without newline"
    );
}

// ── 61. Table With `nowrap` Attribute on `<td>` ────────────────────────────

#[test]
fn regress_td_nowrap() {
    let html = r#"<html><body>
        <table summary="nowrap">
            <tr><th>Name</th><th>Description</th></tr>
            <tr><td nowrap>Short</td><td>Long text here</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows[0], ["Short", "Long text here"]);
}

// ── 62. `<table>` With Mixed `<tr>` and Wbr / Other Inline Elements ────────

#[test]
fn regress_table_wbr_and_others() {
    let html = r#"<html><body>
        <table summary="mixed inline">
            <tr><th>URL</th></tr>
            <tr><td>https://example.com/<wbr/>long<wbr/>path</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // <wbr> produces no text, scrapers's text() skips it
    assert_eq!(tables[0].rows[0][0], "https://example.com/longpath");
}

// ── 63. `<td>` With Nested `<div>` / `<p>` Block Elements ──────────────────

#[test]
fn regress_td_with_block_elements() {
    let html = r#"<html><body>
        <table summary="block in td">
            <tr><th>Description</th></tr>
            <tr><td><p>Paragraph one.</p><p>Paragraph two.</p></td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper's text() concatenates all text: "Paragraph one.Paragraph two."
    let cell = &tables[0].rows[0][0];
    assert!(
        cell.contains("Paragraph one"),
        "p text extracted: '{}'",
        cell
    );
    assert!(
        cell.contains("Paragraph two"),
        "p text extracted: '{}'",
        cell
    );
}

// ── 64. `<table>` With Duplicate Header Row (Copy-Paste Artifact) ──────────

#[test]
fn regress_duplicate_header_row() {
    let html = r#"<html><body>
        <table summary="dup header">
            <tr><th>Name</th><th>Score</th></tr>
            <tr><th>Name</th><th>Score</th></tr>
            <tr><td>Alice</td><td>95</td></tr>
            <tr><td>Bob</td><td>87</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // header_row_seen is set after first <th> row; second <th> row falls through
    // th_cells = ["Name", "Score"], td_cells = [] -> cells.is_empty() -> skipped
    assert_eq!(tables[0].headers, ["Name", "Score"]);
    assert_eq!(
        tables[0].rows.len(),
        2,
        "duplicate header row should be skipped: got {} rows",
        tables[0].rows.len()
    );
    assert_eq!(tables[0].rows[0][0], "Alice");
}

// ── 65. `<table>` With Merged Header Row (Single `<th>` With Colspan) ──────

#[test]
fn regress_merged_header_row() {
    let html = r#"<html><body>
        <table summary="merged header">
            <tr><th colspan="2">Merged Header</th></tr>
            <tr><th>SubA</th><th>SubB</th></tr>
            <tr><td>1</td><td>2</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // First row: th_cells=["Merged Header"], header_row_seen=false -> headers=["Merged Header"]
    // Second row: th_cells=["SubA","SubB"] -> header_row_seen=true -> skipped? No, it's seen.
    //   th_cells.len()==2 != 1 -> just check td_cells which is [] -> cells.is_empty() -> skip
    // So headers stay as the single merged header
    assert_eq!(
        tables[0].headers.len(),
        1,
        "merged th row becomes sole header: got {:?}",
        tables[0].headers
    );
    assert_eq!(tables[0].headers[0], "Merged Header");
    // Data row is found
    assert_eq!(tables[0].rows[0], ["1", "2"]);
}

// ── 66. `<table>` With No Summary and No `<th>` — Multi-Column Layout Grid ─

#[test]
fn regress_layout_grid_no_headers() {
    let html = r#"<html><body>
        <table>
            <tr><td>Photo1</td><td>Photo2</td><td>Photo3</td></tr>
            <tr><td>Desc1</td><td>Desc2</td><td>Desc3</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // First row promoted to header
    assert_eq!(tables[0].headers, ["Photo1", "Photo2", "Photo3"]);
    assert_eq!(tables[0].rows[0], ["Desc1", "Desc2", "Desc3"]);
}

// ── 67. `<td>` With Extra Whitespace / Newlines Inside Tags ─────────────────

#[test]
fn regress_td_whitespace_in_tags() {
    let html = r#"<html><body>
        <table summary="ws">
            <tr>
                <th  >Name</th>
                <th
                >Value</th>
            </tr>
            <tr>
                <td

                >Item</td>
                <td  >42</td>
            </tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["Name", "Value"]);
    assert_eq!(tables[0].rows[0], ["Item", "42"]);
}

// ── 68. HTML Entities in Attribute (summary) ───────────────────────────────

#[test]
fn regress_entity_in_summary() {
    let html = r#"<html><body>
        <table summary="Tom &amp; Jerry Table">
            <tr><th>Character</th></tr>
            <tr><td>Tom</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper decodes HTML entities in attributes
    assert_eq!(tables[0].name, "Tom & Jerry Table");
}

// ── 69. `<script>` Inside `<td>` (Should Be Stripped) ──────────────────────
// Rare, but happens with broken HTML

#[test]
fn regress_script_in_td() {
    let html = r#"<html><body>
        <table summary="script in td">
            <tr><th>Data</th></tr>
            <tr><td>real content <script>var x = 1;</script> more</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    // scraper's text() does NOT skip <script> content — it concatenates all text nodes
    // including JS code. This is a known limitation.
    let cell = &tables[0].rows[0][0];
    assert!(
        cell.contains("real content"),
        "real content present: '{}'",
        cell
    );
    // JS content may leak through (scraper behavior)
}

// ── 70. `<style>` Inside `<td>` ────────────────────────────────────────────

#[test]
fn regress_style_in_td() {
    let html = r#"<html><body>
        <table summary="style in td">
            <tr><th>Item</th></tr>
            <tr><td>visible <style>.hidden{}</style> text</td></tr>
        </table>
    </body></html>"#;
    let tables = extract_tables(html).unwrap();
    assert_eq!(tables.len(), 1);
    let cell = &tables[0].rows[0][0];
    assert!(cell.contains("visible"), "visible text: '{}'", cell);
    assert!(cell.contains("text"), "trailing text: '{}'", cell);
}
