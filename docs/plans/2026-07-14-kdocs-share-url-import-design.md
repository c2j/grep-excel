# Design: Cloud Share URL Import (Kingsoft Docs / WPS)

> **Status:** Design — **Momus APPROVED** (2026-07-14)  
> **Date:** 2026-07-14  
> **Related verification:** Cookie-authenticated download of `https://www.kdocs.cn/l/catuXnZRIB58` succeeded end-to-end (office download API → xlsx → SHA1 match).

---

## 1. Problem Statement

Users often hold **cloud spreadsheet share links** (Kingsoft Docs / WPS 金山文档) rather than local files. `grep-excel` today only accepts **local filesystem paths** (`PathBuf` → `import_excel(&Path)` → calamine). There is no HTTP client or remote source layer.

**Goal:** Allow positional inputs (and MCP `import_file`) that are **share URLs** to be resolved into a temporary local `.xlsx` (or original extension when known), then imported through the existing pipeline for CLI / TUI / SQL REPL / MCP.

**Non-goal for MVP:** Cloud write-back, OAuth app registration, anonymous download without credentials, reverse-engineering WebOffice cell streaming as a primary path.

---

## 2. Verified Facts (Empirical)

Against share `sid=catuXnZRIB58` (`tmp1.xlsx`, 30732 bytes):

| Step | Anonymous | With login Cookie |
|------|-----------|-------------------|
| Share page HTML | Redirects to login SSO | N/A for CLI |
| `GET …/api/v5/links/{sid}` (drive) | Metadata OK | Metadata OK |
| Download APIs | `userNotLogin` | **OK** |
| Binary file | — | Valid xlsx; SHA1 matches meta |

**Working download paths (with Cookie + `Origin`/`Referer`):**

1. **Preferred (sid-direct):**  
   `GET https://{api_host}/api/v3/office/file/{sid}/download`  
   → JSON `{ "url" | "download_url", "status": "finished", "fize": N }`  
   → `GET` temporary object URL → file bytes.

2. **Fallback (drive):**  
   `GET …/api/v5/links/{sid}` → `fileinfo.id`, `fileinfo.groupid`, `fname`  
   `GET …/api/v5/groups/{gid}/files/{fid}/download?isblocks=false&support_checksums=md5,sha1`  
   → JSON `{ "url", "fsize", "checksums" }` → temp URL → bytes.

**CSRF:** `POST /api/v3/office/csrf_token` works but was **not required** for the download calls above when Cookie + Origin/Referer were present. Treat CSRF as optional retry enhancement, not MVP hard dependency.

**Implication:** Preview-in-browser ≠ downloadable without session. CLI must obtain a **session cookie** (or future official token).

---

## 3. Goals & Non-Goals

### 3.1 Goals (MVP)

1. Accept share URLs as file inputs alongside local paths.
2. Resolve share URL → tempfile → existing `import_excel`.
3. Credentials via (priority high→low):
   - CLI `--kdocs-cookie <STRING>` (name kept for UX; applies only to matched cloud-share sources)
   - Env `KDOCS_COOKIE` (alias `WPS_COOKIE`)
   - Config file (see §6)
4. **Do not hardcode a single production hostname.** Host matching and API base URLs must be **configurable**, with **built-in defaults** for common Kingsoft endpoints.
5. Clear, actionable errors (missing cookie, auth failure, forbidden download, network, invalid file).
6. Never log full cookie values.
7. Feature-flag optional HTTP stack if needed to keep default builds lean (decision in §8).

### 3.2 Non-Goals (MVP)

- Official KDocs Open Platform OAuth / app keys (document as Phase 2).
- Saving edits back to cloud.
- Headless browser automation.
- Parsing arbitrary third-party cloud products beyond the **configured provider profile** (MVP ships one profile: Kingsoft share links; architecture allows more profiles later).
- Guaranteeing download for every share permission combination (read-only preview without download ACL may fail by design).

### 3.3 Success Criteria

```bash
export KDOCS_COOKIE='…'   # or --kdocs-cookie
grep_excel 'https://www.kdocs.cn/l/<sid>' -t
# lists tables; same data as local tmp1.xlsx

grep_excel --kdocs-cookie "$COOKIE" 'https://kdocs.cn/l/<sid>' -q 'keyword'
# CLI search works

grep_excel a.xlsx 'https://www.kdocs.cn/l/<sid>' b.csv -i
# mixed local + remote; cookie only used for share URL entries
```

---

## 4. Design Principles

1. **Single choke point:** All entry points call `resolve_import_source` before `import_excel`.
2. **Local Path remains the engine contract:** Engines stay `import_excel(&Path, …)`.
3. **Provider profile, not hostname ifs scattered in code:** URL recognition and API templates live in a **ShareProvider** config (defaults + user override).
4. **Credentials are input to resolver, not globals in core:** CLI assembles `ShareAuth`; core download function takes explicit auth.
5. **Security by default in logging:** Redact secrets; warn in help about shell history.
6. **YAGNI:** One provider profile (Kingsoft share) in MVP; structure allows adding profiles without rewriting import paths.

---

## 5. Architecture

### 5.1 Data flow

```text
User input string (CLI FILES / MCP file_path / TUI path)
        │
        ▼
┌───────────────────────────┐
│ classify_source(input)    │  ← uses ShareProvider URL rules (configurable)
└───────────┬───────────────┘
            │
     ┌──────┴───────┐
     ▼              ▼
 LocalPath      CloudShare { provider_id, sid, original_url, api_base? }
     │              │
     │              ▼
     │      resolve_auth(cli/env/file)
     │              │
     │              ▼
     │      download_share(profile, sid, auth) → TempFile
     │              │
     └──────┬───────┘
            ▼
     import_excel(path) / import_excel_repair
            ▼
     existing CLI / TUI / SQL / MCP behavior
```

### 5.2 Module layout

| Component | Suggested location | Responsibility |
|-----------|-------------------|----------------|
| `source` module | `crates/core/src/source.rs` (or `crates/cli/src/source.rs` if HTTP kept out of core) | Classification, provider profiles, download, tempfile |
| `ShareProvider` | same | URL patterns, API templates, default hosts |
| `ShareAuth` | same | Cookie string only for MVP |
| CLI wiring | `crates/cli/src/main.rs` | `--kdocs-cookie`, call resolve before import |
| TUI wiring | `crates/cli/src/app/mod.rs` | Resolve when importing path-like strings that are URLs |
| MCP wiring | `crates/cli/src/mcp.rs` | `import_file` resolves `file_path` if share URL |
| Config load | `crates/cli` or core | Read cookie file + optional provider override file |
| i18n | `crates/core/src/i18n.rs` | Error/help strings EN/ZH |

**Placement recommendation:** Put download + provider in **`crates/core`** behind feature `share-url` (name TBD), so Desktop/Tauri can reuse without duplicating. CLI-only is acceptable for MVP if core dependency policy prefers zero HTTP in core — then export a thin trait/`resolve` from cli and document Tauri follow-up. **Prefer core + feature** for one implementation.

### 5.3 Dependency

- `reqwest` with `blocking` + `rustls-tls` (or project-standard TLS) for sync import paths.
- Optional: `url` crate for parsing (or `reqwest::Url` / std).
- `tempfile` crate for owned temp paths with reliable cleanup.

---

## 6. Host & URL Handling (No Hardcoded Single Host)

### 6.1 Problem with hardcoding

Production hosts observed or expected:

- `www.kdocs.cn`
- `kdocs.cn` (redirects)
- Possible enterprise / regional / CDN frontends (e.g. custom domains, `*.kdocs.cn`, future `wps.cn` share fronts)
- API hosts may differ from page hosts (`drive.kdocs.cn` vs `www.kdocs.cn`)

Hardcoding `www.kdocs.cn` only will break valid user links and enterprise deployments.

### 6.2 ShareProvider profile (config-driven)

MVP ships a built-in profile **`kingsoft_share`** defined as data, not scattered string literals in business logic.

```rust
// Conceptual schema (not final code)
struct ShareProvider {
    /// Stable id, e.g. "kingsoft_share"
    id: String,

    /// Regex or structured matchers against full URL (scheme/host/path)
    /// Examples of default matchers (configurable):
    /// - host matches: `(^|\.)kdocs\.cn$` OR exact allow-list from config
    /// - path matches: `^/l/(?P<sid>[A-Za-z0-9_-]+)/?$`
    url_matchers: Vec<UrlMatcher>,

    /// Where to send office download API.
    /// May use placeholders: {sid}, {host}, {origin}
    /// Default template:
    ///   "https://www.kdocs.cn/api/v3/office/file/{sid}/download"
    /// Override examples:
    ///   "https://{host}/api/v3/office/file/{sid}/download"
    ///   "https://docs.example.com/api/v3/office/file/{sid}/download"
    office_download_url_template: String,

    /// Optional drive meta + download templates for fallback
    drive_links_url_template: Option<String>,
    drive_download_url_template: Option<String>,

    /// Headers
    origin_template: String,   // e.g. "https://{host}" or fixed override
    referer_template: String,  // e.g. "{original_url}" or "https://{host}/l/{sid}"

    /// Which auth modes this provider accepts
    auth: AuthKind, // Cookie for MVP
}
```

### 6.3 Default matchers (built-in, overridable)

Defaults **seed** the profile but live in one constant/config block:

| Rule | Default |
|------|---------|
| Scheme | `http` or `https` |
| Host | Regex: `(?i)^(www\.)?kdocs\.cn$` **plus** optional suffix rule `(?i)^(.+\.)?kdocs\.cn$` if we choose subdomain-wide match — **document the choice** |
| Path | `^/l/(?P<sid>[A-Za-z0-9_-]+)/?$` (query/fragment ignored for sid) |
| Sid capture | Named group `sid` |

**Recommended default host policy:**

1. Match host equal to `kdocs.cn` or `www.kdocs.cn` (case-insensitive).
2. **Also** match any host ending with `.kdocs.cn` (subdomains), unless disabled by config.
3. **Do not** match arbitrary hosts unless listed in user config `extra_hosts` / custom profile.

User config file (optional) example:

```toml
# ~/.config/grep-excel/share_providers.toml  (optional)

[providers.kingsoft_share]
# Replace or extend host matching
extra_hosts = ["docs.mycompany.com", "kdocs.internal.example"]
# If set, used as API origin instead of deriving from URL host
api_base = "https://www.kdocs.cn"
# Full template override (optional)
office_download_url_template = "https://www.kdocs.cn/api/v3/office/file/{sid}/download"
```

**API base resolution order for a matched URL:**

1. Provider `api_base` override from config (if set).
2. Else if template contains `{host}`, substitute **URL’s host** (or normalized host).
3. Else use template’s fixed default host in the built-in profile string (still a single data field, not magic in download code).

This way enterprise users can point API to a known gateway while still opening links from multiple front-door hosts.

### 6.4 Classification algorithm

```text
fn classify(input: &str, providers: &[ShareProvider]) -> SourceKind:
  if looks_like_url(input):  // has scheme http(s):// OR parseable as URL
     for provider in providers:
        if let Some(cap) = provider.try_match(input):
           return CloudShare { provider, sid: cap.sid, original_url: normalized }
     // http(s) but no provider matched:
     return UnsupportedRemote { url }  // clear error: not a configured share URL
  else:
     return LocalPath(PathBuf::from(input))
```

**Important:** Unmatched `https://…` must **not** be treated as a local path (would fail confusingly). Error: “URL is not a configured cloud share link; download the file locally or add a provider host in config.”

### 6.5 What is intentionally not hardcoded in logic

| Item | Where it lives |
|------|----------------|
| Host allow-list / regex | Provider config + defaults table |
| API path templates | Provider config |
| Origin/Referer templates | Provider config |
| Cookie header name | Constant `Cookie` (HTTP standard) — OK to hardcode |
| Temp file prefix | Constant `grep-excel-share-` — OK |

---

## 7. Authentication

### 7.1 MVP: Session Cookie

`ShareAuth::Cookie(String)` — raw `Cookie` header value (as browser sends).

**Resolution order:**

1. `--kdocs-cookie <STRING>` (CLI only; highest)
2. `KDOCS_COOKIE` env
3. `WPS_COOKIE` env (alias)
4. Cookie file: platform config dir  
   - macOS: `~/Library/Application Support/grep-excel/kdocs_cookie`  
   - Linux: `~/.config/grep-excel/kdocs_cookie`  
   - Windows: `%APPDATA%\grep-excel\kdocs_cookie`  
   File content: single line, cookie header body only; recommend mode `0600` on Unix (warn if world-readable).

If multiple share URLs in one invocation, **one** resolved cookie applies to all (MVP). Per-URL cookies out of scope.

### 7.2 CLI flag semantics

| Flag | `--kdocs-cookie <STRING>` |
|------|---------------------------|
| Short flag | None (reduce accidental exposure / ambiguity) |
| Applies to | Only inputs classified as cloud share for providers that use cookie auth |
| Ignored when | All inputs are local paths |
| Help text | EN/ZH: only for Kingsoft/WPS cloud share URLs; security warning |

Optional later: `--kdocs-cookie-file <PATH>` to avoid argv exposure — **Phase 1.1**, not blocking MVP if env/file already exist.

### 7.3 Security requirements

1. Never print cookie in logs, panic messages, or MCP tool results.
2. Help/docs warn: shell history stores argv; prefer env or cookie file.
3. Redact in `Debug` impls (`***`).
4. Temp files: user-only permissions when OS allows (`0o600`).
5. Document that cookie equals account session; use test accounts when possible.

### 7.4 Phase 2 auth (document only)

- Official Open Platform `access_token` + `file_token`
- Browser cookie export helpers  
Not in MVP implementation plan beyond interface seam: `enum ShareAuth { Cookie(String), Bearer(String) }`.

---

## 8. Download Procedure (Kingsoft profile)

### 8.1 Primary

```text
GET {office_download_url_template with sid}
Headers:
  Cookie: {auth}
  User-Agent: grep-excel/{version} (compatible; desktop-like UA optional)
  Origin: {origin_template}
  Referer: {referer_template}
Timeout: connect 10s, total 120s (configurable constants)
Response JSON:
  extract first non-empty of: url, download_url
  optional: fize/fsize, checksums.sha1
GET temp_url (follow redirects)
Write to tempfile with extension .xlsx (or from Content-Disposition filename if safe)
Validate:
  - non-empty
  - size matches fsize if present (warn on mismatch, hard-fail if zero)
  - optional: zip magic / [Content_Types].xml for xlsx
  - optional: sha1 if provided
Return path + TempGuard
```

### 8.2 Fallback

On primary failure (404/non-JSON/missing url), if drive templates configured:

1. GET links meta by sid  
2. Read `fileinfo.id`, `groupid`, `fname`  
3. GET group file download with `isblocks=false&support_checksums=md5,sha1`  
4. Same temp URL fetch + validate  

### 8.3 Error mapping

| Condition | User-facing error (i18n key) |
|-----------|------------------------------|
| No cookie configured | Missing credentials; show flag/env/file |
| HTTP 401/403 or body `userNotLogin` | Session expired / not logged in |
| JSON ok but no url | Download not permitted or API changed |
| Network/TLS/timeout | Network error with host (no cookie) |
| Invalid xlsx after download | Corrupted or non-spreadsheet payload |
| Unmatched https URL | Not a configured share provider |

---

## 9. Integration Points

### 9.1 CLI (`main.rs`)

- Add `kdocs_cookie: Option<String>` to clap `Args`.
- Replace raw `PathBuf` assumption for each `FILES` entry:
  - Keep `files: Vec<String>` **or** keep `PathBuf` but also accept strings that are URLs (clap `PathBuf` can hold URL-like strings on Unix; **prefer `Vec<String>`** for clarity and Windows safety).
- Shared helper:

```text
fn resolve_and_import(db, input: &str, auth: &ShareAuthResolve, repair: bool) -> Result<FileInfo>
```

Used by: `run_cli`, `run_sql_cli`, `run_interactive_cli`, `run_tui` pre-import, `--exec` auto-import.

- `file.exists()` checks must run **only** on local paths after classification.

### 9.2 TUI

- Preload from args: resolve share URLs before `app.import_file`.
- File dialog remains local-only.
- Optional later: paste URL in UI — out of MVP unless trivial.

### 9.3 MCP / `--exec`

- `import_file` `file_path` may be share URL.
- Auth from env/file (and optional param `kdocs_cookie` in tool schema — **optional MVP+**).
- Document that MCP servers should set env in config, not put cookies in chat logs.

### 9.4 Display names

- Imported `FileInfo.name` should prefer:
  1. `Content-Disposition` / drive `fname` if known (`tmp1.xlsx`)
  2. Else `kdocs-{sid}.xlsx`
- Avoid using full URL as table alias (ugly, special chars).

### 9.5 Tempfile lifetime

- Hold `TempPath`/`NamedTempFile` in a session-scoped `Vec` on CLI process or `App` so file survives for search/edit until process exit.
- On Drop, delete temp files.
- `save` / `save_as`: saving **overwrites temp or new local path only**; never upload to cloud (document clearly). If user `save` on cloud-origin file, write local path and message: “saved locally; cloud not updated.”

---

## 10. Feature Flag & Packaging

| Option | Pros | Cons |
|--------|------|------|
| Always-on `reqwest` | Simplest UX | Larger binary |
| Feature `share-url` | Opt-in size | Users must build with feature |

**Recommendation:** Feature `share-url` enabled in `full` and documented; consider enabling in default if binary size acceptable after measurement. Implementation plan should `cargo build --release` size note.

---

## 11. CLI UX Summary

```text
grep_excel [OPTIONS] [SOURCES]...

SOURCES: local paths and/or cloud share URLs matched by configured providers

--kdocs-cookie <STRING>
    Session Cookie header value for Kingsoft/WPS cloud share downloads only.
    Prefer KDOCS_COOKIE env or config file to avoid shell history exposure.
```

Examples in README (EN/ZH):

```bash
# Env (recommended)
export KDOCS_COOKIE='wps_sid=…; …'
grep_excel 'https://www.kdocs.cn/l/xxxx' -t

# Flag
grep_excel --kdocs-cookie "$KDOCS_COOKIE" 'https://kdocs.cn/l/xxxx' -q 'foo'

# Mixed
grep_excel data.xlsx 'https://www.kdocs.cn/l/xxxx' -i
```

---

## 12. Testing Plan

### 12.1 Unit tests (no network)

- URL classification: `kdocs.cn`, `www.kdocs.cn`, subdomain, wrong host, path without `/l/`, local paths, Windows paths.
- Template rendering: `{sid}`, `{host}`, `{origin}`.
- Auth resolution order (mock env/file).
- JSON url extraction from fixture responses (office + drive shapes).
- Error mapping from fixture status/bodies.
- Cookie redaction in Debug.

### 12.2 Integration tests (optional, ignored by default)

- `#[ignore]` network test behind env `KDOCS_COOKIE` + `KDOCS_TEST_SID`.
- Assert download size > 0 and calamine opens.

### 12.3 Manual verification checklist

- [ ] Known share with cookie: `-t` lists sheets  
- [ ] No cookie: actionable error  
- [ ] Expired cookie: login error  
- [ ] Local path still works without cookie  
- [ ] Mixed local + share  
- [ ] Custom `extra_hosts` / `api_base` config (if implemented in MVP; else Phase 1.1)

---

## 13. Implementation Phases

### Phase 0 — Design approval (this doc)

Momus review → revise → accept.

### Phase 1 — MVP

1. Provider defaults + classify + cookie auth resolve  
2. Office download primary path + tempfile  
3. Wire CLI (`String` sources + `--kdocs-cookie`)  
4. Wire SQL/REPL/TUI preload + MCP import_file  
5. i18n errors + README  
6. Unit tests  

### Phase 1.1 — Hardening

- Drive API fallback  
- `--kdocs-cookie-file`  
- Optional `share_providers.toml` (`extra_hosts`, `api_base`)  
- SHA1 verify when present  
- Size measurement / feature default decision  

### Phase 2 — Future

- Official Open API tokens  
- Generic direct `https://…xlsx` download (separate provider)  
- TUI paste-URL  
- Cookie refresh / CSRF retry if APIs tighten  

---

## 14. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Undocumented APIs change | Isolate in provider templates; fallback path; clear “API changed” error |
| Cookie in shell history | Docs + prefer env/file; no short flag |
| Enterprise custom domains | Config `extra_hosts` / `api_base` (1.1) |
| Temp file left behind | `tempfile` + Drop; unique names |
| Legal/ToS of session cookie automation | Document personal/test use; official API Phase 2 |
| Matching too broad (any `.kdocs.cn`) | Document default; allow tightening via config |

---

## 15. Open Questions (for review)

1. **Default host breadth:** only `kdocs.cn`+`www.kdocs.cn`, or all `*.kdocs.cn`?  
   - **Proposal:** `kdocs.cn`, `www.kdocs.cn`, and `*.kdocs.cn` subdomains; everything else via `extra_hosts`.
2. **Feature default on/off?**  
   - **Proposal:** `share-url` in `full`; measure then consider default.
3. **Rename flag** to `--share-cookie` for provider-neutrality vs keep `--kdocs-cookie` for user clarity?  
   - **Proposal:** Keep `--kdocs-cookie` in MVP (user-facing clarity); internally `ShareAuth`.
4. **Core vs CLI for HTTP?**  
   - **Proposal:** `crates/core` + feature `share-url`.

---

## 16. Out-of-Scope Explicit Checklist

- [ ] Hardcoded-only `www.kdocs.cn` with no override path  
- [ ] Cloud save/upload  
- [ ] OAuth device flow in MVP  
- [ ] Scraping `window.__WPSENV__` as primary  
- [ ] Using analytics hosts (`shuc-js.ksord.com`) for anything  

---

## 17. References

- Empirical download: `GET /api/v3/office/file/{sid}/download` with session Cookie  
- Drive fallback: `/api/v5/links/{sid}`, `/api/v5/groups/{gid}/files/{fid}/download?isblocks=false&support_checksums=md5,sha1`  
- Existing import choke point: `SearchEngine::import_excel(&Path, …)` in `crates/core/src/engine/mod.rs`  
- CLI entry: `crates/cli/src/main.rs` (`files`, `import_file_with_repair`)

---

## 18. Approval

| Role | Status |
|------|--------|
| Author | Draft 2026-07-14 |
| Momus review | **APPROVE** (2026-07-14) — references verified; no blocking issues |
| Implementation | Not started |

**Next:** Write implementation plan (`writing-plans`), then optional git worktree + Phase 1 implementation.

### Momus notes (non-blocking)

- All codebase references in §17 verified exact.
- Prefer `files: Vec<String>` over `PathBuf` for URL-safe CLI inputs (§9.1).
- Open questions §15 already have proposals; implement with those defaults unless product overrides.
