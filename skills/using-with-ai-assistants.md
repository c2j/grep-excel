# Skill: Using grep-excel with AI Assistants

Integrate grep-excel's search and editing capabilities with AI assistants (Claude Desktop, Cursor) via the MCP (Model Context Protocol) server.

## Prerequisites

- grep-excel built with `--features mcp-server` (included in `full`)
- An MCP-compatible AI assistant (Claude Desktop, Cursor, etc.)

## Setup

### Claude Desktop

Edit `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/usr/local/bin/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

**Path tips:**
- macOS (Homebrew): `/opt/homebrew/bin/grep_excel` or `/usr/local/bin/grep_excel`
- Linux: `~/.cargo/bin/grep_excel` or `/usr/local/bin/grep_excel`
- Windows: `C:\\Users\\YourName\\.cargo\\bin\\grep_excel.exe`

### Cursor

In Cursor Settings → MCP, add the same configuration:

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/usr/local/bin/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

### Verify Connection

After restarting the AI assistant, ask: "List the grep-excel tools you have available."

The assistant should be able to call `list_files`, `import_file`, `search`, etc.

## Workflows

### Exploring an Unknown File

```
You: Import data.xlsx and tell me what's in it
AI: [calls import_file]
    → Shows file name, sheet count, and total rows

AI: [calls get_metadata]
    → Shows each sheet's column names and row counts

AI: [calls get_sheet_sample, sample_size=5]
    → Shows 5 evenly-spaced rows to understand the data shape
```

**Pattern**: `import_file` → `get_metadata` → `get_sheet_sample`

Always use `get_sheet_sample` instead of `get_sheet_data` for initial exploration — it's faster and gives a representative overview.

### Data Analysis with SQL

```
You: Analyze the sales data — total revenue by region
AI: [calls import_file, sales.xlsx]
AI: [calls execute_sql]
    → SELECT Region, SUM(Revenue) AS total
      FROM sales.Sheet1
      GROUP BY Region
      ORDER BY total DESC
```

Use friendly aliases (`sales.Sheet1`) — run `list_files` first if you don't know the alias.

### Multi-Step Analysis with Temp Tables

```
You: Find departments where average salary exceeds 100K,
     then list the employees in those departments
AI: [calls materialize_query, name="high_pay_depts"]
    → SELECT Department FROM employees.Sheet1
      GROUP BY Department HAVING AVG(Salary) > 100000

AI: [calls execute_sql]
    → SELECT Name, Department, Salary
      FROM employees.Sheet1
      WHERE Department IN (SELECT Department FROM high_pay_depts)
      ORDER BY Salary DESC

AI: [calls drop_temp_table, name="high_pay_depts"]
```

**Pattern**: `materialize_query` (cache expensive subquery) → `execute_sql` (use it) → `drop_temp_table` (clean up)

### Editing & Saving

```
You: Change "Engineering" to "R&D" for all rows
AI: [calls search, query="Engineering", mode="exact"]
    → Finds 45 matches

AI: [calls update_cells]
    → Batch updates all matching rows (column="Department", value="R&D")

You: Save the changes
AI: [calls save]
    → Overwrites the original file
```

For safety, use `save_as` first to create a backup before `save`.

### Data Profiling

```
You: What's the data quality like? Any null values?
AI: [calls get_sheet_statistics]
    → Per-column stats: null counts, distinct counts, top values
    → "Salary: 3 nulls out of 200 rows. 45 distinct values. Top: 50000(15), 48000(12)"
```

Use `get_sheet_statistics` for quick data quality assessment before running queries.

## Tips

- **Preview first**: Always `get_sheet_sample` before `search` or `execute_sql` — understand the schema to write correct queries
- **Friendly aliases**: Use `filename.SheetName` in SQL, not internal `sheet_1_0` names
- **Pagination is numeric**: `get_sheet_data` params `start_row` and `end_row` must be numbers (`0`, `100`), not strings
- **Temp tables for complex queries**: Materialize expensive subqueries for reuse across multiple analysis steps
- **Backup before save**: Use `save_as` before `save` when editing production data

## Troubleshooting

**"MCP server not connecting"**
- Verify the binary path is absolute and correct: `which grep_excel`
- Check that the binary was built with `--features mcp-server`
- Restart the AI assistant after adding the MCP config

**"Tool not found"**
- The assistant may need to re-discover tools after restart
- Run `grep_excel --exec help` to see all available tools

## See Also

- [Analyzing Database Exports](./analyzing-database-exports.md) — SQL analysis patterns
- [Batch Processing Pipelines](./batch-processing-pipelines.md) — CLI automation alternative
- [MCP Server Mode](../docs/UserGuide.md#mcp-server-mode) — full MCP documentation
