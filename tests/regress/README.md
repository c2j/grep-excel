# Regression Test Fixtures — HTML Table Extraction

This directory contains HTML test fixtures for the regression test suite.

The regression test code lives at `crates/core/tests/regress.rs` (core crate integration test).
Each test uses inline HTML strings to be self-contained; the fixtures here serve as
reference examples and documentation for each edge case.

## Test Cases Coverage

| #  | Edge Case                  | Description                                      |
|----|----------------------------|--------------------------------------------------|
| 1  | HTML Fragment              | Bare `<table>` without `<html>/<body>` wrappers  |
| 2  | Semantic Sections          | `<thead>`, `<tbody>`, `<tfoot>` structure        |
| 3  | `colspan`                  | Merged header/data cells                         |
| 4  | `rowspan`                  | Vertically merged cells                          |
| 5  | Nested Table               | Table inside a table cell                        |
| 6  | `<caption>` as Name        | Table named via `<caption>` tag                  |
| 7  | Empty & Whitespace Cells   | `<td></td>`, `<td> </td>`, `<td>&nbsp;</td>`    |
| 8  | `<br>` in Cell             | Multi-line cell content                          |
| 9  | `<a>` Links in Cell        | Anchor text extraction                           |
| 10 | Embedded CSS/JS            | `<style>` / `<script>` ignored                   |
| 11 | Mid-table `<th>` Rows      | Section header rows inside data                  |
| 12 | No `<th>` (Infer Header)   | All `<td>`, first row becomes header             |
| 13 | Jagged Rows                | Varying column counts across rows                |
| 14 | Missing `</table>`         | Unclosed table tag (malformed HTML)              |
| 15 | Multi-table no names       | Auto-generated Table_1, Table_2 names            |
| 16 | `summary` + `caption`      | Name source prioritization                       |
| 17 | Mixed `<th>+<td>` Row      | Single row with both header/data cells           |
| 18 | HTML Comments               | `<!-- -->` inside table                          |
| 19 | Image in Cell              | `<img>` with `alt` text                          |
| 20 | UTF-8 BOM                  | Byte order mark prefix                           |
| 21 | Chinese Content            | CJK text in table cells                          |
| 22 | Deep Nesting               | Nested tables 3+ levels deep                     |

### Irregular / Non-Compliant HTML (Browser-Renderable)

| #  | Case                          | Description                                    |
|----|-------------------------------|------------------------------------------------|
| 44 | `<td>/<th>` without `<tr>`    | Bare cells directly under `<table>`            |
| 45 | `<th>` only without `<tr>`    | Header cells without row wrapper               |
| 46 | Uppercase tag names           | `<TABLE>`, `<TR>`, `<TD>`, `<TH>`              |
| 47 | Mixed case tag names          | `<Tr>`, `<Td>`, `<Th>`                         |
| 48 | Single-quoted attributes      | `summary='text'`                               |
| 49 | Unquoted attribute values     | `summary=text`                                 |
| 50 | Missing `</td>` close tag     | `<td>cell1<td>cell2` (browser auto-closes)     |
| 51 | Missing `</th>` + mixed td    | Unclosed `<th>` interleaved with `<td>`        |
| 52 | Layout table pure `<td>` grid | No `<th>` at all (layout tables)               |
| 53 | Nested layout tables          | Old-school table-based layouts                 |
| 54 | `border` attr, no summary     | `<table border="1">`                           |
| 55 | Deprecated `align`/`bgcolor`  | Legacy attributes on `<tr>`                    |
| 56 | `<table>` inside `<form>`     | Form-wrapped tables (very common real-world)   |
| 57 | `cellpadding`/`cellspacing`   | Classic table attributes                       |
| 58 | `<col span="N">`              | Column spanning groups                         |
| 59 | Text nodes in `<tr>`          | Mixed text + `<td>` inside `<tr>`              |
| 60 | `<br>` as row separator       | Multi-row data in single `<td>` via `<br>`     |
| 61 | `nowrap` on `<td>`            | Deprecated inline attribute                    |
| 62 | `<wbr>` in cell               | Word break opportunity element                 |
| 63 | `<div>`/`<p>` blocks in `<td>`| Block elements inside table cells              |
| 64 | Duplicate header row          | Copy-paste artifact with repeated `<th>` row   |
| 65 | Merged header (colspan)       | Single `<th colspan="N">` as sole header       |
| 66 | Layout grid, no headers       | Multi-column grid with no `<th>`               |
| 67 | Whitespace inside tags        | Newlines/spaces between tag name and `>`       |
| 68 | HTML entity in `summary`      | `summary="Tom &amp; Jerry"`                    |
| 69 | `<script>` inside `<td>`      | JS code in table cells                         |
| 70 | `<style>` inside `<td>`       | CSS in table cells                             |
