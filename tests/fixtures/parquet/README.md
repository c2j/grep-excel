# Parquet test fixtures

- `sample.parquet` — generated from pyarrow, 3 rows × 4 columns (int, string, double, bool).
  Regenerate with: `python3 -c "import pyarrow as pa, pyarrow.parquet as pq; pq.write_table(pa.table({'id':[1,2,3],'name':['Alice','Bob','Charlie'],'score':[95.5,87.2,92.8],'active':[True,False,True]}), 'sample.parquet')"
