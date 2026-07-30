# Skill: Analyzing Database Performance Reports (AWR/WDR)

Learn how to use grep-excel to analyze Oracle AWR (Automatic Workload Repository) and openGauss WDR (Workload Diagnosis Report) HTML reports — turning diagnostic tables into queryable datasets.

## Prerequisites

- grep-excel with `--features full` (DuckDB engine recommended for complex SQL analysis)
- Oracle AWR report (`.html`, `.md`, or `.txt`) or openGauss WDR report (`.html`)

## Supported Report Types

| Report | Database | Formats | Table Naming |
|--------|----------|---------|--------------|
| AWR | Oracle | `.html`, `.md`, `.txt` | `<caption>` tag or auto-generated |
| WDR | openGauss | `.html` | `<table summary="...">` |

Each `<table>` in the HTML becomes a sheet; `<th>` cells become column headers. A typical WDR report yields 25–31 sheets; an AWR report yields 15–25 sheets.

## CLI Workflow

### Step 1: Discover Available Tables

```bash
grep_excel wdr_report.html -t
```

Shows all extracted sheets with friendly aliases:

```
Available tables:
  wdr_report.This table displays report type             → sheet_1_0  (1 rows)
  wdr_report.This table displays snapshot info             → sheet_1_1  (2 rows)
  wdr_report.This table displays host info                 → sheet_1_2  (1 rows)
  wdr_report.This table displays Database Stat             → sheet_1_3  (3 rows)
  wdr_report.This table displays Load Profile              → sheet_1_4  (14 rows)
  wdr_report.This table displays SQL ordered by Elapsed Time → sheet_1_9  (200 rows)
  ...
```

### Step 2: Preview Key Tables

```bash
# Check load profile for DB Time and CPU metrics
grep_excel wdr_report.html -x "
  SELECT \"Metric\", \"Per Second\", \"Per Transaction\"
  FROM \"wdr_report.This table displays Load Profile\"
  WHERE \"Metric\" ILIKE '%DB%' OR \"Metric\" ILIKE '%CPU%'
"

# Preview top 5 SQL by elapsed time
grep_excel wdr_report.html -x "
  SELECT \"Unique SQL Id\", \"Total Elapse Time(us)\", \"Calls\", \"SQL Text\"
  FROM \"wdr_report.This table displays SQL ordered by Elapsed Time\"
  ORDER BY \"Total Elapse Time(us)\" DESC
  LIMIT 5
"
```

### Step 3: Diagnose Performance Issues

**"Why is the database slow?"**

```bash
# 1. Check DB Time ratio → high DB Time vs Elapsed = busy database
grep_excel wdr_report.html -x "
  SELECT * FROM \"wdr_report.This table displays Load Profile\"
  WHERE \"Metric\" IN ('DB Time(us)', 'CPU Time(us)')
"

# 2. Check buffer efficiency → < 95% = potential IO bottleneck
grep_excel wdr_report.html -x "
  SELECT * FROM \"wdr_report.This table displays Instance Efficiency Percentages (Target 100%)\"
  WHERE \"Metric Name\" ILIKE '%buffer%' OR \"Metric Name\" ILIKE '%parse%'
"

# 3. Find the worst SQL by average time
grep_excel wdr_report.html -x "
  SELECT \"SQL Text\",
         \"Total Elapse Time(us)\" / NULLIF(\"Calls\", 0) AS avg_time_us,
         \"Calls\",
         \"Total Elapse Time(us)\"
  FROM \"wdr_report.This table displays SQL ordered by Elapsed Time\"
  ORDER BY \"Total Elapse Time(us)\" DESC
  LIMIT 10
"
```

### Step 4: Cross-Table Analysis

Join SQL stats with full SQL text (WDR v1 splits them; v2 includes inline):

```bash
grep_excel wdr_report.html -x "
  SELECT d.\"SQL Text\", s.\"Total Elapse Time(us)\", s.\"Calls\", s.\"CPU Time(us)\"
  FROM \"wdr_report.This table displays SQL Text\" d
  JOIN \"wdr_report.This table displays SQL ordered by Elapsed Time\" s
    ON d.\"Unique SQL Id\" = s.\"Unique SQL Id\"
  WHERE s.\"Total Elapse Time(us)\" > 10000
  ORDER BY s.\"Total Elapse Time(us)\" DESC
  LIMIT 10
"
```

### Step 5: Compare Two Snapshots

Import both WDR snapshots and JOIN to find regressions:

```bash
grep_excel snap_101.html snap_102.html -x "
  SELECT a.\"Metric\",
         a.\"Per Second\" AS snap1,
         b.\"Per Second\" AS snap2,
         CAST(b.\"Per Second\" AS DOUBLE) - CAST(a.\"Per Second\" AS DOUBLE) AS delta
  FROM \"snap_101.This table displays Load Profile\" a
  JOIN \"snap_102.This table displays Load Profile\" b ON a.\"Metric\" = b.\"Metric\"
  WHERE ABS(CAST(b.\"Per Second\" AS DOUBLE) - CAST(a.\"Per Second\" AS DOUBLE)) > 10
  ORDER BY delta DESC
"
```

### Step 6: Export Analysis Results

```bash
grep_excel wdr_report.html \
  -x "SELECT * FROM \"wdr_report.This table displays Load Profile\" WHERE \"Metric\" ILIKE '%DB%' OR \"Metric\" ILIKE '%CPU%'" \
  -e load_profile_analysis.csv
```

## MCP Workflow (for AI Assistants)

When using grep-excel as an MCP server (`grep_excel --mcp`), follow this exploration sequence:

```
import_file → get_metadata → get_sheet_sample → search/execute_sql → export_query
```

Key MCP tools for AWR/WDR analysis:

| Tool | Use |
|------|-----|
| `import_file` | Load the AWR/WDR HTML report |
| `get_metadata` | List all extracted sheets + column names |
| `get_sheet_sample` | Preview any sheet with evenly-spaced sampling (fast) |
| `search` | Fulltext/exact/wildcard/regex search with aggregation |
| `execute_sql` | Cross-table JOINs, window functions, complex filters |
| `materialize_query` | Cache intermediate results as temp tables |
| `get_sheet_statistics` | Null/distinct counts, top values per column |
| `export_query` | Export SQL results to `.xlsx` |

### MCP Search Examples

```json
// Find SQL statements with table scans
{"tool":"search","params":{"query":"SELECT.*FROM","mode":"regex","sheet":"This table displays SQL ordered by Elapsed Time","limit":20}}

// Aggregate wait events by type
{"tool":"search","params":{"query":"buffer","mode":"fulltext","aggregate":"Event"}}

// Multi-condition filter: log-related waits with > 5% DB time
{"tool":"search","params":{"query":"log","mode":"fulltext","conditions":[{"column":"% DB time","operator":">","value":"5"}]}}
```

## Key WDR Tables Reference

A typical openGauss WDR report contains these sections (sheet names from `summary` attribute):

### Summary Tables
| Sheet | Key Columns |
|-------|-------------|
| `This table displays Load Profile` | Metric, Per Second, Per Transaction, Per Exec |
| `This table displays Instance Efficiency Percentages (Target 100%)` | Metric Name, Metric Value |
| `This table displays Database Stat` | DB Name, Backends, Xact Commit, Blks Read, Blks Hit |
| `This table displays IO Profile` | Metric, Read+Write Per Sec, Read Per Sec, Write Per Sec |

### SQL Statistics (7 sort orders, 26 columns)
| Sheet | Sort Key |
|-------|----------|
| `This table displays SQL ordered by Elapsed Time` | Total Elapse Time(us) DESC |
| `This table displays SQL ordered by CPU Time` | CPU Time(us) DESC |
| `This table displays SQL ordered by Rows Returned` | Returned Rows DESC |
| `This table displays SQL ordered by Row Read` | Tuples Read DESC |
| `This table displays SQL ordered by Executions` | Calls DESC |
| `This table displays SQL ordered by Physical Reads` | Physical Read DESC |
| `This table displays SQL ordered by Logical Reads` | Logical Read DESC |

Common columns across all SQL tables: `Unique SQL Id`, `Node Name`, `User Name`, `Total Elapse Time(us)`, `Calls`, `CPU Time(us)`, `Logical Read`, `Physical Read`, `SQL Text`.

### Object IO Activity (8 tables)
User table/index IO activity ordered by heap hit ratio, blocks read, blocks hit for tables, indexes, TOAST, and TOAST indexes.

### Object Stats (3 tables)
`User Tables stats`, `User Index stats`, `Bad lock stats` — vacuum/analyze info, dead tuples, lock conflicts.

### SQL Detail
`This table displays SQL Text` — full SQL text linked via `Unique SQL Id`. Use JOIN with SQL statistics tables for complete analysis.

## AWR Report Notes

AWR reports follow a similar structure. Key AWR tables include:
- **Load Profile** — DB Time, CPU, Redo size, Logical/Physical reads
- **Instance Efficiency Percentages** — Buffer Hit%, Library Hit%, Soft Parse%
- **Top 5 Timed Foreground Events** — Wait events ranked by DB time
- **SQL ordered by Elapsed Time / CPU Time / Gets / Reads / Executions**
- **Wait Event Histogram** — Wait distribution by latency bucket
- **IO Stats by tablespace / file** — Tablespace-level IO metrics
- **Buffer Pool Advisory / PGA Memory Advisory** — Sizing recommendations

AWR also supports Markdown (`.md`) and plain text (`.txt`) formats — both parsed into the same table structure.

## Tips

- **Use DuckDB**: DuckDB enables `ILIKE`, `regexp_matches()`, window functions, and `::` type casts — critical for report analysis. The in-memory engine supports basic SQL only.
- **Friendly aliases**: Run `-t` first to discover table aliases. Use `"filename.Sheet Name"` (double-quoted when names contain spaces).
- **Materialize for multi-step**: Use `.let` (REPL) or `materialize_query` (MCP) to cache JOIN results when comparing multiple sections.
- **Context lines**: The `search` MCP tool supports `context_lines` (grep `-C` style) to show surrounding rows.
- **WDR v1 vs v2**: v1 splits SQL text into a separate lookup table; v2 may include SQL text inline. Always check with `get_metadata` first.

## See Also

- [Analyzing Database Exports](./analyzing-database-exports.md) — import multi-table DB dumps and JOIN
- [Using with AI Assistants](./using-with-ai-assistants.md) — MCP server setup and configuration
- [Batch Processing Pipelines](./batch-processing-pipelines.md) — automate multi-report analysis with `--exec`
- [User Guide](../docs/UserGuide.md) — full grep-excel documentation
