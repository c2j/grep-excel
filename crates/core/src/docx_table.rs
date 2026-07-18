use std::path::Path;

use crate::excel::SheetData;

pub fn parse_docx(_path: &Path) -> anyhow::Result<Vec<SheetData>> {
    anyhow::bail!("docx parsing not yet implemented")
}
