use std::path::Path;

/// A single table extracted from markdown or plain text.
/// Mirrors the sheet-level abstraction used by Excel import.
#[derive(Debug, Clone)]
pub struct TableData {
    /// Table name derived from a preceding heading, section title, or index
    pub name: String,
    /// Column headers
    pub headers: Vec<String>,
    /// Data rows (each row = Vec of cell text)
    pub rows: Vec<Vec<String>>,
}

// ── GFM Pipe Table Parser ────────────────────────────────────────────────────

/// Extract all GFM pipe tables from a markdown string.
/// Returns a Vec of TableData, one per pipe table found.
pub fn extract_tables_md(text: &str) -> Vec<TableData> {
    let mut tables = Vec::new();
    let mut in_code_block = false;
    let mut current_heading: Option<String> = None;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Track code blocks (``` fences)
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            i += 1;
            continue;
        }
        if in_code_block {
            i += 1;
            continue;
        }

        // Track headings for sheet naming
        if let Some(heading) = extract_heading(line) {
            if !heading.is_empty() {
                current_heading = Some(heading);
            }
            i += 1;
            continue;
        }

        // Check for pipe table start: current line has pipe, next line is separator
        if is_pipe_line(line) && i + 1 < lines.len() && is_pipe_separator(lines[i + 1]) {
            let mut table_rows: Vec<Vec<String>> = Vec::new();

            // Header row
            table_rows.push(parse_pipe_row(line));

            // Skip separator line
            i += 1;

            // Data rows — continue until blank line or non-pipe line
            i += 1;
            while i < lines.len() {
                let data_line = lines[i];
                if data_line.trim().is_empty() || !is_pipe_line(data_line) {
                    break;
                }
                table_rows.push(parse_pipe_row(data_line));
                i += 1;
            }

            if table_rows.is_empty() {
                continue;
            }

            let headers = table_rows.remove(0);
            let name = current_heading
                .clone()
                .unwrap_or_else(|| format!("Table_{}", tables.len() + 1));

            tables.push(TableData {
                name,
                headers,
                rows: table_rows,
            });
            continue;
        }

        i += 1;
    }

    tables
}

/// Check if a line looks like a pipe table row (starts and ends with `|`)
fn is_pipe_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|')
}

/// Check if a line is a pipe table separator (contains only `-`, `:`, ` `, `|`)
fn is_pipe_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return false;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    // Each cell in the separator must contain only dashes, colons, and spaces
    inner.split('|').all(|cell| {
        let c = cell.trim();
        !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
    })
}

/// Split a pipe-delimited row into cell strings, trimming each cell
fn parse_pipe_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    // Strip leading and trailing pipe
    let inner = if trimmed.starts_with('|') && trimmed.ends_with('|') {
        &trimmed[1..trimmed.len() - 1]
    } else if trimmed.starts_with('|') {
        &trimmed[1..]
    } else if trimmed.ends_with('|') {
        &trimmed[..trimmed.len() - 1]
    } else {
        trimmed
    };
    inner.split('|').map(|cell| cell.trim().to_string()).collect()
}

/// Extract heading text from a markdown heading line (# to ######)
fn extract_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    for prefix in &["###### ", "##### ", "#### ", "### ", "## ", "# "] {
        if let Some(text) = trimmed.strip_prefix(prefix) {
            let h = text.trim().to_string();
            if !h.is_empty() {
                return Some(h);
            }
        }
    }
    None
}

// ── TXT Table Parser ─────────────────────────────────────────────────────────

const MIN_TABLE_ROWS: usize = 3; // header + separator + 1 data row

/// A section delimited by ~~~ markers in a text file.
struct Section {
    /// Section title (line before the ~~~)
    title: String,
    /// Content lines after the ~~~ line
    body: Vec<String>,
}

/// Collect sections from a plain text file, split by ~~~ marker lines.
/// Also returns a "preamble" section (content before the first ~~~).
fn collect_sections(text: &str) -> (Vec<String>, Vec<Section>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut preamble = Vec::new();
    let mut sections = Vec::new();
    let mut i = 0;

    // Find first tilde line
    while i < lines.len() && !is_tilde_line(lines[i]) {
        let trimmed = lines[i].trim();
        if !trimmed.is_empty() {
            preamble.push(lines[i].to_string());
        } else {
            preamble.push(String::new());
        }
        i += 1;
    }

    // Process sections
    while i < lines.len() {
        if is_tilde_line(lines[i]) {
            // Line before tilde is the section title
            let title = if i > 0 {
                lines[i - 1].trim().to_string()
            } else {
                format!("Section_{}", sections.len() + 1)
            };

            let mut body = Vec::new();
            i += 1; // Skip tilde line
            while i < lines.len() && !is_tilde_line(lines[i]) {
                body.push(lines[i].to_string());
                i += 1;
            }
            // Don't skip past the tilde — the loop will handle it

            if !body.iter().all(|l| l.trim().is_empty()) {
                sections.push(Section { title, body });
            }
        } else {
            i += 1;
        }
    }

    (preamble, sections)
}

/// Check if a line is a ~~~ section marker
fn is_tilde_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    trimmed.chars().all(|c| c == '~')
}

/// Check if a line is a dash-separator (e.g., `---- ---- ----` or `- - -`)
fn is_dash_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    // Must contain at least 2 dash runs (2+ dash characters, not necessarily contiguous)
    let dash_count = trimmed.chars().filter(|&c| c == '-').count();
    if dash_count < 2 {
        return false;
    }
    // All non-space chars must be dashes
    trimmed.chars().all(|c| c == ' ' || c == '-')
}

/// Extract column boundaries from a dash-separator line.
/// Returns a Vec of (start, end) byte positions for each column.
/// The `end` represents the right edge of the dash-run; actual cell content
/// may extend further (handled by `split_by_boundaries`).
fn detect_column_boundaries(separator: &str) -> Vec<(usize, usize)> {
    let mut boundaries = Vec::new();
    let bytes = separator.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'-' {
            let start = i;
            while i < len && bytes[i] == b'-' {
                i += 1;
            }
            boundaries.push((start, i));
        } else {
            i += 1;
        }
    }

    boundaries
}

/// Split a line into cells using column boundary positions.
/// Each cell spans from the start of its column to the start of the next column
/// (or the end of the line for the last column). This allows cell content to
/// extend beyond the dash-run width.
fn split_by_boundaries(line: &str, boundaries: &[(usize, usize)]) -> Vec<String> {
    let line_len = line.len();
    boundaries
        .iter()
        .enumerate()
        .map(|(idx, &(start, _))| {
            let end = if idx + 1 < boundaries.len() {
                boundaries[idx + 1].0
            } else {
                line_len
            };
            if line_len > start {
                let slice = &line[start..std::cmp::min(line_len, end)];
                slice.trim().to_string()
            } else {
                String::new()
            }
        })
        .collect()
}

/// Extract a table from a section that contains a dash-separator line.
/// Returns Some(TableData) if a table is found.
fn extract_table_from_section(section: &Section) -> Option<TableData> {
    let body_lines: Vec<&str> = section.body.iter().map(|s| s.as_str()).collect();
    if body_lines.len() < MIN_TABLE_ROWS {
        return None;
    }

    // Find the dash-separator line
    let mut separator_idx = None;
    for (idx, line) in body_lines.iter().enumerate() {
        if is_dash_separator(line) {
            separator_idx = Some(idx);
            break;
        }
    }

    let separator_idx = separator_idx?;

    // We need at least one row below the separator
    if separator_idx + 1 >= body_lines.len() {
        return None;
    }

    let separator_line = body_lines[separator_idx];
    let boundaries = detect_column_boundaries(separator_line);

    if boundaries.is_empty() || boundaries.len() < 2 {
        return None;
    }

    // Collect header lines (above separator, walking upward)
    let mut header_lines: Vec<&str> = Vec::new();
    let mut hi = separator_idx;
    while hi > 0 {
        hi -= 1;
        let line = body_lines[hi];
        let trimmed = line.trim();
        if trimmed.is_empty()
            || is_tilde_line(line)
            || is_dash_separator(line)
        {
            break;
        }
        // Check if this line looks like a section title (short, no overlap with boundaries)
        let first_content_pos = line.find(|c: char| !c.is_whitespace());
        let first_col_start = boundaries.first().map(|&(s, _)| s).unwrap_or(0);
        match first_content_pos {
            Some(pos) if pos < first_col_start.saturating_sub(2) => {
                // Content starts well before first column — likely a title, stop
                break;
            }
            None => break,
            _ => {}
        }
        header_lines.push(line);
    }
    header_lines.reverse();

    // If no header lines found, use the line immediately before separator
    if header_lines.is_empty() && separator_idx > 0 {
        header_lines.push(body_lines[separator_idx - 1]);
    }

    // Parse headers (handle multi-line merge)
    let headers = if header_lines.len() == 1 {
        split_by_boundaries(header_lines[0], &boundaries)
    } else if header_lines.is_empty() {
        vec![String::new(); boundaries.len()]
    } else {
        // Multi-line header merge
        let mut merged = vec![String::new(); boundaries.len()];
        for hline in &header_lines {
            let cells = split_by_boundaries(hline, &boundaries);
            for (i, cell) in cells.iter().enumerate() {
                if i < merged.len() {
                    if merged[i].is_empty() {
                        merged[i] = cell.clone();
                    } else if !cell.is_empty() {
                        merged[i] = format!("{} {}", merged[i], cell);
                    }
                }
            }
        }
        merged
    };

    // Parse data rows (lines below separator until blank/tilde/next separator)
    let mut rows = Vec::new();
    for data_line in body_lines.iter().skip(separator_idx + 1) {
        let trimmed = data_line.trim();
        if trimmed.is_empty() || is_tilde_line(data_line) || is_dash_separator(data_line) {
            break;
        }
        let cells = split_by_boundaries(data_line, &boundaries);
        if !cells.iter().all(|c| c.is_empty()) {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return None;
    }

    Some(TableData {
        name: section.title.clone(),
        headers,
        rows,
    })
}

/// Attempt to detect a table in a section by analyzing column alignment,
/// for sections that have a tilde underline but NO dash-separator line.
/// This handles Pattern C from AWR reports (e.g., Host CPU, Instance CPU).
fn detect_table_by_alignment(section: &Section) -> Option<TableData> {
    let body_lines: Vec<&str> = section.body.iter().map(|s| s.as_str()).collect();
    if body_lines.len() < 2 {
        return None;
    }

    // Filter out blank lines
    let non_blank: Vec<&str> = body_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .copied()
        .collect();

    if non_blank.len() < 3 {
        return None;
    }

    // Parse each line into (token_count, token_start_positions)
    struct LineTokens {
        count: usize,
        positions: Vec<usize>,
    }

    let parsed: Vec<LineTokens> = non_blank
        .iter()
        .map(|line| {
            let mut tokens = Vec::new();
            let mut chars = line.char_indices().peekable();
            while let Some(&(pos, c)) = chars.peek() {
                if c.is_whitespace() {
                    chars.next();
                } else {
                    tokens.push(pos);
                    // Skip to end of this token
                    while let Some(&(_, c2)) = chars.peek() {
                        if c2.is_whitespace() {
                            break;
                        }
                        chars.next();
                    }
                }
            }
            LineTokens {
                count: tokens.len(),
                positions: tokens,
            }
        })
        .collect();

    if parsed.is_empty() {
        return None;
    }

    // Find the most common token count (mode)
    let mut counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for pt in &parsed {
        *counts.entry(pt.count).or_insert(0) += 1;
    }
    let best_count = counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(col, _)| col)
        .unwrap_or(0);

    if best_count < 2 {
        return None;
    }

    // Filter lines matching the best token count
    let matching: Vec<&LineTokens> = parsed.iter().filter(|pt| pt.count == best_count).collect();
    if matching.len() < 3 {
        return None;
    }

    // Compute median start position for each column index
    let mut boundaries: Vec<(usize, usize)> = Vec::new();
    for col in 0..best_count {
        let mut positions: Vec<usize> =
            matching.iter().filter_map(|pt| pt.positions.get(col)).copied().collect();
        if positions.is_empty() {
            return None;
        }
        positions.sort();
        let median = positions[positions.len() / 2];
        let end = if col + 1 < best_count {
            let mut next_positions: Vec<usize> = matching
                .iter()
                .filter_map(|pt| pt.positions.get(col + 1))
                .copied()
                .collect();
            if next_positions.is_empty() {
                non_blank.iter().map(|l| l.len()).max().unwrap_or(80)
            } else {
                next_positions.sort();
                next_positions[next_positions.len() / 2]
            }
        } else {
            non_blank.iter().map(|l| l.len()).max().unwrap_or(80)
        };
        boundaries.push((median, end));
    }

    if boundaries.len() < 2 {
        return None;
    }

    // First line = header, rest = data
    let header = split_by_boundaries(non_blank[0], &boundaries);
    let rows: Vec<Vec<String>> = non_blank[1..]
        .iter()
        .map(|line| split_by_boundaries(line, &boundaries))
        .collect();

    if rows.is_empty() {
        return None;
    }

    Some(TableData {
        name: section.title.clone(),
        headers: header,
        rows,
    })
}

/// Extract tables from a plain text file using section segmentation and
/// dash-separator detection, with alignment-based fallback.
pub fn extract_tables_txt(text: &str) -> Vec<TableData> {
    let (_preamble, sections) = collect_sections(text);
    let mut tables = Vec::new();

    for section in &sections {
        // Primary: try dash-separator detection
        if let Some(table) = extract_table_from_section(section) {
            tables.push(table);
        } else if let Some(table) = detect_table_by_alignment(section) {
            // Fallback: try alignment-based detection (Pattern C)
            tables.push(table);
        }
    }

    tables
}

/// Public entry point: detect file type by extension and dispatch to the
/// appropriate parser. For .md files with no pipe tables, falls back to the
/// TXT parser to handle markdown files with aligned text tables.
pub fn extract_tables(path: &Path, content: &str) -> Result<Vec<TableData>, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "md" || ext == "markdown" {
        let tables = extract_tables_md(content);
        if tables.is_empty() {
            // Fallback: try TXT parser — some .md files have aligned text tables
            let txt_tables = extract_tables_txt(content);
            if !txt_tables.is_empty() {
                return Ok(txt_tables);
            }
        }
        Ok(tables)
    } else {
        Ok(extract_tables_txt(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md_basic_pipe_table() {
        let md = r#"| Name | Age | City |
|---|---|---|
| Alice | 30 | NYC |
| Bob | 25 | SF |
"#;
        let tables = extract_tables_md(md);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "Table_1");
        assert_eq!(tables[0].headers, vec!["Name", "Age", "City"]);
        assert_eq!(tables[0].rows.len(), 2);
        assert_eq!(tables[0].rows[0], vec!["Alice", "30", "NYC"]);
    }

    #[test]
    fn test_md_heading_as_name() {
        let md = r#"## Performance Metrics

| Metric | Value |
|--------|-------|
| CPU | 45% |
"#;
        let tables = extract_tables_md(md);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "Performance Metrics");
    }

    #[test]
    fn test_md_multiple_tables() {
        let md = r#"## Section One

| A | B |
|---|---|
| 1 | 2 |

## Section Two

| C | D |
|---|---|
| 3 | 4 |
"#;
        let tables = extract_tables_md(md);
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].name, "Section One");
        assert_eq!(tables[1].name, "Section Two");
    }

    #[test]
    fn test_md_empty_cells() {
        let md = r#"| Col1 | Col2 | Col3 |
|---|---|---|
| a | | c |
| | b | |
"#;
        let tables = extract_tables_md(md);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].rows[0], vec!["a", "", "c"]);
        assert_eq!(tables[0].rows[1], vec!["", "b", ""]);
    }

    #[test]
    fn test_md_code_block_skips() {
        let md = r#"```markdown
| This | Should | Not |
|---|---|---|
| be | extracted | ! |
```

| Real | Table |
|---|---|
| foo | bar |
"#;
        let tables = extract_tables_md(md);
        assert_eq!(tables.len(), 1, "tables inside code blocks should be skipped");
        assert_eq!(tables[0].headers, vec!["Real", "Table"]);
    }

    #[test]
    fn test_md_no_tables() {
        let tables = extract_tables_md("Just prose.\nNo tables.");
        assert!(tables.is_empty());
    }

    #[test]
    fn test_md_empty() {
        let tables = extract_tables_md("");
        assert!(tables.is_empty());
    }

    #[test]
    fn test_md_no_separator_line() {
        // Without separator line, no table should be detected
        let md = r#"| H1 | H2 |
| A | B |
"#;
        let tables = extract_tables_md(md);
        assert!(tables.is_empty(), "separator-less pipe lines should not parse as table");
    }

    #[test]
    fn test_extract_heading() {
        assert_eq!(extract_heading("# Title").unwrap(), "Title");
        assert_eq!(extract_heading("## Sub Title").unwrap(), "Sub Title");
        assert_eq!(extract_heading("###### Deep").unwrap(), "Deep");
        assert_eq!(extract_heading("Not a heading"), None);
        assert_eq!(extract_heading("#"), None);
        assert_eq!(extract_heading("##   "), None);
    }

    #[test]
    fn test_is_pipe_separator() {
        assert!(is_pipe_separator("|---|---|---|"));
        assert!(is_pipe_separator("|:---|---:|"));
        assert!(is_pipe_separator("|:---:|:---|"));
        assert!(!is_pipe_separator("| a | b |"));
        assert!(!is_pipe_separator("not a separator"));
    }

    #[test]
    fn test_is_pipe_line() {
        assert!(is_pipe_line("| a | b |"));
        assert!(is_pipe_line("  | a | b |  "));
        assert!(!is_pipe_line("a | b"));
        assert!(!is_pipe_line(""));
    }

    #[test]
    fn test_parse_pipe_row() {
        let cells = parse_pipe_row("| a | b | c |");
        assert_eq!(cells, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_md_pipe_trims_whitespace() {
        let md = r#"|  X  |  Y  |  Z  |
|---|---|---|
|  1  |  2  |  3  |
"#;
        let tables = extract_tables_md(md);
        assert_eq!(tables[0].headers, vec!["X", "Y", "Z"]);
        assert_eq!(tables[0].rows[0], vec!["1", "2", "3"]);
    }

    // ── TXT helper tests ─────────────────────────────────────────────────────

    #[test]
    fn test_is_tilde_line() {
        assert!(is_tilde_line("~~~~"));
        assert!(is_tilde_line("~~~~~~~~~~~~"));
        assert!(is_tilde_line("  ~~~~  "));
        assert!(!is_tilde_line("~~~a"));
        assert!(!is_tilde_line(""));
        assert!(!is_tilde_line("--"));
    }

    #[test]
    fn test_is_dash_separator() {
        assert!(is_dash_separator("---- ---- ----"));
        assert!(is_dash_separator("  ---- ----  "));
        assert!(is_dash_separator("-- --- -"));
        assert!(!is_dash_separator("----a----"));
        assert!(!is_dash_separator(""));
        assert!(!is_dash_separator("~~~~"));
    }

    #[test]
    fn test_detect_column_boundaries() {
        let bounds = detect_column_boundaries("---- ---- ----");
        assert_eq!(bounds.len(), 3);
        assert_eq!(bounds[0], (0, 4));
        assert_eq!(bounds[1], (5, 9));
        assert_eq!(bounds[2], (10, 14));

        // Irregular spacing
        let bounds = detect_column_boundaries("-- --- -");
        assert_eq!(bounds.len(), 3);

        // Single-dash separators
        let bounds = detect_column_boundaries("- - -");
        assert_eq!(bounds.len(), 3);
    }

    #[test]
    fn test_split_by_boundaries() {
        let bounds = vec![(0, 4), (5, 9)];
        let cells = split_by_boundaries("Hello World", &bounds);
        assert_eq!(cells, vec!["Hello", "World"]);

        let cells = split_by_boundaries("Short", &bounds);
        assert_eq!(cells, vec!["Short", ""]);
    }

    #[test]
    fn test_collect_sections_basic() {
        let text = r#"Preamble line

Section One
~~~~~~~~~~~~
ColA    ColB
----    ----
1       2

Section Two
~~~~~~~~~~~~
X    Y
-    -
a    b
"#;
        let (preamble, sections) = collect_sections(text);
        assert!(!preamble.is_empty());
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "Section One");
        assert_eq!(sections[1].title, "Section Two");
    }

    #[test]
    fn test_collect_sections_no_tilde() {
        let text = "Just text.\nNo tildes.";
        let (_preamble, sections) = collect_sections(text);
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_tables_txt_basic() {
        let txt = r#"Load Profile
~~~~~~~~~~~~
Metric         Value
------         -----
DB Time(s)     4.1
CPU Time(s)    2.3
"#;
        let tables = extract_tables_txt(txt);
        assert_eq!(tables.len(), 1, "should find one table");
        assert_eq!(tables[0].name, "Load Profile");
        assert_eq!(tables[0].headers, vec!["Metric", "Value"]);
        assert_eq!(tables[0].rows.len(), 2);
    }

    #[test]
    fn test_extract_tables_txt_multi_section() {
        let txt = r#"Section One
~~~~~~~~~~~
ColA    ColB
----    ----
1       2
3       4

Section Two
~~~~~~~~~~~
X        Y        Z
-        -        -
a        b        c
"#;
        let tables = extract_tables_txt(txt);
        assert_eq!(tables.len(), 2, "should find two tables");
        assert_eq!(tables[0].name, "Section One");
        assert_eq!(tables[1].name, "Section Two");
    }

    #[test]
    fn test_extract_tables_txt_no_tables() {
        let tables = extract_tables_txt("Just prose.\nNo sections.");
        assert!(tables.is_empty());
    }

    #[test]
    fn test_extract_tables_txt_empty() {
        let tables = extract_tables_txt("");
        assert!(tables.is_empty());
    }

    #[test]
    fn test_extract_tables_txt_mixed_prose() {
        let txt = r#"Some intro text.

First Table
~~~~~~~~~~~~
K    V
-    -
a    1

Explanation between.

Second Table
~~~~~~~~~~~~
X    Y
-    -
b    2
"#;
        let tables = extract_tables_txt(txt);
        assert_eq!(tables.len(), 2, "prose between sections should be skipped");
    }

    #[test]
    fn test_detect_table_by_alignment_pattern_c() {
        // Pattern C: no dash separator, header directly after tildes
        let txt = r#"Host CPU
~~~~~~~~
%User  %System  %Idle
 45.2    12.3    32.1
 67.8     8.9    23.3
"#;
        let tables = extract_tables_txt(txt);
        assert_eq!(tables.len(), 1, "should detect Pattern C table");
        assert_eq!(tables[0].name, "Host CPU");
        assert_eq!(tables[0].rows.len(), 2);
    }

    #[test]
    fn test_detect_table_by_alignment_not_enough_rows() {
        // Only 1 data row -> not enough for alignment detection
        let txt = r#"Mini
~~~~~
A    B
1    2
"#;
        let tables = extract_tables_txt(txt);
        assert!(tables.is_empty(), "1 data row is not enough");
    }

    #[test]
    fn test_extract_tables_txt_section_no_table() {
        // Section with tilde but no table content at all
        let txt = r#"Empty Section
~~~~~~~~~~~~~~~
Just some text.
Nothing tabular.
"#;
        let tables = extract_tables_txt(txt);
        assert!(tables.is_empty(), "section without table should yield nothing");
    }
}
