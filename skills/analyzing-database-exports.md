# Skill: Analyzing Database Exports

Learn how to use grep-excel to analyze multi-table database dumps, CSV exports, and other tabular data dumps — without a database server.

## Prerequisites

- grep-excel with `--features full` (DuckDB engine recommended for analytical queries)
- One or more tabular data files (CSV, TSV, Excel, etc.)

## Workflow

### Step 1: Import All Files

grep-excel imports multiple files in one command and makes them available for cross-table queries:

```bash
grep_excel orders.csv customers.csv products.xlsx -t
```

The `--list-tables` / `-t` flag shows the friendly alias for each imported table:

```
Available tables:
  orders.Sheet1    → sheet_1_0 (1500 rows) [OrderID, CustomerID, ProductID, Amount, Date]
  customers.Sheet1 → sheet_2_0 (200 rows)  [CustomerID, Name, City, Segment]
  products.Sheet1  → sheet_3_0 (50 rows)   [ProductID, Name, Category, Price]
```

### Step 2: Explore Data with SQL

Use friendly aliases (`filename.SheetName`) instead of internal names (`sheet_1_0`):

```bash
# Preview each table
grep_excel orders.csv -x "SELECT * FROM orders.Sheet1 LIMIT 5"
grep_excel customers.csv -x "SELECT * FROM customers.Sheet1 LIMIT 5"
```

### Step 3: JOIN Across Files

Cross-table analysis with standard SQL JOINs:

```bash
# Total revenue per customer
grep_excel orders.csv customers.csv -x "
  SELECT c.Name, c.City, SUM(o.Amount) AS total_spent
  FROM orders.Sheet1 o
  JOIN customers.Sheet1 c ON o.CustomerID = c.CustomerID
  GROUP BY c.Name, c.City
  ORDER BY total_spent DESC
  LIMIT 20
"
```

### Step 4: Aggregation & Analytics

DuckDB enables full analytical SQL — window functions, CTEs, and more:

```bash
# Revenue by product category
grep_excel orders.csv products.xlsx -x "
  SELECT p.Category, COUNT(*) AS order_count, SUM(o.Amount) AS revenue
  FROM orders.Sheet1 o
  JOIN products.Sheet1 p ON o.ProductID = p.ProductID
  GROUP BY p.Category
  ORDER BY revenue DESC
"
```

```bash
# Top customers by segment (with ranking)
grep_excel orders.csv customers.csv -x "
  SELECT c.Segment, c.Name, SUM(o.Amount) AS revenue,
         RANK() OVER (PARTITION BY c.Segment ORDER BY SUM(o.Amount) DESC) AS rank
  FROM orders.Sheet1 o
  JOIN customers.Sheet1 c ON o.CustomerID = c.CustomerID
  GROUP BY c.Segment, c.Name
  QUALIFY rank <= 3
  ORDER BY c.Segment, rank
"
```

### Step 5: Export Results

Save results for reporting or further processing:

```bash
# Export to CSV
grep_excel orders.csv customers.csv \
  -x "SELECT c.City, COUNT(*) FROM orders.Sheet1 o JOIN customers.Sheet1 c ON o.CustomerID = c.CustomerID GROUP BY c.City" \
  -e city_orders.csv

# Export with specific format
grep_excel orders.csv -x "SELECT * FROM orders.Sheet1 WHERE Amount > 1000" \
  -e high_value.json -f json
```

### REPL Workflow (Interactive)

For iterative analysis, use the SQL REPL:

```bash
grep_excel orders.csv customers.csv products.xlsx -i
```

```
$ .tables
orders.Sheet1    → sheet_1_0 (1500 rows)
customers.Sheet1 → sheet_2_0 (200 rows)
products.Sheet1  → sheet_3_0 (50 rows)

$ .let summary AS SELECT c.City, COUNT(*) AS n, SUM(o.Amount) AS total
  FROM orders.Sheet1 o JOIN customers.Sheet1 c ON o.CustomerID = c.CustomerID
  GROUP BY c.City;

$ SELECT * FROM summary WHERE n > 10 ORDER BY total DESC;
$ .save city_summary.csv
$ .drop summary
$ .exit
```

## Tips

- **DuckDB for analytics**: DuckDB (`--features engine-duckdb`) provides window functions, CTEs, `::` casts, and `ILIKE` — ideal for data analysis. The in-memory engine works for simple searches but lacks advanced SQL.
- **Friendly aliases**: Always use `filename.SheetName` syntax — run `-t` first to discover available aliases.
- **Materialize intermediate results**: Use `.let` (REPL) or `materialize_query` (MCP) to cache expensive JOINs for multi-step analysis.
- **Large files**: DuckDB handles millions of rows efficiently. Use `get_sheet_sample` (MCP) or `SELECT ... LIMIT n` while exploring.

## See Also

- [Batch Processing Pipelines](./batch-processing-pipelines.md) — automate multi-step ETL
- [Working with Archives](./working-with-archives.md) — analyze data inside archives
- [SQL Query Guide](../docs/UserGuide.md#sql-query-guide) — detailed SQL reference
