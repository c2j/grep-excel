use std::path::Path;

/// File format identified by extension.
///
/// Used as a zero-cost dispatch tag. `Copy` ensures no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Excel / ODS formats (calamine): .xlsx .xls .xlsm .xlsb .ods
    Excel,
    /// Comma-separated values (csv crate): .csv
    Csv,
    /// Tab-separated values (csv crate, delimiter=b'\t'): .tsv .tab
    Tsv,
    /// HTML tables (scraper): .html .htm
    Html,
    /// Plain-text heuristic tables: .txt
    Text,
    /// Markdown pipe tables: .md .markdown
    Markdown,
    /// dBase database files: .dbf
    Dbf,
    /// XML data files: .xml
    Xml,
}

impl FileFormat {
    /// Detect format from file extension. Returns `None` for unknown extensions
    /// (caller should fall back to calamine as a last resort).
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "csv" => Some(Self::Csv),
            "tsv" | "tab" => Some(Self::Tsv),
            "html" | "htm" => Some(Self::Html),
            "txt" => Some(Self::Text),
            "md" | "markdown" => Some(Self::Markdown),
            "dbf" => Some(Self::Dbf),
            "xml" => Some(Self::Xml),
            "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => Some(Self::Excel),
            _ => None,
        }
    }

    /// All extensions recognized as table files (for archive filtering).
    /// Derived at compile time from the same extension→format mapping.
    pub const TABLE_EXTENSIONS: &[&str] = &[
        "xlsx", "xls", "xlsm", "xlsb", "ods",
        "csv", "tsv", "tab",
        "html", "htm",
        "txt", "md", "markdown",
        "dbf",
        "xml",
    ];
}
