# grep-excel

TUI tool for searching Excel/CSV files with DuckDB-powered performance.

grep-excel provides a fast, interactive terminal interface for searching across multiple Excel and CSV files. It uses DuckDB as the query engine for high-performance full-text search, exact matching, wildcard pattern matching, and regex search.

## Features

- Full-text search across all imported Excel/CSV files
- Exact match mode for precise string comparisons
- Wildcard pattern matching with `%` and `_` operators
- Regex mode for multi-keyword OR search (e.g. `keyword1|keyword2`)
- Column filtering to narrow search scope
- Native file picker dialog (press `o` in TUI, requires `file-dialog` feature)
- Matched cells highlighted in green in search results
- Multi-file support - import and search across multiple spreadsheets
- DuckDB backend for high-performance in-memory queries
- Cross-platform: Windows, macOS (Intel/Apple Silicon), and Linux (x64/ARM64)
- Terminal UI built with ratatui for smooth keyboard-driven navigation

## Installation

### Download Prebuilt Binaries

Download the latest release for your platform from the [GitHub Releases page](https://github.com/c2j/grep-excel/releases).

Available targets:
- Windows (x86_64)
- macOS (Intel x86_64, Apple Silicon ARM64)
- Linux (x86_64, ARM64)

### Build from Source

```bash
git clone https://github.com/c2j/grep-excel.git
cd grep-excel
cargo build --release
```

The binary will be available at `target/release/grep_excel`.

> **Note:** Set `DUCKDB_DOWNLOAD_LIB=1` when building to download pre-built DuckDB libraries instead of compiling from source, which significantly speeds up the build process.

## Usage

### Command Line

```bash
grep_excel [FILES...] [OPTIONS]
```

### CLI Options

| Flag | Short | Description |
|------|-------|-------------|
| `--query` | `-q` | Search query string |
| `--column` | `-c` | Filter to specific column name |
| `--mode` | `-m` | Search mode: `fulltext` (default), `exact`, `wildcard`, or `regex` |

### Examples

Search a single file:
```bash
grep_excel data.xlsx -q "search term"
```

Search multiple files with column filter:
```bash
grep_excel a.xlsx b.xlsx -c "Name" -m exact
```

Wildcard search (use `%` for any characters, `_` for single character):
```bash
grep_excel data.xlsx -q "Jo%" -m wildcard
```

Regex multi-keyword search (use `|` for OR):
```bash
grep_excel data.xlsx -q "张三|李四" -m regex
```

Launch in TUI mode (no CLI arguments):
```bash
grep_excel
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `/` | Enter search query |
| `c` | Set column filter |
| `Tab` | Cycle through search modes (fulltext / exact / wildcard / regex) |
| `Enter` | Execute search |
| `o` | Open file picker (with `file-dialog` feature) or view loaded files |
| `?` | Show help |
| `j` / `k` | Navigate results down/up |
| `g` | Jump to top of results |
| `G` | Jump to bottom of results |
| `d` | Clear search results |

## Supported Formats

grep-excel can read the following formats:

- `.xlsx` - Excel 2007+ (Open XML)
- `.xls` - Excel 97-2004 (BIFF8)
- `.xlsm` - Excel Macro-Enabled
- `.xlsb` - Excel Binary
- `.ods` - OpenDocument Spreadsheet
- `.csv` - Comma-Separated Values

## Build from Source

Requirements:
- Rust 1.70+ and Cargo

```bash
# Clone the repository
git clone https://github.com/c2j/grep-excel.git
cd grep-excel

# Build release binary (with native file dialog)
cargo build --release

# Build without file dialog (for headless environments)
cargo build --release --no-default-features

# Or with faster DuckDB library download
DUCKDB_DOWNLOAD_LIB=1 cargo build --release
```

> **Note:** `DUCKDB_DOWNLOAD_LIB=1` downloads pre-built DuckDB libraries instead of compiling from source, which significantly speeds up the build process.

## License

MIT License - see [LICENSE](LICENSE) for details.
