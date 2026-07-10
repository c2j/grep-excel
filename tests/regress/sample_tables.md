# Sample Tables for Regression Testing

This file contains various pipe tables to test the markdown table extraction feature.

## Basic Table

| Name  | Age | City   |
|-------|-----|--------|
| Alice | 30  | NYC    |
| Bob   | 25  | SF     |

## Alignment

| Left | Center | Right |
|:-----|:------:|------:|
| A    | B      | C     |
| D    | E      | F     |

## No Separator Line

| H1 | H2 |
| A  | B  |
| C  | D  |

## Empty Cells

| Col1 | Col2 | Col3 |
|------|------|------|
| a    |      | c    |
|      | b    |      |

## Single Column

| X |
|---|
| 1 |
| 2 |

## Multiple Tables Consecutive (No Text Between)

| A | B |
|---|---|
| 1 | 2 |

| C | D |
|---|---|
| 3 | 4 |

## Section Heading Naming

### Performance Metrics

| Metric    | Value |
|-----------|-------|
| CPU Usage | 45%   |
| Memory    | 8GB   |

### IO Statistics

| Device | Reads | Writes |
|--------|-------|--------|
| sda    | 1000  | 500    |
| sdb    | 2000  | 800    |

## Table Inside Code Block (Should Be Skipped)

```markdown
| This | Should | Not |
|------|--------|-----|
| be   | extracted | ! |
```

## Table After Code Block (Should Be Extracted)

| Key | Value |
|-----|-------|
| foo | bar   |

## Prose Without Table

This is just a paragraph with some random text.
There are no tables here at all.

## Ragged Columns

| H1 | H2 | H3 |
|----|----|----|
| a  | b  |
| c  | d  | e  | f |
| g  |

## Table With Leading/Trailing Pipe Trim

| X | Y | Z |
|---|---|---|
| 1 | 2 | 3 |
