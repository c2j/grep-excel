# Skill: Batch Processing Pipelines

Automate multi-step data processing workflows with `--exec` pipelines and `--run` shell commands — no scripting required.

## Prerequisites

- grep-excel with `--features mcp-server` (for `--exec` save/edit tools)
- Basic JSON syntax knowledge

## `--exec` Pipeline Mode

Execute MCP tools directly from the command line. State is preserved across steps in a multi-step pipeline.

### Single Commands

```bash
# List files
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}'

# Search with aggregation
grep_excel data.xlsx --exec '{"tool":"search","params":{"query":"Engineering","mode":"exact","aggregate":"City"}}'

# Execute SQL
grep_excel data.xlsx --exec '{"tool":"execute_sql","params":{"sql":"SELECT * FROM sheet_1_0 LIMIT 5"}}'
```

### Multi-Step Pipelines

Use a JSON array to chain multiple tools. Each step sees the state from previous steps:

```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"data.xlsx"}},
  {"tool":"get_metadata","params":{}},
  {"tool":"get_sheet_sample","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","sample_size":5}},
  {"tool":"search","params":{"query":"张三","mode":"exact"}}
]'
```

### Common Pipeline Patterns

**Pattern 1: Import → Search → Edit → Save**

```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"data.xlsx"}},
  {"tool":"search","params":{"query":"Engineering","mode":"exact"}},
  {"tool":"update_cells","params":{"file_name":"data.xlsx","sheet_name":"Sheet1","updates":[{"row":0,"column":"Department","value":"R&D"},{"row":1,"column":"Department","value":"R&D"}]}},
  {"tool":"save","params":{"file_name":"data.xlsx"}}
]'
```

**Pattern 2: Explore → Analyze → Export**

```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"sales.xlsx"}},
  {"tool":"get_metadata","params":{}},
  {"tool":"get_sheet_statistics","params":{"file_name":"sales.xlsx","sheet_name":"Sheet1"}},
  {"tool":"export_query","params":{"sql":"SELECT Region, SUM(Revenue) FROM sheet_1_0 GROUP BY Region","output_path":"regional.xlsx"}}
]'
```

**Pattern 3: Clean & Enrich**

```bash
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"messy.xlsx"}},
  {"tool":"add_column","params":{"file_name":"messy.xlsx","sheet_name":"Sheet1","column_name":"Status","default_value":"pending"}},
  {"tool":"save_as","params":{"file_name":"messy.xlsx","output_path":"clean.xlsx"}}
]'
```

### Output Format Control

```bash
# JSON output (default for --exec)
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}'

# Markdown table output
grep_excel data.xlsx --exec '{"tool":"list_files","params":{}}' -f markdown

# Simple TSV output
grep_excel data.xlsx --exec '{"tool":"get_metadata","params":{}}' -f simple
```

Use `grep_excel --exec help` to list all available tools and their parameters.

## `--run` Shell Command Mode

Execute external commands for each matching row, using `${column_name}` for cell values.

### Basic Usage

```bash
grep_excel data.xlsx -q "ERROR" -c "Level" --run './analyzer "${Message}"'
```

For each row where the `Level` column matches "ERROR", this runs `./analyzer` with the `Message` column value as argument.

### Enrich Data with External Tools

```bash
# Run classifier for each row, write results to a new column
grep_excel data.xlsx -q "TODO" -c "Type" \
  --run './classifier "${Title}"' \
  --run-output-column "Category" \
  -e enriched.xlsx
```

This classifies each matching row with an external tool, writes the result to the `Category` column, and exports the enriched file.

### Combine with SQL

```bash
# Select specific rows via SQL, then process with shell
grep_excel data.xlsx \
  --sql "SELECT Name, SQL FROM sheet_1_0 WHERE Type='legacy'" \
  --run './formatter "${SQL}"'
```

### Variable Syntax

- `${column_name}` — substituted with the cell value (auto shell-escaped with single quotes)
- `$$` — literal `$` character
- Commands execute via `sh -c`

## Real-World Examples

### Clean a Database Export

```bash
# Find nulls → fill defaults → add audit column → export
grep_excel --exec '[
  {"tool":"import_file","params":{"file_path":"db_export.csv"}},
  {"tool":"add_column","params":{"file_name":"db_export.csv","sheet_name":"Sheet1","column_name":"cleaned_at","default_value":"2026-01-01"}},
  {"tool":"save_as","params":{"file_name":"db_export.csv","output_path":"cleaned_export.xlsx"}}
]'
```

### Batch Classify Data

```bash
# Import → classify each row → export with new category column
grep_excel data.xlsx -q "unprocessed" -c "Status" \
  --run './categorizer "${Description}"' \
  --run-output-column "Category" \
  -e categorized.xlsx
```

## Tips

- **Discover tools**: `grep_excel --exec help` lists all available `--exec` tools
- **State preservation**: In multi-step pipelines, imported files and temp tables persist across all steps
- **temp tables in pipelines**: Use `materialize_query` mid-pipeline to cache intermediate SQL results
- **`--run` requires `-q` or `-x`**: Always pair `--run` with a search query (`-q`) or SQL query (`-x`)
- **Export after `--run`**: Use `-e` to export the full dataset (not just matched rows) after enrichment

## See Also

- [Analyzing Database Exports](./analyzing-database-exports.md) — SQL analysis patterns
- [Using with AI Assistants](./using-with-ai-assistants.md) — MCP-based workflows
