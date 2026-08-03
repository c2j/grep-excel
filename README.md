[中文版](README.zh-CN.md)

# grep-excel

**grep, but for spreadsheets.** Search Excel, CSV, HTML reports, Word docs, ZIP archives — as if they were tabular text. And when you need more power, just write SQL.

I spend a lot of time digging through multi-sheet Excel files and database diagnostic reports (AWR, WDR — the kind of HTML files with dozens of tables each). Searching for a keyword meant clicking through every sheet. Got tired of it. So I built this: throw any format at it, search instantly, and when you need real analysis, DuckDB is under the hood — handles millions of rows without breaking a sweat.

## What it does

- **Search spreadsheets like text.** Fulltext, exact match, wildcard, regex — `grep_excel data.xlsx -q "keyword"` and you're done.
- **SQL straight on your data.** Imported tables become database tables. JOINs, GROUP BY, window functions — whatever you need. DuckDB engine, way more analytical firepower than SQLite.
- **Eats every format.** Excel / CSV / TSV / HTML / Markdown / Word / PowerPoint / DBF / XML / PDF / Parquet — it knows them all. ZIP and tar.gz archives? Drop them in, tables get extracted automatically.
- **TUI for keyboard people.** Import and auto-browse, tabbed results, detail panel, flat/table views. No mouse required.
- **MCP for AI assistants.** Fire up `--mcp` and let Claude or Cursor search, analyze, edit, and export your data directly.
- **Chinese-friendly.** Auto-detects language from your system locale. Handles GBK-encoded files with CJK fallback — no manual transcoding needed.

## What it's not

- Not an Excel replacement. If you need charts, pivot tables, or conditional formatting, stick with Excel or WPS.
- Not an OCR tool. PDF support means text-based tables — scanned documents won't work.
- Not a database. Once you're past a hundred million rows, import into a real database.
- Performance depends on your engine. The default in-memory engine is fine for most things. DuckDB is faster, but compiling it is a pain (more on that below).

## What it looks like

Just run `grep_excel` with no arguments to drop into TUI mode:

![TUI screenshot](https://github.com/c2j/grep-excel/raw/main/docs/tui.png)

## Installation

### Download binaries

Grab the right archive from the [Releases](https://github.com/c2j/grep-excel/releases) page:

- **Windows** — `grep_excel-windows-x86_64.zip`
- **macOS Intel** — `grep_excel-macos-x86_64.zip`
- **macOS Apple Silicon** — `grep_excel-macos-aarch64.zip`
- **Linux x86_64** — `grep_excel-linux-x86_64.zip`
- **Linux ARM64** — `grep_excel-linux-aarch64.zip`

Unzip and run. That's it.

### Build from source

Requires Rust 1.70+ and Cargo.

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# Default build (in-memory engine, fast)
cargo build --release

# Recommended: full-featured build (DuckDB + MCP + archive support)
cargo build --release --features full
```

> DuckDB takes a while to compile. Set `DUCKDB_DOWNLOAD_LIB=1` to download a prebuilt library — much faster.

For other feature combos (SQLite engine, headless build, etc.), see [CONTRIBUTING.md](CONTRIBUTING.md).

## Usage

### The basics

```bash
# Search one file
grep_excel data.xlsx -q "keyword"

# Search multiple files, filter by column
grep_excel a.xlsx b.xlsx -c Name -m exact

# Invert: find rows that do NOT match
grep_excel data.xlsx -q "normal" -v
```

### Real-world examples

```bash
# Your boss sends sales-2026.zip with 50 Excel files inside.
# Find every row mentioning "Refund" and break it down by department:
grep_excel sales-2026.zip -q Refund -g Department

# Check CPU load in an AWR report
grep_excel awr_report.html -x "SELECT * FROM \"CPU Load\" WHERE \"%Busy\" > 80"

# Export aggregated results
grep_excel national_sales.xlsx -q East -g City -e east_stats.csv

# Run an external script on each matching row
grep_excel data.xlsx -q ERROR -c Level --run './analyzer "${Message}"'
```

### TUI mode

Launch without arguments:

```bash
grep_excel
```

Imported files auto-browse the first sheet. Essential keys:

| Key | Action |
|-----|--------|
| `/` or `e` | Enter search query |
| `S` | SQL query mode |
| `Tab` | Cycle search mode |
| `v` | Toggle flat/table view |
| `Ctrl+←` / `Ctrl+→` | Switch sheet |
| `Ctrl+↑` / `Ctrl+↓` | Switch file |
| `?` | Help |

Full keybindings: [UserGuide](docs/UserGuide.md).

### SQL REPL

```bash
grep_excel data.xlsx employees.xlsx -i
```

Drops you into an interactive SQL shell with dot-commands (`.tables`, `.files`, `.output`, `.save`, etc.). Command history is saved between sessions.

### MCP mode

```bash
grep_excel --mcp
```

Add to your AI assistant's MCP config:

```json
{
  "mcpServers": {
    "grep-excel": {
      "command": "/path/to/grep_excel",
      "args": ["--mcp"]
    }
  }
}
```

Now Claude or Cursor can search, analyze, edit cells, and export — directly from conversation. See [UserGuide](docs/UserGuide.md) for full workflow examples.

## Supported formats

| Format | Extension | Editable |
|--------|-----------|----------|
| Excel 2007+ | `.xlsx` | ✅ |
| Excel 97-2004 | `.xls` | ✅ |
| CSV / TSV | `.csv` `.tsv` `.tab` | ✅ |
| HTML tables | `.html` `.htm` | ✅ |
| Markdown pipe tables | `.md` | ✅ |
| Plain-text tables | `.txt` | ✅ |
| Word docs | `.docx` | ❌ read-only |
| PowerPoint | `.pptx` | ❌ read-only |
| dBase database | `.dbf` | ✅ |
| XML data | `.xml` | ✅ |
| PDF | `.pdf` | ❌ read-only |
| Parquet | `.parquet` | ❌ read-only |
| ZIP / TAR archives | `.zip` `.tar.gz` etc. | — auto-extracted |

## More

- [UserGuide.md](docs/UserGuide.md) — full usage manual
- [DeveloperGuide.md](docs/DeveloperGuide.md) — architecture, SearchEngine trait, adding new engines
- [CONTRIBUTING.md](CONTRIBUTING.md) — contribution guide, PR checklist, feature flags

## License

MIT License — see [LICENSE](LICENSE)
