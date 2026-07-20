pub use grep_excel_core::{engine, excel, i18n, types};

pub mod app;
pub mod event;
pub mod interactive;

#[cfg(feature = "mcp-server")]
pub mod mcp;

#[cfg(feature = "share-url")]
pub fn resolve_share_auth(
    cli_cookie: Option<&str>,
) -> Option<grep_excel_core::source::download::ShareAuth> {
    use grep_excel_core::source::download::ShareAuth;

    if let Some(c) = cli_cookie {
        if !c.is_empty() {
            return Some(ShareAuth {
                cookie: c.to_string(),
            });
        }
    }
    if let Ok(c) = std::env::var("KDOCS_COOKIE") {
        if !c.is_empty() {
            return Some(ShareAuth { cookie: c });
        }
    }
    if let Ok(c) = std::env::var("WPS_COOKIE") {
        if !c.is_empty() {
            return Some(ShareAuth { cookie: c });
        }
    }
    read_cookie_file().map(|c| ShareAuth { cookie: c })
}

#[cfg(feature = "share-url")]
fn read_cookie_file() -> Option<String> {
    let path = if cfg!(target_os = "macos") {
        dirs::data_dir()?.join("grep-excel").join("kdocs_cookie")
    } else {
        dirs::config_dir()?.join("grep-excel").join("kdocs_cookie")
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mode = meta.permissions().mode();
            if mode & 0o077 != 0 {
                eprintln!(
                    "Warning: {} is world/group readable. Recommend: chmod 600 {}",
                    path.display(),
                    path.display()
                );
            }
        }
    }

    let content = std::fs::read_to_string(&path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
