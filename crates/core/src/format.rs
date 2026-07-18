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
            .unwrap_or("");

        if ext.eq_ignore_ascii_case("csv") {
            Some(Self::Csv)
        } else if ext.eq_ignore_ascii_case("tsv") || ext.eq_ignore_ascii_case("tab") {
            Some(Self::Tsv)
        } else if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") {
            Some(Self::Html)
        } else if ext.eq_ignore_ascii_case("txt") {
            Some(Self::Text)
        } else if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
            Some(Self::Markdown)
        } else if ext.eq_ignore_ascii_case("dbf") {
            Some(Self::Dbf)
        } else if ext.eq_ignore_ascii_case("xml") {
            Some(Self::Xml)
        } else if ext.eq_ignore_ascii_case("xlsx")
            || ext.eq_ignore_ascii_case("xls")
            || ext.eq_ignore_ascii_case("xlsm")
            || ext.eq_ignore_ascii_case("xlsb")
            || ext.eq_ignore_ascii_case("ods")
        {
            Some(Self::Excel)
        } else {
            None
        }
    }

    /// Parse format from a CLI string (for --as flag).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "tsv" | "tab" => Some(Self::Tsv),
            "html" | "htm" => Some(Self::Html),
            "txt" | "text" => Some(Self::Text),
            "md" | "markdown" => Some(Self::Markdown),
            "dbf" => Some(Self::Dbf),
            "xml" => Some(Self::Xml),
            "excel" | "xlsx" | "xls" => Some(Self::Excel),
            _ => None,
        }
    }

    /// Human-readable names accepted by `from_name()`, for help text.
    pub const ALL_NAMES: &[&str] = &[
        "csv", "tsv", "html", "txt", "md", "dbf", "xml", "excel",
    ];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_valid() {
        assert_eq!(FileFormat::from_name("csv"), Some(FileFormat::Csv));
        assert_eq!(FileFormat::from_name("TSV"), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_name("Excel"), Some(FileFormat::Excel));
        assert_eq!(FileFormat::from_name("md"), Some(FileFormat::Markdown));
        assert_eq!(FileFormat::from_name("tab"), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_name("TEXT"), Some(FileFormat::Text));
        assert_eq!(FileFormat::from_name("htm"), Some(FileFormat::Html));
    }

    #[test]
    fn from_name_invalid() {
        assert_eq!(FileFormat::from_name("pdf"), None);
        assert_eq!(FileFormat::from_name(""), None);
    }
}
