# Cloud Share URL Import Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow Kingsoft Docs / WPS cloud share URLs as file inputs — download via session Cookie to tempfile, then import through existing pipeline.

**Architecture:** New `source` module in `crates/core` behind feature `share-url` handles URL classification (configurable provider profile, no hardcoded single host) and Cookie-authenticated download. CLI/TUI/MCP call `resolve_source()` before `import_excel`. Existing engine contract unchanged.

**Tech Stack:** Rust, `reqwest` (blocking + rustls), `tempfile`, existing clap/calamine/anyhow

**Design doc:** `docs/plans/2026-07-14-kdocs-share-url-import-design.md` (Momus-approved)

---

### Task 1: Add Dependencies and Feature Flag

**Files:**
- Modify: `Cargo.toml` (workspace root) — add workspace deps
- Modify: `crates/core/Cargo.toml` — add reqwest, tempfile, feature `share-url`
- Modify: `crates/cli/Cargo.toml` — propagate feature

**Step 1: Add workspace deps in `Cargo.toml`**

Add to `[workspace.dependencies]`:

```toml
reqwest = { version = "0.12", features = ["blocking", "json", "rustls-tls"], default-features = false }
tempfile = "3"
```

**Step 2: Add to `crates/core/Cargo.toml`**

In `[dependencies]` section add:

```toml
reqwest = { workspace = true, optional = true }
tempfile = { workspace = true, optional = true }
```

In `[features]` section add:

```toml
share-url = ["dep:reqwest", "dep:tempfile"]
```

**Step 3: Add to `crates/cli/Cargo.toml`**

In `[features]` section add:

```toml
share-url = ["grep-excel-core/share-url"]
```

Update `full` feature:

```toml
# BEFORE:
full = ["engine-memory", "file-dialog", "mcp-server"]
# AFTER:
full = ["engine-memory", "file-dialog", "mcp-server", "share-url"]
```

**Step 4: Verify build**

Run: `cargo check --features share-url`
Expected: compiles (nothing uses new deps yet, just available).

Run: `cargo check` (default features, no share-url)
Expected: compiles (feature off, no new code referenced).

**Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/core/Cargo.toml crates/cli/Cargo.toml
git commit -m "deps: add reqwest + tempfile behind share-url feature"
```

---

### Task 2: Create `source.rs` — URL Classification + Provider Profile

**Files:**
- Create: `crates/core/src/source.rs`
- Modify: `crates/core/src/lib.rs` — add `pub mod source;`

**Step 1: Write `source.rs` with types and classification**

```rust
//! Cloud share URL classification and provider profiles.
//!
//! When feature `share-url` is enabled, this module can also download
//! matched share URLs to temporary files for local import.

use std::path::PathBuf;

/// Result of classifying a user-supplied input string.
#[derive(Debug, Clone)]
pub enum SourceKind {
    /// A local filesystem path — pass through to existing import.
    Local(PathBuf),
    /// A cloud share URL matched by a configured provider.
    #[cfg(feature = "share-url")]
    CloudShare {
        provider: ShareProvider,
        sid: String,
        original_url: String,
    },
    /// An http(s):// URL that didn't match any provider.
    UnsupportedRemote { url: String },
}

/// Built-in provider profile for Kingsoft Docs / WPS cloud shares.
#[derive(Debug, Clone)]
pub struct ShareProvider {
    /// Stable identifier, e.g. `"kingsoft_share"`.
    pub id: &'static str,
    /// Hosts that this provider will match (case-insensitive).
    /// Checked against the URL host with exact match OR suffix `.host`.
    pub hosts: &'static [&'static str],
    /// Path prefix that marks a share link (e.g. `/l/`).
    pub share_path_prefix: &'static str,
    /// API template for the office download endpoint.
    /// `{sid}` is replaced with the captured share id.
    /// `{host}` is replaced with the URL's host.
    pub office_download_template: &'static str,
    /// Origin header template. `{host}` → URL host.
    pub origin_template: &'static str,
    /// Referer header template.
    pub referer_template: &'static str,
}

/// Default Kingsoft Docs / WPS share provider.
///
/// Hosts are data, not hardcoded logic — extend via config in future phases.
pub const KINGSOFT_SHARE: ShareProvider = ShareProvider {
    id: "kingsoft_share",
    // Match kdocs.cn, www.kdocs.cn, and *.kdocs.cn subdomains.
    // Suffix matching: if host == item OR host.ends_with("." + item).
    hosts: &["kdocs.cn"],
    share_path_prefix: "/l/",
    office_download_template: "https://www.kdocs.cn/api/v3/office/file/{sid}/download",
    origin_template: "https://{host}",
    referer_template: "https://{host}/l/{sid}",
};

/// All built-in providers. Future: extend with user config.
pub const BUILTIN_PROVIDERS: &[ShareProvider] = &[KINGSOFT_SHARE];

/// Classify a user input into `SourceKind`.
///
/// - Inputs without `://` are treated as local paths.
/// - `http://` or `https://` inputs are matched against providers.
/// - Unmatched URLs become `UnsupportedRemote`.
pub fn classify_source(input: &str) -> SourceKind {
    classify_with_providers(input, BUILTIN_PROVIDERS)
}

/// Classification with explicit provider list (for testing).
pub fn classify_with_providers(input: &str, providers: &[ShareProvider]) -> SourceKind {
    // Heuristic: if it doesn't start with http:// or https://, it's local.
    let lower = input.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return SourceKind::Local(PathBuf::from(input));
    }

    // Parse host and path from URL manually (avoid pulling in `url` crate for MVP).
    let after_scheme = input
        .splitn(2, "://")
        .nth(1)
        .unwrap_or("");
    let (host_port, path_query) = match after_scheme.find('/') {
        Some(idx) => (&after_scheme[..idx], &after_scheme[idx..]),
        None => (after_scheme, ""),
    };
    // Strip port
    let host = host_port.split(':').next().unwrap_or(host_port);
    let path = path_query.split(['?', '#']).next().unwrap_or(path_query);

    for provider in providers {
        if host_matches(host, provider.hosts) {
            if let Some(sid) = extract_sid(path, provider.share_path_prefix) {
                return SourceKind::CloudShare {
                    provider: provider.clone(),
                    sid,
                    original_url: input.to_string(),
                };
            }
        }
    }

    SourceKind::UnsupportedRemote {
        url: input.to_string(),
    }
}

/// Check if `host` matches any entry in `allowed`.
/// Exact match OR `host` ends with `.entry` (subdomain).
fn host_matches(host: &str, allowed: &[&str]) -> bool {
    let host_lower = host.to_ascii_lowercase();
    allowed.iter().any(|entry| {
        let entry_lower = entry.to_ascii_lowercase();
        host_lower == entry_lower || host_lower.ends_with(&format!(".{}", entry_lower))
    })
}

/// Extract share id from path like `/l/{sid}` or `/l/{sid}/`.
fn extract_sid(path: &str, prefix: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let after = path.strip_prefix(prefix)?;
    let sid = after.split('/').next()?;
    if sid.is_empty() {
        return None;
    }
    // Validate: alphanumeric + dash/underscore only
    if sid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        Some(sid.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_local_path() {
        assert!(matches!(
            classify_source("data.xlsx"),
            SourceKind::Local(_)
        ));
        assert!(matches!(
            classify_source("/home/user/file.xlsx"),
            SourceKind::Local(_)
        ));
        assert!(matches!(
            classify_source("C:\\Users\\file.xlsx"),
            SourceKind::Local(_)
        ));
    }

    #[test]
    fn classify_kdocs_share() {
        match classify_source("https://www.kdocs.cn/l/catuXnZRIB58") {
            SourceKind::CloudShare { sid, .. } => assert_eq!(sid, "catuXnZRIB58"),
            other => panic!("expected CloudShare, got {:?}", other),
        }
        match classify_source("https://kdocs.cn/l/abc123") {
            SourceKind::CloudShare { sid, .. } => assert_eq!(sid, "abc123"),
            other => panic!("expected CloudShare, got {:?}", other),
        }
    }

    #[test]
    fn classify_kdocs_subdomain() {
        match classify_source("https://e.kdocs.cn/l/xyz") {
            SourceKind::CloudShare { sid, .. } => assert_eq!(sid, "xyz"),
            other => panic!("expected CloudShare for subdomain, got {:?}", other),
        }
    }

    #[test]
    fn classify_unmatched_url() {
        assert!(matches!(
            classify_source("https://example.com/file.xlsx"),
            SourceKind::UnsupportedRemote { .. }
        ));
        assert!(matches!(
            classify_source("https://google.com"),
            SourceKind::UnsupportedRemote { .. }
        ));
    }

    #[test]
    fn classify_kdocs_non_share_path() {
        // kdocs.cn but not /l/ path → unsupported
        assert!(matches!(
            classify_source("https://www.kdocs.cn/doc/123"),
            SourceKind::UnsupportedRemote { .. }
        ));
    }

    #[test]
    fn classify_url_with_query_fragment() {
        match classify_source("https://www.kdocs.cn/l/catuXnZRIB58?from=docs#section") {
            SourceKind::CloudShare { sid, .. } => assert_eq!(sid, "catuXnZRIB58"),
            other => panic!("expected CloudShare, got {:?}", other),
        }
    }

    #[test]
    fn host_match_exact_and_subdomain() {
        assert!(host_matches("kdocs.cn", &["kdocs.cn"]));
        assert!(host_matches("www.kdocs.cn", &["kdocs.cn"]));
        assert!(host_matches("e.kdocs.cn", &["kdocs.cn"]));
        assert!(!host_matches("notkdocs.cn", &["kdocs.cn"]));
        assert!(!host_matches("kdocs.cn.evil.com", &["kdocs.cn"]));
    }

    #[test]
    fn extract_sid_various() {
        assert_eq!(extract_sid("/l/abc", "/l/"), Some("abc".into()));
        assert_eq!(extract_sid("/l/abc/", "/l/"), Some("abc".into()));
        assert_eq!(extract_sid("/l/", "/l/"), None);
        assert_eq!(extract_sid("/doc/abc", "/l/"), None);
    }
}
```

**Step 2: Register module in `lib.rs`**

Add `pub mod source;` to `crates/core/src/lib.rs`:

```rust
// BEFORE:
pub mod types;
pub mod engine;
pub mod excel;
pub mod html_table;
pub mod text_table;
pub mod i18n;

// AFTER:
pub mod types;
pub mod engine;
pub mod excel;
pub mod html_table;
pub mod text_table;
pub mod i18n;
pub mod source;
```

**Step 3: Run tests**

Run: `cargo test -p grep-excel-core source:: -- --nocapture`
Expected: All 8 tests pass.

**Step 4: Commit**

```bash
git add crates/core/src/source.rs crates/core/src/lib.rs
git commit -m "feat: add source module for URL classification + provider profiles"
```

---

### Task 3: Implement Download Function (behind `share-url` feature)

**Files:**
- Modify: `crates/core/src/source.rs` — add `download_share`, `ShareAuth`, `resolve_source`

**Step 1: Add download implementation to `source.rs`**

Append to `source.rs` (everything behind `#[cfg(feature = "share-url")]`):

```rust
// ── Download support (feature-gated) ──────────────────────────────────────

#[cfg(feature = "share-url")]
pub mod download {
    use super::*;
    use anyhow::{anyhow, Context, Result};
    use std::path::PathBuf;

    /// Authentication for cloud share downloads.
    #[derive(Clone)]
    pub struct ShareAuth {
        /// Raw Cookie header value, e.g. `"wps_sid=...; csrf=..."`
        pub cookie: String,
    }

    impl std::fmt::Debug for ShareAuth {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // NEVER log the actual cookie value.
            f.debug_struct("ShareAuth")
                .field("cookie", &format!("*** ({} chars)", self.cookie.len()))
                .finish()
        }
    }

    /// Resolved source: either a local path or a downloaded temp file.
    pub enum ResolvedSource {
        /// Original local path.
        Local(PathBuf),
        /// Downloaded temp file path + original filename for display.
        Downloaded {
            path: PathBuf,
            display_name: String,
            /// Holds the temp file alive until this is dropped.
            _guard: TempGuard,
        },
    }

    /// Keeps a temp file alive; deletes on drop.
    pub struct TempGuard {
        path: PathBuf,
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Resolve a user input into a local path.
    ///
    /// - Local path → `ResolvedSource::Local` (no-op).
    /// - Cloud share URL → downloads to temp, returns `ResolvedSource::Downloaded`.
    /// - Unsupported URL → error with guidance.
    pub fn resolve_source(input: &str, auth: Option<&ShareAuth>) -> Result<ResolvedSource> {
        match classify_source(input) {
            SourceKind::Local(path) => Ok(ResolvedSource::Local(path)),
            SourceKind::CloudShare {
                provider,
                sid,
                original_url,
            } => {
                let a = auth.ok_or_else(|| {
                    anyhow!(
                        "Cloud share URL requires authentication.\n\
                         Set KDOCS_COOKIE env var or use --kdocs-cookie flag.\n\
                         URL: {}",
                        original_url
                    )
                })?;
                download_share(&provider, &sid, &original_url, a)
            }
            SourceKind::UnsupportedRemote { url } => Err(anyhow!(
                "URL is not a recognized cloud share link: {}\n\
                 Supported: kdocs.cn / *.kdocs.cn share links (/l/...).\n\
                 For other URLs, download the file locally first.",
                url
            )),
        }
    }

    /// Download a cloud share to a temp file.
    fn download_share(
        provider: &ShareProvider,
        sid: &str,
        original_url: &str,
        auth: &ShareAuth,
    ) -> Result<ResolvedSource> {
        let api_url = provider
            .office_download_template
            .replace("{sid}", sid);

        let origin = provider
            .origin_template
            .replace("{host}", &extract_host(original_url).unwrap_or_default());

        let referer = provider
            .referer_template
            .replace("{host}", &extract_host(original_url).unwrap_or_default())
            .replace("{sid}", sid);

        // Step 1: Request download URL from API
        let resp = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?
            .get(&api_url)
            .header("Cookie", &auth.cookie)
            .header("Origin", &origin)
            .header("Referer", &referer)
            .header(
                "User-Agent",
                "Mozilla/5.0 (compatible; grep-excel)",
            )
            .send()
            .context("Failed to connect to share API")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            if body.contains("userNotLogin") || status == reqwest::StatusCode::FORBIDDEN {
                return Err(anyhow!(
                    "Authentication failed: session expired or insufficient permissions.\n\
                     Re-login to kdocs.cn and update your KDOCS_COOKIE.\n\
                     API returned: {}",
                    truncate(&body, 200)
                ));
            }
            return Err(anyhow!(
                "Share API returned HTTP {}:\n{}",
                status,
                truncate(&body, 200)
            ));
        }

        let json: serde_json::Value =
            resp.json().context("Share API returned non-JSON response")?;

        // Extract download URL: try "url" then "download_url"
        let dl_url = json
            .get("url")
            .or_else(|| json.get("download_url"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "Share API response missing download URL.\n\
                     Possible: download not permitted for this share, or API changed.\n\
                     Response: {}",
                    truncate(&json.to_string(), 300)
                )
            })?;

        // Try to get filename from response or default
        let display_name = json
            .get("fname")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("kdocs-{}.xlsx", sid));

        // Step 2: Download the actual file
        let file_resp = reqwest::blocking::get(dl_url)
            .context("Failed to download file from temporary URL")?;

        if !file_resp.status().is_success() {
            return Err(anyhow!(
                "File download failed: HTTP {}",
                file_resp.status()
            ));
        }

        let bytes = file_resp
            .bytes()
            .context("Failed to read file bytes")?;

        if bytes.is_empty() {
            return Err(anyhow!("Downloaded file is empty"));
        }

        // Step 3: Write to temp file
        let ext = std::path::Path::new(&display_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("xlsx");

        let mut tmp_builder = tempfile::Builder::new();
        tmp_builder
            .prefix(&format!("grep-excel-share-{}-", &sid[..sid.len().min(16)]))
            .suffix(&format!(".{}", ext));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp_builder.permissions(std::fs::Permissions::from_mode(0o600));
        }

        let mut tmp = tmp_builder
            .tempfile()
            .context("Failed to create temp file")?;

        use std::io::Write;
        tmp.write_all(&bytes).context("Failed to write temp file")?;
        tmp.flush()?;

        // We need the path to persist; convert to NamedTempFile path via keep()
        let (file, path) = tmp
            .keep()
            .context("Failed to finalize temp file")?;
        drop(file); // close file handle; path guarded by TempGuard

        Ok(ResolvedSource::Downloaded {
            path: path.clone(),
            display_name,
            _guard: TempGuard { path },
        })
    }

    /// Extract host from a URL string (manual parse for MVP).
    fn extract_host(url: &str) -> Option<String> {
        let after_scheme = url.splitn(2, "://").nth(1)?;
        let host_port = after_scheme.split('/').next()?;
        let host = host_port.split(':').next()?;
        Some(host.to_string())
    }

    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            format!("{}...(truncated)", &s[..max])
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn auth_debug_redacts_cookie() {
            let auth = ShareAuth {
                cookie: "super_secret_cookie_value".to_string(),
            };
            let dbg = format!("{:?}", auth);
            assert!(dbg.contains("***"));
            assert!(!dbg.contains("super_secret"));
        }

        #[test]
        fn extract_host_works() {
            assert_eq!(
                extract_host("https://www.kdocs.cn/l/abc"),
                Some("www.kdocs.cn".into())
            );
            assert_eq!(extract_host("https://kdocs.cn"), Some("kdocs.cn".into()));
            assert_eq!(extract_host("not_a_url"), None);
        }
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p grep-excel-core source:: --features share-url -- --nocapture`
Expected: All tests pass (including new auth redaction test).

Run: `cargo test -p grep-excel-core source::` (without feature)
Expected: Classification tests pass; download tests skipped (feature off).

**Step 3: Commit**

```bash
git add crates/core/src/source.rs
git commit -m "feat: add share-url download to tempfile (feature share-url)"
```

---

### Task 4: Wire CLI — `--kdocs-cookie` + Resolve in All Entry Points

**Files:**
- Modify: `crates/cli/src/main.rs` — add CLI arg, resolve helper, wire into all run_* functions
- Modify: `crates/cli/src/lib.rs` — re-export source module if needed

**Step 1: Add `--kdocs-cookie` to clap `Args`**

In `crates/cli/src/main.rs`, add field to the `Args` struct (after `run_output_column`):

```rust
    #[arg(
        long,
        help = "Cookie for Kingsoft Docs / WPS cloud share URL downloads only. \
                Prefer KDOCS_COOKIE env var to avoid shell history exposure."
    )]
    kdocs_cookie: Option<String>,
```

**Step 2: Add resolve helper function**

Add a helper near `import_file_with_repair` (after line 227):

```rust
/// Resolve a source string (local path or cloud share URL) to a local path.
///
/// For cloud share URLs: downloads to temp file using provided auth.
/// For local paths: returns the path as-is.
#[cfg(feature = "share-url")]
fn resolve_source_to_path(
    input: &str,
    auth: Option<&grep_excel_core::source::download::ShareAuth>,
    repair: bool,
) -> anyhow::Result<(
    std::path::PathBuf,
    String, // display name
    Option<grep_excel_core::source::download::TempGuard>, // temp guard if downloaded
)> {
    use grep_excel_core::source::download::{resolve_source, ResolvedSource};

    match resolve_source(input, auth)? {
        ResolvedSource::Local(path) => {
            let display = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            Ok((path, display, None))
        }
        ResolvedSource::Downloaded {
            path,
            display_name,
            _guard,
        } => {
            let _ = repair; // repair applies at import_excel level
            Ok((path, display_name, Some(_guard)))
        }
    }
}
```

**Step 3: Build ShareAuth from CLI/env/file**

Add a function to resolve auth credentials:

```rust
#[cfg(feature = "share-url")]
fn resolve_share_auth(cli_cookie: Option<&str>) -> Option<grep_excel_core::source::download::ShareAuth> {
    use grep_excel_core::source::download::ShareAuth;

    // 1. CLI flag
    if let Some(c) = cli_cookie {
        if !c.is_empty() {
            return Some(ShareAuth { cookie: c.to_string() });
        }
    }

    // 2. Env var KDOCS_COOKIE
    if let Ok(c) = std::env::var("KDOCS_COOKIE") {
        if !c.is_empty() {
            return Some(ShareAuth { cookie: c });
        }
    }

    // 3. Env var WPS_COOKIE (alias)
    if let Ok(c) = std::env::var("WPS_COOKIE") {
        if !c.is_empty() {
            return Some(ShareAuth { cookie: c });
        }
    }

    // 4. Config file
    if let Some(cookie) = read_cookie_file() {
        return Some(ShareAuth { cookie });
    }

    None
}

#[cfg(feature = "share-url")]
fn read_cookie_file() -> Option<String> {
    let path = if cfg!(target_os = "macos") {
        dirs::data_dir()?.join("grep-excel").join("kdocs_cookie")
    } else {
        dirs::config_dir()?.join("grep-excel").join("kdocs_cookie")
    };

    // Warn if world-readable on Unix
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
```

**Step 4: Modify `run_cli` to resolve sources**

Current code iterates `args.files` with `file.exists()` checks. Replace with resolve-aware version.

**IMPORTANT:** Change `files: Vec<PathBuf>` to `files: Vec<String>` in the `Args` struct (line 14). This is required because `PathBuf` mangles URL strings on Windows and makes URL detection unreliable.

```rust
// In Args struct, BEFORE:
//     files: Vec<PathBuf>,
// AFTER:
    #[arg(name = "FILES")]
    files: Vec<String>,
```

Then update `import_file_with_repair` to take `&str` instead of `&PathBuf`, and update `run_cli`:

```rust
fn run_cli(args: &Args) -> Result<()> {
    let mut db = DefaultEngine::new()?;

    #[cfg(feature = "share-url")]
    let share_auth = resolve_share_auth(args.kdocs_cookie.as_deref());

    // Keep temp file guards alive for the duration of the function
    #[cfg(feature = "share-url")]
    let mut _temp_guards: Vec<grep_excel_core::source::download::TempGuard> = Vec::new();

    for input in &args.files {
        #[cfg(feature = "share-url")]
        {
            match resolve_source_to_path(input, share_auth.as_ref(), args.repair) {
                Ok((path, display_name, guard)) => {
                    if let Some(g) = guard {
                        _temp_guards.push(g);
                    }
                    if !path.exists() {
                        eprintln!("{}", grep_excel::i18n::cli_file_not_found(&display_name));
                        continue;
                    }
                    match import_file_with_repair(&mut db, &path, args.repair) {
                        Ok(info) => eprintln!(
                            "{}",
                            grep_excel::i18n::cli_imported(
                                &info.name,
                                info.sheets.len(),
                                info.total_rows
                            )
                        ),
                        Err(e) => eprintln!(
                            "{}",
                            grep_excel::i18n::cli_import_failed(
                                &display_name,
                                &e.to_string()
                            )
                        ),
                    }
                }
                Err(e) => {
                    eprintln!("Error resolving '{}': {}", input, e);
                }
            }
        }

        #[cfg(not(feature = "share-url"))]
        {
            let path = std::path::PathBuf::from(input);
            if !path.exists() {
                eprintln!(
                    "{}",
                    grep_excel::i18n::cli_file_not_found(&path.display().to_string())
                );
                continue;
            }
            match import_file_with_repair(&mut db, &path, args.repair) {
                Ok(info) => eprintln!(
                    "{}",
                    grep_excel::i18n::cli_imported(&info.name, info.sheets.len(), info.total_rows)
                ),
                Err(e) => eprintln!(
                    "{}",
                    grep_excel::i18n::cli_import_failed(&path.display().to_string(), &e.to_string())
                ),
            }
        }
    }

    // ... rest of run_cli unchanged (query setup, search, output)
```

**Step 5: Apply the same pattern to `run_tui`, `run_sql_cli`, `run_interactive_cli`, `run_exec`**

For each of these functions that iterate `args.files`:
- Add `#[cfg(feature = "share-url")]` resolve block
- Add `#[cfg(not(feature = "share-url"))]` fallback using `PathBuf::from(input)`
- Keep temp guards alive in a local `Vec`

Pattern for `run_tui`:

```rust
fn run_tui(args: &Args) -> Result<()> {
    let database = DefaultEngine::new()?;
    let (event_tx, event_rx) = create_event_channel();
    let mut app = App::new(database, event_tx, event_rx);

    #[cfg(feature = "share-url")]
    let share_auth = resolve_share_auth(args.kdocs_cookie.as_deref());

    for input in &args.files {
        #[cfg(feature = "share-url")]
        {
            match resolve_source_to_path(input, share_auth.as_ref(), args.repair) {
                Ok((path, _, _guard)) => {
                    // NOTE: _guard drops here, but import_file copies data into engine,
                    // so the temp file is only needed during import, not after.
                    // This is fine for TUI preload.
                    if path.exists() {
                        app.import_file(path);
                    }
                }
                Err(e) => eprintln!("Error resolving '{}': {}", input, e),
            }
        }

        #[cfg(not(feature = "share-url"))]
        {
            let path = std::path::PathBuf::from(input);
            if path.exists() {
                app.import_file(path);
            }
        }
    }

    app.run()
}
```

**IMPORTANT — Tempfile lifetime for TUI:** The `import_file` in TUI spawns a thread that reads the file. If the guard drops immediately, the file may be deleted before the thread finishes reading. 

**Fix:** Store guards on the `App` struct or a long-lived container. For MVP simplicity in TUI, hold guards in a `Vec` local to `run_tui` (it lives as long as the TUI session):

```rust
fn run_tui(args: &Args) -> Result<()> {
    let database = DefaultEngine::new()?;
    let (event_tx, event_rx) = create_event_channel();
    let mut app = App::new(database, event_tx, event_rx);

    #[cfg(feature = "share-url")]
    let share_auth = resolve_share_auth(args.kdocs_cookie.as_deref());
    #[cfg(feature = "share-url")]
    let mut temp_guards: Vec<grep_excel_core::source::download::TempGuard> = Vec::new();

    for input in &args.files {
        #[cfg(feature = "share-url")]
        {
            match resolve_source_to_path(input, share_auth.as_ref(), args.repair) {
                Ok((path, _, guard)) => {
                    if let Some(g) = guard {
                        temp_guards.push(g);
                    }
                    if path.exists() {
                        app.import_file(path);
                    }
                }
                Err(e) => eprintln!("Error resolving '{}': {}", input, e),
            }
        }

        #[cfg(not(feature = "share-url"))]
        {
            let path = std::path::PathBuf::from(input);
            if path.exists() {
                app.import_file(path);
            }
        }
    }

    app.run() // temp_guards live until end of run_tui
}
```

**Step 6: Fix all `args.files` usages that assumed `PathBuf`**

Search for all places that use `args.files` and ensure they work with `Vec<String>`. The main change is converting `file` (now `&String`) to `&PathBuf` or `&Path` where needed via `PathBuf::from(file)` or `Path::new(file)`.

Functions to check:
- `run_list_tables_cli`
- `run_sql_cli`
- `run_interactive_cli`
- `run_exec`
- `run_exec_shell`

For each, apply the same resolve pattern. For `run_exec` and `run_exec_shell`, auto-import of positional files should also resolve share URLs.

**Step 7: Verify build**

Run: `cargo check --features share-url`
Expected: compiles.

Run: `cargo check`
Expected: compiles (cfg-not paths active).

**Step 8: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat: wire --kdocs-cookie + URL resolve into CLI entry points"
```

---

### Task 5: Wire MCP `import_file` for Share URLs

**Files:**
- Modify: `crates/cli/src/mcp.rs` — resolve share URL in `import_file` tool

**Step 1: Modify `import_file` in `mcp.rs`**

Current `import_file` (line 260) does `PathBuf::from(&params.file_path)` directly. Add resolve before import:

```rust
    pub async fn import_file(
        &self,
        Parameters(params): Parameters<ImportFileParams>,
    ) -> Result<String, String> {
        let file_path_str = params.file_path.clone();
        let db = Arc::clone(&self.db);
        let import_paths = Arc::clone(&self.import_paths);

        #[cfg(feature = "share-url")]
        {
            let auth = crate::resolve_share_auth(None); // MCP uses env/file only
            match grep_excel_core::source::download::resolve_source(&file_path_str, auth.as_ref()) {
                Ok(grep_excel_core::source::download::ResolvedSource::Local(path)) => {
                    return self.do_import(&path, &file_path_str, db, import_paths).await;
                }
                Ok(grep_excel_core::source::download::ResolvedSource::Downloaded {
                    path,
                    display_name,
                    _guard,
                }) => {
                    // NOTE: guard dropped here — but import is synchronous in spawn_blocking,
                    // so file is read before guard drops.
                    // For safety, leak the guard (temp file cleaned by OS) or store in server.
                    // MVP: use keep() path that persists; guard deletion happens on import completion.
                    let result = self.do_import(&path, &display_name, db, import_paths).await;
                    // Guard drops here, temp file deleted after import
                    let _ = _guard;
                    return result;
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        #[cfg(not(feature = "share-url"))]
        {
            let path = std::path::PathBuf::from(&file_path_str);
            self.do_import(&path, &file_path_str, db, import_paths).await
        }
    }

    /// Shared import logic (extracted from import_file).
    async fn do_import(
        &self,
        path: &std::path::Path,
        display_name_or_url: &str,
        db: Arc<Mutex<SyncDb>>,
        import_paths: Arc<RwLock<std::collections::HashMap<String, String>>>,
    ) -> Result<String, String> {
        let canonical = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| display_name_or_url.to_string());
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let mut guard = db.lock();
            guard
                .0
                .import_excel(&path, &|_, _| {})
                .map(|info| {
                    import_paths.write().insert(info.name.clone(), canonical);
                    let mcp_info: McpFileInfo = info.into();
                    serde_json::to_string_pretty(&mcp_info)
                        .unwrap_or_else(|_| format!("{:?}", info))
                })
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("Join error: {}", e))?
    }
```

**Step 2: Make `resolve_share_auth` accessible from mcp.rs**

If `resolve_share_auth` is defined in `main.rs`, move it to `lib.rs` or make it `pub`. Simplest: move to `crates/cli/src/lib.rs`:

```rust
// crates/cli/src/lib.rs
#[cfg(feature = "share-url")]
pub fn resolve_share_auth(cli_cookie: Option<&str>) -> Option<grep_excel_core::source::download::ShareAuth> {
    // ... (same implementation as Task 4 Step 3)
}
```

Then in `mcp.rs` use `crate::resolve_share_auth(None)`.

**Step 3: Verify build**

Run: `cargo check --features "share-url mcp-server"`
Expected: compiles.

**Step 4: Commit**

```bash
git add crates/cli/src/mcp.rs crates/cli/src/lib.rs crates/cli/src/main.rs
git commit -m "feat: wire MCP import_file to resolve share URLs"
```

---

### Task 6: Add i18n Error Strings + Update README

**Files:**
- Modify: `crates/core/src/i18n.rs` — add share-related messages
- Modify: `README.md` — document the feature

**Step 1: Add i18n functions**

In `crates/core/src/i18n.rs`, add:

```rust
/// Cloud share URL requires authentication.
pub fn share_needs_auth(url: &str) -> String {
    match current() {
        Lang::Zh => format!(
            "云文档链接需要登录凭证: {}\n\
             请设置 KDOCS_COOKIE 环境变量，或使用 --kdocs-cookie 参数。\n\
             获取方式：浏览器登录 kdocs.cn → F12 → Network → 复制 Cookie",
            url
        ),
        Lang::En => format!(
            "Cloud share URL requires authentication: {}\n\
             Set KDOCS_COOKIE env var or use --kdocs-cookie flag.\n\
             To get cookie: login to kdocs.cn → F12 → Network → copy Cookie header",
            url
        ),
    }
}

/// Unsupported remote URL.
pub fn share_unsupported_url(url: &str) -> String {
    match current() {
        Lang::Zh => format!(
            "不支持的远程链接: {}\n\
             支持: kdocs.cn / *.kdocs.cn 分享链接 (/l/...)。",
            url
        ),
        Lang::En => format!(
            "Unsupported remote URL: {}\n\
             Supported: kdocs.cn / *.kdocs.cn share links (/l/...).",
            url
        ),
    }
}

/// Authentication failed (expired/invalid).
pub fn share_auth_failed() -> String {
    match current() {
        Lang::Zh => "认证失败: 会话已过期或权限不足，请重新登录 kdocs.cn 并更新 Cookie。".to_string(),
        Lang::En => "Authentication failed: session expired or insufficient permissions. Re-login and update cookie.".to_string(),
    }
}
```

**Step 2: Update README**

Add to the Features section (both EN and ZH):

**English:**
```markdown
- **Cloud Share URL Import** — Pass Kingsoft Docs / WPS (`kdocs.cn`) share links directly; downloads via session cookie. Use `--kdocs-cookie` or `KDOCS_COOKIE` env var.
```

Add to CLI Options table:
```markdown
| `--kdocs-cookie` | — | Cookie for Kingsoft Docs (kdocs.cn) share URL downloads only |
```

Add example:
```bash
# Import from a WPS cloud share link
export KDOCS_COOKIE='wps_sid=...; ...'
grep_excel 'https://www.kdocs.cn/l/xxxx' -q "search term"

# Or pass cookie directly
grep_excel --kdocs-cookie "$KDOCS_COOKIE" 'https://www.kdocs.cn/l/xxxx' -t
```

**中文:**
```markdown
- **云文档链接导入** — 直接传入金山文档 (kdocs.cn) 分享链接；通过登录 Cookie 下载。使用 `--kdocs-cookie` 或 `KDOCS_COOKIE` 环境变量。
```

Add to CLI 选项 table:
```markdown
| `--kdocs-cookie` | — | 金山文档 (kdocs.cn) 分享链接下载专用 Cookie |
```

**Step 3: Commit**

```bash
git add crates/core/src/i18n.rs README.md
git commit -m "docs: add cloud share URL feature docs + i18n strings"
```

---

### Task 7: Build Verification + Diagnostics

**Step 1: Build all feature combinations**

```bash
# Default (no share-url)
cargo build

# With share-url
cargo build --features share-url

# Full (includes share-url)
cargo build --features full

# Share-url + mcp-server
cargo build --features "share-url mcp-server"
```

Expected: all compile without errors.

**Step 2: Run all tests**

```bash
cargo test
cargo test --features share-url
```

Expected: all pass.

**Step 3: Run lsp_diagnostics on changed files**

Check diagnostics for:
- `crates/core/src/source.rs`
- `crates/cli/src/main.rs`
- `crates/cli/src/mcp.rs`
- `crates/cli/src/lib.rs`
- `crates/core/src/i18n.rs`

Expected: no errors (warnings OK for unused code behind cfg).

**Step 4: Manual smoke test (if KDOCS_COOKIE available)**

```bash
export KDOCS_COOKIE='...'  # from test account
cargo run --features share-url -- 'https://www.kdocs.cn/l/catuXnZRIB58' -t
```

Expected: lists tables from the downloaded xlsx.

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat: cloud share URL import (Kingsoft Docs / WPS kdocs.cn)"
```

---

## Summary

| Task | Description | Key files |
|------|-------------|-----------|
| 1 | Deps + feature flag | Cargo.toml, crates/{core,cli}/Cargo.toml |
| 2 | source.rs: classify + provider | crates/core/src/source.rs |
| 3 | source.rs: download + tempfile | crates/core/src/source.rs (feature-gated) |
| 4 | CLI wiring | crates/cli/src/main.rs |
| 5 | MCP wiring | crates/cli/src/mcp.rs |
| 6 | i18n + README | crates/core/src/i18n.rs, README.md |
| 7 | Verify | build + test + diagnostics |

**Execution order:** Tasks 1→2→3 are sequential (deps → module → download). Task 4 depends on 3. Task 5 depends on 4. Tasks 6 and 7 are final.
