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
///
/// Hosts are data (not hardcoded logic) — extend via config in future phases.
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
    pub office_download_template: &'static str,
    /// Fallback: drive links meta API template.
    pub drive_links_template: &'static str,
    /// Fallback: drive file download API template.
    pub drive_download_template: &'static str,
    /// Origin header template. `{host}` → URL host.
    pub origin_template: &'static str,
    /// Referer header template. `{host}` and `{sid}` substituted.
    pub referer_template: &'static str,
}

/// Default Kingsoft Docs / WPS share provider.
///
/// Uses `{host}` in API templates so enterprise deployments on custom domains
/// work automatically. For host matching, `kdocs.cn` covers the public service.
/// Add enterprise domains via `SHARE_HOSTS` env var (comma-separated).
pub const KINGSOFT_SHARE: ShareProvider = ShareProvider {
    id: "kingsoft_share",
    hosts: &["kdocs.cn"],
    share_path_prefix: "/l/",
    office_download_template: "{scheme}://{host}/api/v3/office/file/{sid}/download",
    drive_links_template: "{scheme}://{host}/api/v5/links/{sid}",
    drive_download_template:
        "{scheme}://{host}/api/v5/groups/{groupid}/files/{fileid}/download?isblocks=false&support_checksums=md5,sha1",
    origin_template: "{scheme}://{host}",
    referer_template: "{scheme}://{host}/l/{sid}",
};

/// All built-in providers. Future: extend with user config.
pub const BUILTIN_PROVIDERS: &[ShareProvider] = &[KINGSOFT_SHARE];

/// Classify a user input into [`SourceKind`].
///
/// - Inputs without `://` are treated as local paths.
/// - `http://` or `https://` inputs are matched against providers.
/// - Unmatched URLs become [`SourceKind::UnsupportedRemote`].
pub fn classify_source(input: &str) -> SourceKind {
    let result = classify_with_providers(input, BUILTIN_PROVIDERS);
    if matches!(result, SourceKind::UnsupportedRemote { .. }) {
        if let Some(extra) = classify_extra_hosts(input) {
            return extra;
        }
    }
    result
}

/// Check if input matches a `SHARE_HOSTS` env var host with the same `/l/{sid}` pattern.
fn classify_extra_hosts(input: &str) -> Option<SourceKind> {
    #[cfg(feature = "share-url")]
    {
        let extra = std::env::var("SHARE_HOSTS").ok()?;
        let lower = input.to_ascii_lowercase();
        if !lower.starts_with("http://") && !lower.starts_with("https://") {
            return None;
        }
        let after_scheme = input.split_once("://")?.1;
        let idx = after_scheme.find('/')?;
        let (host_port, path_query) = (&after_scheme[..idx], &after_scheme[idx..]);
        let host = host_port.split(':').next()?;
        let path = path_query.split(['?', '#']).next().unwrap_or(path_query);

        for entry in extra.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if host_matches(host, &[entry]) {
                if let Some(sid) = extract_sid(path, KINGSOFT_SHARE.share_path_prefix) {
                    return Some(SourceKind::CloudShare {
                        provider: KINGSOFT_SHARE,
                        sid,
                        original_url: input.to_string(),
                    });
                }
            }
        }
        None
    }

    #[cfg(not(feature = "share-url"))]
    {
        None
    }
}

/// Classification with explicit provider list (for testing).
pub fn classify_with_providers(input: &str, providers: &[ShareProvider]) -> SourceKind {
    let lower = input.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return SourceKind::Local(PathBuf::from(input));
    }

    let after_scheme = input.split_once("://").map_or("", |x| x.1);
    let (host_port, path_query) = match after_scheme.find('/') {
        Some(idx) => (&after_scheme[..idx], &after_scheme[idx..]),
        None => (after_scheme, ""),
    };
    let host = host_port.split(':').next().unwrap_or(host_port);
    let path = path_query.split(['?', '#']).next().unwrap_or(path_query);

    for provider in providers {
        if host_matches(host, provider.hosts) {
            if let Some(sid) = extract_sid(path, provider.share_path_prefix) {
                #[cfg(feature = "share-url")]
                {
                    return SourceKind::CloudShare {
                        provider: provider.clone(),
                        sid,
                        original_url: input.to_string(),
                    };
                }
                #[cfg(not(feature = "share-url"))]
                {
                    let _ = sid;
                    return SourceKind::UnsupportedRemote {
                        url: input.to_string(),
                    };
                }
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

/// Extract share id from paths like `/l/{sid}`, `/l/{sid}/`, `/weboffice/l/{sid}`.
fn extract_sid(path: &str, prefix: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let idx = path.find(prefix)?;
    let after = &path[idx + prefix.len()..];
    let sid = after.split('/').next()?;
    if sid.is_empty() {
        return None;
    }
    if sid
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(sid.to_string())
    } else {
        None
    }
}

/// Extract host from a URL string (manual parse for MVP).
#[cfg(feature = "share-url")]
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://")?.1;
    let host_port = after_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;
    Some(host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_local_path() {
        assert!(matches!(classify_source("data.xlsx"), SourceKind::Local(_)));
        assert!(matches!(
            classify_source("/home/user/file.xlsx"),
            SourceKind::Local(_)
        ));
        assert!(matches!(
            classify_source("C:\\Users\\file.xlsx"),
            SourceKind::Local(_)
        ));
    }

    #[cfg(feature = "share-url")]
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

    #[cfg(feature = "share-url")]
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
        assert!(matches!(
            classify_source("https://www.kdocs.cn/doc/123"),
            SourceKind::UnsupportedRemote { .. }
        ));
    }

    #[cfg(feature = "share-url")]
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
        /// Downloaded temp file path + display name.
        /// `_guard` keeps the temp file alive until dropped.
        Downloaded {
            path: PathBuf,
            display_name: String,
            _guard: TempGuard,
        },
    }

    /// Keeps a temp file alive; deletes on drop.
    pub struct TempGuard {
        /// The temp file path. May be `None` if the guard was "leaked"
        /// (file left for OS cleanup — used when import runs in a separate thread).
        path: Option<PathBuf>,
    }

    impl TempGuard {
        /// Create a guard that will delete the file on drop.
        pub fn new(path: PathBuf) -> Self {
            Self { path: Some(path) }
        }

        /// Detach: don't delete on drop (temp file left for OS cleanup).
        /// Used when import runs in a spawned thread and timing is uncertain.
        pub fn detach(mut self) {
            self.path = None;
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            if let Some(ref path) = self.path {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Resolve a user input into a local path.
    ///
    /// - Local path → [`ResolvedSource::Local`] (no-op).
    /// - Cloud share URL → downloads to temp, returns [`ResolvedSource::Downloaded`].
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
                    anyhow!("{}", crate::i18n::share_needs_auth(&original_url))
                })?;
                download_share(&provider, &sid, &original_url, a)
            }
            SourceKind::UnsupportedRemote { url } => Err(anyhow!(
                "{}",
                crate::i18n::share_unsupported_url(&url)
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
        let host = extract_host(original_url).unwrap_or_default();
        let scheme = if original_url.starts_with("https://") {
            "https"
        } else {
            "http"
        };
        let api_url = provider
            .office_download_template
            .replace("{scheme}", scheme)
            .replace("{host}", &host)
            .replace("{sid}", sid);
        let origin = provider
            .origin_template
            .replace("{scheme}", scheme)
            .replace("{host}", &host);
        let referer = provider
            .referer_template
            .replace("{scheme}", scheme)
            .replace("{host}", &host)
            .replace("{sid}", sid);

        let debug = std::env::var("SHARE_DEBUG").unwrap_or_default() == "1";
        let insecure = std::env::var("SHARE_INSECURE").unwrap_or_default() == "1";
        if debug {
            eprintln!("[share-url] original_url: {}", original_url);
            eprintln!("[share-url] host: {}, scheme: {}", host, scheme);
            eprintln!("[share-url] api_url: {}", api_url);
            eprintln!("[share-url] origin: {}", origin);
            eprintln!("[share-url] referer: {}", referer);
            eprintln!("[share-url] cookie present: {} ({} chars)", !auth.cookie.is_empty(), auth.cookie.len());
            eprintln!("[share-url] insecure (skip TLS verify): {}", insecure);
        }

        let mut client_builder = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30));
        if insecure {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }
        let client = client_builder
            .build()
            .with_context(|| format!("Failed to build HTTP client for {}", api_url))?;

        let resp = client
            .get(&api_url)
            .header("Cookie", &auth.cookie)
            .header("Origin", &origin)
            .header("Referer", &referer)
            .header("User-Agent", "Mozilla/5.0 (compatible; grep-excel)")
            .send()
            .with_context(|| {
                format!(
                    "Failed to connect to share API: {}\n\
                     Set SHARE_DEBUG=1 for details. For self-signed certs: SHARE_INSECURE=1",
                    api_url
                )
            })?;

        let status = resp.status();
        if debug {
            eprintln!("[share-url] API response status: {}", status);
        }
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            if debug {
                eprintln!("[share-url] API error body: {}", truncate(&body, 500));
            }
            if body.contains("userNotLogin") || status == reqwest::StatusCode::FORBIDDEN {
                return Err(anyhow!(
                    "{}\nAPI: {} returned {}",
                    crate::i18n::share_auth_failed(),
                    api_url,
                    status
                ));
            }
            return Err(anyhow!(
                "Share API returned HTTP {} for:\n  {}\nBody: {}",
                status,
                api_url,
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
                     The share may not permit downloads, or the API changed.\n\
                     Response: {}",
                    truncate(&json.to_string(), 300)
                )
            })?;

        if !dl_url.starts_with("https://") && !dl_url.starts_with("http://") {
            return Err(anyhow!(
                "Share API returned an unsafe download URL (expected http/https): {}",
                truncate(dl_url, 100)
            ));
        }

        // Try to get filename from response or default
        let display_name = json
            .get("fname")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("kdocs-{}.xlsx", sid));

        if debug {
            eprintln!("[share-url] download_url: {}", dl_url);
            eprintln!("[share-url] display_name: {}", display_name);
        }

        // Step 2: Download the actual file
        let file_resp = reqwest::blocking::get(dl_url)
            .context("Failed to download file from temporary URL")?;

        if !file_resp.status().is_success() {
            return Err(anyhow!(
                "File download failed: HTTP {}",
                file_resp.status()
            ));
        }

        let bytes = file_resp.bytes().context("Failed to read file bytes")?;

        if bytes.is_empty() {
            return Err(anyhow!("Downloaded file is empty"));
        }

        // Step 3: Write to temp file with user-only permissions
        let ext = std::path::Path::new(&display_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("xlsx");

        let prefix_sid = &sid[..sid.len().min(16)];
        let tmp_prefix = format!("grep-excel-share-{}-", prefix_sid);
        let tmp_suffix = format!(".{}", ext);

        let mut tmp_builder = tempfile::Builder::new();
        tmp_builder.prefix(&tmp_prefix).suffix(&tmp_suffix);

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

        let (file, path) = tmp.keep().context("Failed to finalize temp file")?;
        drop(file);

        Ok(ResolvedSource::Downloaded {
            path: path.clone(),
            display_name,
            _guard: TempGuard::new(path),
        })
    }

    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            let truncated: String = s.chars().take(max).collect();
            format!("{}...(truncated)", truncated)
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
        fn temp_guard_deletes_on_drop() {
            let dir = std::env::temp_dir();
            let path = dir.join("grep-excel-test-guard-deletion.txt");
            std::fs::write(&path, b"test").unwrap();
            assert!(path.exists());

            {
                let _guard = TempGuard::new(path.clone());
                assert!(path.exists());
            }
            assert!(!path.exists());
        }

        #[test]
        fn temp_guard_detach_does_not_delete() {
            let dir = std::env::temp_dir();
            let path = dir.join("grep-excel-test-guard-detach.txt");
            std::fs::write(&path, b"test").unwrap();
            assert!(path.exists());

            {
                let guard = TempGuard::new(path.clone());
                guard.detach();
            }
            assert!(path.exists());
            std::fs::remove_file(&path).ok();
        }
    }
}
