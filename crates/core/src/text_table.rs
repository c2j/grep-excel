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
}
