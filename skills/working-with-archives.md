# Skill: Working with Archives

Search and analyze table files inside compressed archives without manual decompression — grep-excel handles extraction transparently.

## Prerequisites

- grep-excel with `--features archive-support` (included in `full`)

## Supported Archive Formats

| Format | Notes |
|--------|-------|
| `.zip` | Standard ZIP archives |
| `.tar` | Uncompressed TAR |
| `.tar.gz` / `.tgz` | Gzip-compressed TAR |
| `.tar.bz2` | Bzip2-compressed TAR |
| `.tar.xz` | XZ-compressed TAR |
| `.tar.zst` | Zstd-compressed TAR |
| `.zip.001` / `.zip.002` | Multi-volume split ZIP (pass `.zip.001` only) |

## How It Works

When you pass an archive file, grep-excel:

1. Opens the archive and enumerates all entries
2. Identifies entries with recognizable table file extensions (`.xlsx`, `.csv`, `.tsv`, etc.)
3. Extracts each matching entry into memory
4. Imports each as a separate entry named `archive::path/file.xlsx`

All of this happens transparently — you use the same commands as you would with regular files.

## Basic Usage

### List Contents

```bash
# See what's inside the archive
grep_excel audit_2026.zip -t
```

Output shows entries with their archive paths:

```
Available tables:
  archive::2026/Q1/sales.xlsx → sheet_1_0 (500 rows)
  archive::2026/Q1/returns.csv → sheet_2_0 (45 rows)
  archive::2026/Q2/sales.xlsx → sheet_3_0 (520 rows)
  archive::2026/Q2/returns.csv → sheet_4_0 (38 rows)
```

### Search Inside Archives

```bash
# Search all files inside the archive at once
grep_excel audit_2026.zip -q "anomaly"

# Filter to a specific sheet
grep_excel audit_2026.zip -q "anomaly" -s "sheet_1_0"
```

### SQL Queries on Archived Data

```bash
# Query specific files within the archive
grep_excel audit_2026.zip -x "SELECT * FROM sheet_1_0 LIMIT 10"

# Cross-file analysis within the archive
grep_excel quarterly_data.tar.gz -x "
  SELECT 'Q1' AS quarter, SUM(Amount) FROM sheet_1_0
  UNION ALL
  SELECT 'Q2' AS quarter, SUM(Amount) FROM sheet_3_0
"
```

## Working with Split Volumes

Multi-volume ZIP archives (`.zip.001`, `.zip.002`, ...) are common for large datasets.
Pass only the first volume:

```bash
grep_excel big_data.zip.001 -t
grep_excel big_data.zip.001 -q "keyword"
grep_excel big_data.zip.001 -x "SELECT COUNT(*) FROM sheet_1_0"
```

grep-excel automatically detects the split archive and reads consecutive volumes.

## Real Examples

### Audit Log Analysis

```bash
# Monthly audit logs archived as ZIP
grep_excel audit_2026.zip -x "
  SELECT
    CASE
      WHEN Amount > 100000 THEN 'High'
      WHEN Amount > 10000 THEN 'Medium'
      ELSE 'Low'
    END AS risk_level,
    COUNT(*) AS count
  FROM sheet_1_0
  GROUP BY risk_level
  ORDER BY count DESC
"
```

### Multi-Year Financial Data

```bash
# Multiple annual archives — import and compare
grep_excel fy2024.tar.gz fy2025.tar.gz -x "
  SELECT 'FY2024' AS year, SUM(Revenue) FROM sheet_1_0
  UNION ALL
  SELECT 'FY2025' AS year, SUM(Revenue) FROM sheet_2_0
"
```

### Database Backup Dump

```bash
# Database export as tar.gz with multiple CSV tables
grep_excel db_backup.tar.gz -t
# Shows: archive::users.csv → sheet_1_0, archive::orders.csv → sheet_2_0, ...

grep_excel db_backup.tar.gz -x "
  SELECT u.Name, COUNT(o.OrderID) AS order_count
  FROM sheet_1_0 u
  JOIN sheet_2_0 o ON u.UserID = o.UserID
  GROUP BY u.Name
  ORDER BY order_count DESC
  LIMIT 20
"
```

## Performance Considerations

- Extraction is done **in memory** — archives are not written to disk
- Memory usage is proportional to the size of extracted files, not the archive itself
- Very large archives (several GB) may need more RAM; split volumes help manage this
- DuckDB engine (`engine-duckdb`) handles large extracted datasets efficiently

## Tips

- **Archive first, search later**: Pass the archive directly — no need to decompress first
- **Use `-t` to explore**: Always `-t` first to understand what tables are available inside
- **Archive naming**: Entries use `archive::path/filename` naming. Use `-t` to discover the exact names
- **Filter by name**: Use `-s` to target specific sheets within the archive
- **Split volumes**: Only the `.zip.001` file is needed — grep-excel reads the rest automatically

## See Also

- [Analyzing Database Exports](./analyzing-database-exports.md) — analyze archived database dumps
- [Batch Processing Pipelines](./batch-processing-pipelines.md) — automate archive processing
- [User Guide: Archives](../docs/UserGuide.md#archives--cloud-share-url-import) — detailed archive documentation
