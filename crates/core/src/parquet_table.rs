//! Parquet columnar format reader.
//!
//! Reads Parquet files via the Apache Arrow `parquet` crate and projects all
//! columns to UTF-8 strings (the existing `SheetData` cell type). Native type
//! fidelity is preserved when the DuckDB engine is active via `read_parquet()`
//! direct path in `engine/duckdb.rs`; this parser is the fallback for memory
//! and sqlite engines where analytical SQL is not the primary use case.

use crate::excel::SheetData;
use std::path::Path;

#[cfg(feature = "parquet-support")]
pub fn parse_parquet(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    use anyhow::Context;
    use parquet::file::reader::FileReader;
    use parquet::file::serialized_reader::SerializedFileReader;

    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open parquet file: {}", path.display()))?;
    let reader = SerializedFileReader::new(file)
        .with_context(|| format!("failed to read parquet metadata: {}", path.display()))?;

    let schema = reader.metadata().file_metadata().schema_descr();
    let num_cols = schema.num_columns();
    let column_names: Vec<String> = (0..num_cols)
        .map(|i| schema.column(i).path().string())
        .collect();

    let sheet_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Sheet1")
        .to_string();

    let mut rows: Vec<Vec<String>> = Vec::new();
    for row_result in reader.into_iter() {
        let row = row_result.map_err(|e| anyhow::anyhow!("parquet row read error: {}", e))?;
        let mut cells: Vec<String> = Vec::with_capacity(num_cols);
        for (_name, col_reader) in row.get_column_iter() {
            let cell_str = parquet_value_to_string(col_reader);
            cells.push(cell_str.unwrap_or_default());
        }
        // Pad if column count mismatches (defensive — should not happen for valid files)
        while cells.len() < num_cols {
            cells.push(String::new());
        }
        rows.push(cells);
    }

    if rows.is_empty() {
        anyhow::bail!(
            "{}",
            crate::i18n::parquet_no_data(path.display().to_string().as_str())
        );
    }

    Ok(vec![SheetData {
        name: sheet_name,
        headers: column_names,
        rows,
        col_widths: vec![],
    }])
}

/// Convert a parquet column value to its display string.
///
/// Nested/complex types (LIST, MAP, STRUCT) are flattened to a best-effort
/// debug representation. This parser targets flat tabular Parquet schemas; complex
/// nested types are out of scope and documented as a limitation.
#[cfg(feature = "parquet-support")]
fn parquet_value_to_string(value: &parquet::record::Field) -> Option<String> {
    use parquet::record::Field;
    match value {
        Field::Null => None,
        Field::Bool(b) => Some(b.to_string()),
        Field::Byte(n) => Some(n.to_string()),
        Field::Short(n) => Some(n.to_string()),
        Field::Int(n) => Some(n.to_string()),
        Field::Long(n) => Some(n.to_string()),
        Field::UByte(n) => Some(n.to_string()),
        Field::UShort(n) => Some(n.to_string()),
        Field::UInt(n) => Some(n.to_string()),
        Field::ULong(n) => Some(n.to_string()),
        Field::Float16(f) => Some(f.to_string()),
        Field::Float(f) => Some(f.to_string()),
        Field::Double(d) => Some(d.to_string()),
        Field::Decimal(d) => Some(format!("{:?}", d)),
        Field::Str(s) => Some(s.clone()),
        Field::Bytes(b) => Some(format!("{:?}", b)), // best-effort
        Field::Date(n) => Some(n.to_string()),
        Field::TimestampMillis(n) => Some(n.to_string()),
        Field::TimestampMicros(n) => Some(n.to_string()),
        // Nested types: out of scope, best-effort debug representation
        Field::Group(_) => Some(format!("{:?}", value)),
        Field::ListInternal(_) => Some(format!("{:?}", value)),
        Field::MapInternal(_) => Some(format!("{:?}", value)),
    }
}

#[cfg(not(feature = "parquet-support"))]
pub fn parse_parquet(path: &Path) -> anyhow::Result<Vec<SheetData>> {
    #[allow(unused_variables)]
    let _ = path;
    anyhow::bail!("{}", crate::i18n::parquet_not_enabled())
}
