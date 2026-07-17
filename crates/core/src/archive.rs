use std::io::Read;
use std::path::{Path, PathBuf};

/// Supported archive formats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    ZipSplit,
}

/// Table file extensions that grep-excel can parse.
pub const TABLE_EXTENSIONS: &[&str] = &[
    "xlsx", "xls", "xlsm", "xlsb", "ods", "csv", "html", "htm", "txt", "md", "markdown",
];

/// An entry within an archive.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
    pub is_file: bool,
}

/// Magic byte signatures.
const ZIP_MAGIC: &[u8] = b"PK\x03\x04";
const _GZ_MAGIC: &[u8] = &[0x1f, 0x8b];

/// Detect archive format from file path and magic bytes.
/// Returns None for table files that happen to be ZIP internally (.xlsx/.xlsm/.xlsb/.ods).
pub fn detect_archive(path: &Path) -> Option<ArchiveFormat> {
    if is_internally_zip_table_format(path) {
        return None;
    }

    if let Some(f) = detect_by_split_zip(path) {
        return Some(f);
    }
    if let Some(f) = detect_by_tar_variants(path) {
        return Some(f);
    }
    if let Some(f) = detect_by_magic(path) {
        return Some(f);
    }
    None
}

fn is_internally_zip_table_format(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(ext.as_str(), "xlsx" | "xlsm" | "xlsb" | "ods")
}

fn detect_by_split_zip(path: &Path) -> Option<ArchiveFormat> {
    let ext = path.extension()?.to_str()?;
    if ext != "001" {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let base = stem.strip_suffix(".zip")?;
    let parent = path.parent()?;
    let zip_path = parent.join(format!("{base}.zip"));
    if zip_path.exists() {
        return Some(ArchiveFormat::ZipSplit);
    }
    let second_part = parent.join(format!("{}.zip.002", base));
    if second_part.exists() {
        return Some(ArchiveFormat::ZipSplit);
    }
    None
}

fn detect_by_tar_variants(path: &Path) -> Option<ArchiveFormat> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "tar" {
        return Some(ArchiveFormat::Tar);
    }

    if ext == "tgz" {
        return Some(ArchiveFormat::TarGz);
    }

    let stem = path.file_stem()?.to_str()?;
    let stem_path = Path::new(stem);
    let inner_ext = stem_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if inner_ext != "tar" {
        return None;
    }

    match ext.as_str() {
        "gz" => Some(ArchiveFormat::TarGz),
        "bz2" => Some(ArchiveFormat::TarBz2),
        "xz" => Some(ArchiveFormat::TarXz),
        "zst" | "zstd" => Some(ArchiveFormat::TarZst),
        _ => None,
    }
}

fn detect_by_magic(path: &Path) -> Option<ArchiveFormat> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf).ok()?;

    if buf.starts_with(ZIP_MAGIC) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "zip" {
            return Some(ArchiveFormat::Zip);
        }
    }

    None
}

/// Check if an entry path has a table file extension.
pub fn is_table_entry(entry_path: &str) -> bool {
    let path = Path::new(entry_path);
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| TABLE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

// ── Entry listing ──────────────────────────────────────────────────────────

/// List all entries in an archive.
#[cfg(feature = "archive-support")]
pub fn list_entries(path: &Path, format: ArchiveFormat) -> anyhow::Result<Vec<ArchiveEntry>> {
    match format {
        ArchiveFormat::Zip => list_zip_entries(path),
        ArchiveFormat::ZipSplit => {
            let merged = concat_split_zip(path)?;
            let entries = list_zip_entries(&merged);
            let _ = std::fs::remove_file(&merged);
            entries
        }
        ArchiveFormat::Tar => list_tar_entries(path),
        ArchiveFormat::TarGz => list_tar_gz_entries(path),
        ArchiveFormat::TarBz2 => list_tar_bz2_entries(path),
        ArchiveFormat::TarXz => list_tar_xz_entries(path),
        ArchiveFormat::TarZst => list_tar_zst_entries(path),
    }
}

#[cfg(not(feature = "archive-support"))]
pub fn list_entries(_path: &Path, _format: ArchiveFormat) -> anyhow::Result<Vec<ArchiveEntry>> {
    Err(anyhow::anyhow!(
        "Archive support is not enabled. Rebuild with --features archive-support"
    ))
}

#[cfg(feature = "archive-support")]
fn list_zip_entries(path: &Path) -> anyhow::Result<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        entries.push(ArchiveEntry {
            path: entry.name().to_string(),
            size: entry.size(),
            is_file: entry.is_file(),
        });
    }
    Ok(entries)
}

#[cfg(feature = "archive-support")]
fn list_tar_entries(path: &Path) -> anyhow::Result<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(path)?;
    let mut archive = tar::Archive::new(file);
    collect_tar_entries(&mut archive)
}

#[cfg(feature = "archive-support")]
fn list_tar_gz_entries(path: &Path) -> anyhow::Result<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    collect_tar_entries(&mut archive)
}

#[cfg(feature = "archive-support")]
fn list_tar_bz2_entries(path: &Path) -> anyhow::Result<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(path)?;
    let bz = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(bz);
    collect_tar_entries(&mut archive)
}

#[cfg(feature = "archive-support")]
fn list_tar_xz_entries(path: &Path) -> anyhow::Result<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(path)?;
    let xz = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(xz);
    collect_tar_entries(&mut archive)
}

#[cfg(feature = "archive-support")]
fn list_tar_zst_entries(path: &Path) -> anyhow::Result<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(path)?;
    let zst = zstd::stream::read::Decoder::new(file)?;
    let mut archive = tar::Archive::new(zst);
    collect_tar_entries(&mut archive)
}

#[cfg(feature = "archive-support")]
fn collect_tar_entries<R: Read>(archive: &mut tar::Archive<R>) -> anyhow::Result<Vec<ArchiveEntry>> {
    let mut entries = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        let header = entry.header();
        if header.entry_type().is_dir() {
            continue;
        }
        entries.push(ArchiveEntry {
            path: entry.path()?.to_string_lossy().to_string(),
            size: header.size()?,
            is_file: header.entry_type().is_file(),
        });
    }
    Ok(entries)
}

// ── Entry extraction to temp ───────────────────────────────────────────────

/// Extract a single entry from an archive to a temp file. Returns the temp path.
#[cfg(feature = "archive-support")]
pub fn extract_entry(
    archive_path: &Path,
    entry_path: &str,
    format: ArchiveFormat,
) -> anyhow::Result<PathBuf> {
    let ext = Path::new(entry_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("tmp");
    let (_, tmp_path) = tempfile::Builder::new()
        .prefix("grep-excel-archive-")
        .suffix(&format!(".{ext}"))
        .tempfile()?
        .keep()?;

    match format {
        ArchiveFormat::Zip => {
            extract_zip_entry(archive_path, entry_path, &tmp_path)?;
        }
        ArchiveFormat::ZipSplit => {
            let merged = concat_split_zip(archive_path)?;
            let result = extract_zip_entry(&merged, entry_path, &tmp_path);
            let _ = std::fs::remove_file(&merged);
            result?;
        }
        ArchiveFormat::Tar
        | ArchiveFormat::TarGz
        | ArchiveFormat::TarBz2
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarZst => {
            extract_tar_entry(archive_path, entry_path, format, &tmp_path)?;
        }
    }

    Ok(tmp_path)
}

#[cfg(feature = "archive-support")]
fn extract_zip_entry(
    archive_path: &Path,
    entry_path: &str,
    dest: &Path,
) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_path)?;
    let mut dest_file = std::fs::File::create(dest)?;
    std::io::copy(&mut entry, &mut dest_file)?;
    Ok(())
}

#[cfg(feature = "archive-support")]
fn extract_tar_entry(
    archive_path: &Path,
    entry_path: &str,
    format: ArchiveFormat,
    dest: &Path,
) -> anyhow::Result<()> {
    fn open_tar(path: &Path, format: ArchiveFormat) -> anyhow::Result<tar::Archive<Box<dyn Read>>> {
        let file = std::fs::File::open(path)?;
        let reader: Box<dyn Read> = match format {
            ArchiveFormat::TarGz => Box::new(flate2::read::GzDecoder::new(file)),
            ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzDecoder::new(file)),
            ArchiveFormat::TarXz => Box::new(xz2::read::XzDecoder::new(file)),
            ArchiveFormat::TarZst => Box::new(zstd::stream::read::Decoder::new(file)?),
            _ => Box::new(file),
        };
        Ok(tar::Archive::new(reader))
    }

    let mut archive = open_tar(archive_path, format)?;
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()?.to_string_lossy() == entry_path {
            let mut dest_file = std::fs::File::create(dest)?;
            std::io::copy(&mut entry, &mut dest_file)?;
            return Ok(());
        }
    }
    Err(anyhow::anyhow!(
        "Entry '{}' not found in archive {:?}",
        entry_path,
        archive_path
    ))
}

// ── Split ZIP (.zip.001, .zip.002, ...) ────────────────────────────────────

/// Concatenate split ZIP parts into a single temp zip file.
#[cfg(feature = "archive-support")]
fn concat_split_zip(first_part: &Path) -> anyhow::Result<PathBuf> {
    let stem = first_part
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let base = stem.strip_suffix(".zip").unwrap_or(stem);
    let parent = first_part.parent().unwrap_or(Path::new("."));

    let tmp = tempfile::Builder::new()
        .prefix("grep-excel-split-")
        .suffix(".zip")
        .tempfile()?;
    let tmp_path = tmp.path().to_path_buf();
    let mut out = std::fs::File::create(&tmp_path)?;

    let mut part_num = 1u32;
    let mut found_any = false;
    loop {
        let part_name = if part_num == 1 {
            format!("{base}.zip.001")
        } else {
            format!("{base}.zip.{part_num:03}")
        };
        let part_path = parent.join(&part_name);
        if !part_path.exists() {
            break;
        }
        let mut part_file = std::fs::File::open(&part_path)?;
        std::io::copy(&mut part_file, &mut out)?;
        found_any = true;
        part_num += 1;
    }

    if !found_any {
        return Err(anyhow::anyhow!(
            "No split ZIP parts found for {:?}",
            first_part
        ));
    }

    Ok(tmp_path)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tar_gz_detection() {
        assert_eq!(
            detect_archive(Path::new("test.tar.gz")),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            detect_archive(Path::new("data.tar.bz2")),
            Some(ArchiveFormat::TarBz2)
        );
        assert_eq!(
            detect_archive(Path::new("archive.tar.xz")),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            detect_archive(Path::new("backup.tar.zst")),
            Some(ArchiveFormat::TarZst)
        );
        assert_eq!(
            detect_archive(Path::new("backup.tar.zstd")),
            Some(ArchiveFormat::TarZst)
        );
    }

    #[test]
    fn test_tar_detection() {
        assert_eq!(
            detect_archive(Path::new("archive.tar")),
            Some(ArchiveFormat::Tar)
        );
    }

    #[test]
    fn test_tgz_detection() {
        assert_eq!(
            detect_archive(Path::new("backup.tgz")),
            Some(ArchiveFormat::TarGz)
        );
    }

    #[test]
    fn test_xlsx_not_archive() {
        assert_eq!(detect_archive(Path::new("data.xlsx")), None);
        assert_eq!(detect_archive(Path::new("macro.xlsm")), None);
        assert_eq!(detect_archive(Path::new("binary.xlsb")), None);
        assert_eq!(detect_archive(Path::new("open.ods")), None);
    }

    #[test]
    fn test_xls_not_archive() {
        assert_eq!(detect_archive(Path::new("legacy.xls")), None);
    }

    #[test]
    fn test_csv_not_archive() {
        assert_eq!(detect_archive(Path::new("data.csv")), None);
    }

    #[test]
    fn test_is_table_entry() {
        assert!(is_table_entry("data.xlsx"));
        assert!(is_table_entry("subdir/report.csv"));
        assert!(is_table_entry("notes.txt"));
        assert!(!is_table_entry("image.png"));
        assert!(!is_table_entry("readme.pdf"));
        assert!(!is_table_entry("dir/"));
    }

    #[test]
    fn test_is_table_entry_case_insensitive() {
        assert!(is_table_entry("DATA.XLSX"));
        assert!(is_table_entry("Report.CSV"));
    }

    #[test]
    fn test_split_zip_detection() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");
        std::fs::write(&zip_path, b"dummy").unwrap();
        // .zip.001 exists → split detection via the zip sibling
        // We can't unit-test without actual files, but the path logic is covered
        let path = dir.path().join("test.zip.001");
        assert_eq!(detect_archive(&path), Some(ArchiveFormat::ZipSplit));
    }

    #[test]
    fn test_unknown_extension_not_archive() {
        assert_eq!(detect_archive(Path::new("data.bin")), None);
        assert_eq!(detect_archive(Path::new("notes")), None);
    }

    #[cfg(feature = "archive-support")]
    #[test]
    fn test_zip_with_csv_extraction() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");

        // Create a real ZIP with a CSV inside
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip_writer.start_file("data.csv", options).unwrap();
        zip_writer.write_all(b"name,value\nalice,10\nbob,20\n").unwrap();
        zip_writer.finish().unwrap();

        // Detect archive
        assert_eq!(detect_archive(&zip_path), Some(ArchiveFormat::Zip));

        // List entries
        let entries = list_entries(&zip_path, ArchiveFormat::Zip).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "data.csv");
        assert!(entries[0].is_file);

        // Check table entry detection
        assert!(is_table_entry("data.csv"));

        // Extract entry
        let tmp = extract_entry(&zip_path, "data.csv", ArchiveFormat::Zip).unwrap();
        assert!(tmp.exists());

        // Verify content
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("alice,10"));
        assert!(content.contains("bob,20"));

        let _ = std::fs::remove_file(&tmp);
    }

    #[cfg(feature = "archive-support")]
    #[test]
    fn test_zip_no_table_files() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("notables.zip");

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip_writer.start_file("readme.txt", options).unwrap();
        zip_writer.write_all(b"hello").unwrap();
        zip_writer.start_file("image.png", options).unwrap();
        zip_writer.write_all(b"pngdata").unwrap();
        zip_writer.finish().unwrap();

        let entries = list_entries(&zip_path, ArchiveFormat::Zip).unwrap();
        // Both entries exist, but only "readme.txt" has a table extension
        let table_count = entries.iter().filter(|e| is_table_entry(&e.path)).count();
        assert_eq!(table_count, 1); // "readme.txt" IS a table extension
        // "image.png" is NOT a table extension
        assert!(!is_table_entry("image.png"));
    }
}
