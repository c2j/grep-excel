# Skill: Debugging & Repairing Corrupted Files

Recover data from damaged or unreadable Excel files using grep-excel's built-in repair mode.

## Prerequisites

- grep-excel with any engine backend (repair works with all engines)

## What `--repair` Does

`--repair` (`-r`) operates at the ZIP/XML level to recover data from corrupted `.xlsx` files:

1. Attempts to open the file as a ZIP archive and repair ZIP structure
2. Extracts XML data from individual sheet parts within the archive
3. Parses recoverable cell values from the XML, ignoring corrupted sections
4. Applies date auto-detection to recovered data

This can recover data from files that:
- Fail to open in Excel ("File is corrupted and cannot be opened")
- Have truncated data (partially downloaded files)
- Have minor ZIP structure corruption
- Have corrupt styles or formatting (data cells are often intact)

## When to Use

### Scenario 1: File won't open at all

```bash
grep_excel corrupted.xlsx -q "keyword" -r
```

If the file fails to import normally, grep-excel **automatically** triggers repair mode.
You can also force it explicitly with `-r`.

### Scenario 2: Import succeeds but data looks wrong

Some files partially import but with garbled values or missing sheets. In this case,
try the explicit repair:

```bash
grep_excel partial.xlsx -r -t
```

Compare the repaired output against a normal import to see what was recovered.

### Scenario 3: Unknown file — let auto-detection work

```bash
grep_excel mystery.xlsx -q "important data"
```

grep-excel first tries a normal import. If that fails, it falls back to `--repair`
automatically. No need to decide upfront.

## Step-by-Step Repair Workflow

### 1. Diagnose

```bash
# Try listing tables normally
grep_excel corrupted.xlsx -t
# If this fails: "Error: Failed to import..." — proceed to repair

# Try with explicit repair
grep_excel corrupted.xlsx -t -r
# Output shows recovered sheets and row counts
```

### 2. Search & Explore

```bash
# Search for known values to verify recovery
grep_excel corrupted.xlsx -q "expected_value" -r

# Preview recovered data
grep_excel corrupted.xlsx -r -x "SELECT * FROM sheet_1_0 LIMIT 20"
```

### 3. Export Recovered Data

```bash
# Save recovered data to a clean file
grep_excel corrupted.xlsx -r -x "SELECT * FROM sheet_1_0" -e recovered.csv
```

## What `--repair` Cannot Fix

- **Encrypted files**: Files with password protection cannot be repaired
- **Completely overwritten data**: If the XML parts themselves are destroyed, there is nothing to recover
- **Binary format corruption (`.xls`)**: Repair targets the ZIP+XML structure of `.xlsx`; older binary `.xls` files are not covered
- **Intentional data removal**: Deleted rows/columns that are no longer in the XML

## Date Recovery

Repair mode preserves grep-excel's date auto-detection:

- Excel date serial numbers (e.g. `44927`) are converted to `2023-01-15` automatically
- Time values are preserved (`2023-01-15 09:30:00`)
- Numeric columns (IDs, amounts) are not falsely converted

## Tips

- **Try auto-detection first**: Let grep-excel decide — it auto-falls-back to repair on failure
- **Export immediately**: Recovered data may be incomplete; save it to a clean format right away
- **Multiple attempts**: If one sheet fails, try importing sheets individually via SQL `SELECT` with `--repair`
- **Check row counts**: Compare the repaired row count against what you expect — missing rows indicate unrecoverable corruption

## See Also

- [Analyzing Database Exports](./analyzing-database-exports.md) — analyze recovered data
- [Working with Archives](./working-with-archives.md) — repair files inside archives
