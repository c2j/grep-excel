use std::path::Path;

use crate::excel::SheetData;

pub fn parse_pptx(_path: &Path) -> anyhow::Result<Vec<SheetData>> {
    anyhow::bail!("pptx parsing not yet implemented")
}
