# Skill: i18n & Localization

grep-excel mandates bilingual support (Chinese and English) for all user-facing strings. This skill covers the i18n conventions, how to add new strings, and what the PR checklist requires.

## Prerequisites

- Understanding of grep-excel's `i18n.rs` module (`crates/core/src/i18n.rs`)
- Knowledge of where user-facing strings appear (CLI, TUI, MCP, errors, REPL)

## Architecture

The `i18n` module provides:

```rust
// Language enum
pub enum Lang { Zh, En }

// Language detection (called once at startup)
pub fn init();

// Get current language
fn current() -> Lang;

// Check if current language is Chinese
pub fn is_zh() -> bool;
```

### Language Detection

At startup, `i18n::init()` is called from `main()`. It detects the language by checking
environment variables in order:

1. `LANG`
2. `LC_ALL`
3. `LC_MESSAGES`

If any of these starts with `"zh"`, the language is set to Chinese. Otherwise, English.

```bash
# Chinese
LANG=zh_CN.UTF-8 grep_excel --help

# English
LANG=en_US.UTF-8 grep_excel --help
```

## Adding New Strings

### Simple Static String

```rust
// In i18n.rs
pub fn my_new_label() -> &'static str {
    match current() {
        Lang::Zh => "中文标签",
        Lang::En => "English Label",
    }
}
```

Usage:
```rust
use grep_excel_core::i18n;

println!("{}", i18n::my_new_label());
```

### Parameterized String (with format args)

```rust
// In i18n.rs
pub fn file_imported_count(count: usize) -> String {
    match current() {
        Lang::Zh => format!("已导入 {} 个文件", count),
        Lang::En => format!("Imported {} files", count),
    }
}
```

Usage:
```rust
println!("{}", i18n::file_imported_count(5));
```

### CLI Help Text

CLI help strings use `clap`'s help attribute. Since these are static strings evaluated at compile time, they use the `#[cfg]` pattern or call `i18n` functions:

For dynamic help text, call `i18n` functions in the `main()` function when building the `clap::Command`.

## Where i18n Is Used

| Area | Location | Notes |
|---|---|---|
| CLI help text | `main.rs` — `Args` struct `help` attributes | Some static, some dynamic |
| TUI labels | `app/render.rs`, `app/ui.rs` | All button labels, status messages, hints |
| MCP tool descriptions | `mcp.rs` — `#[tool(description = ...)]` | Must be bilingual |
| Error messages | Various `anyhow!()` calls | User-facing errors only; internal/debug errors can be English |
| REPL commands | `interactive.rs` | Dot-command help text, prompt messages |

## Rules

### Hard Requirements (PR will be rejected if violated)

1. **All user-facing strings MUST go through `i18n`.** Never hardcode Chinese or English directly in UI code.
2. **New strings require both `Zh` and `En` arms.** Every function in `i18n.rs` must return valid strings for both languages.
3. **Error messages visible to end users must be bilingual.** Internal errors only visible in `RUST_LOG=debug` can stay English.

### Guidelines

- **Prefer `&'static str`** for simple strings (no allocation)
- **Use `String` return type** only when `format!()` is needed
- **Keep translations concise**: Match the tone and length of existing strings
- **Chinese strings use natural phrasing**: Not literal translations from English
- **English strings use technical language**: Clear, direct, no marketing fluff

## Code Pattern

```rust
// RECOMMENDED: Simple static label
pub fn search_mode_label() -> &'static str {
    match current() {
        Lang::Zh => "搜索模式",
        Lang::En => "Search mode",
    }
}

// RECOMMENDED: Parameterized message
pub fn rows_found(count: usize, duration_ms: u64) -> String {
    match current() {
        Lang::Zh => format!("找到 {} 行，耗时 {} 毫秒", count, duration_ms),
        Lang::En => format!("Found {} rows in {} ms", count, duration_ms),
    }
}

// ANTI-PATTERN: NEVER do this
// let label = if is_zh() { "搜索" } else { "Search" };  ← WRONG
// Always define in i18n.rs and call the function.
```

## Testing

```bash
# Test Chinese UI
LANG=zh_CN.UTF-8 cargo run -p grep-excel -- --help

# Test English UI
LANG=en_US.UTF-8 cargo run -p grep-excel -- --help

# Verify TUI renders correctly in both languages
LANG=zh_CN.UTF-8 cargo run -p grep-excel
LANG=en_US.UTF-8 cargo run -p grep-excel
```

For automated tests, check that i18n functions return non-empty strings for both languages:

```rust
#[test]
fn test_i18n_strings_not_empty() {
    grep_excel_core::i18n::init();
    // Test a sampling of i18n functions
    assert!(!my_label().is_empty());
}
```

## PR Checklist

When submitting a PR that adds user-facing text:

- [ ] New strings defined in `i18n.rs` with both `Zh` and `En` arms
- [ ] `LANG=zh_CN.UTF-8` and `LANG=en_US.UTF-8` both show correct text
- [ ] Chinese and English strings both read naturally (not machine-translated)
- [ ] No hardcoded strings in UI code (`app/`, `main.rs`, `mcp.rs`)

## See Also

- [Adding File Format Support](./adding-file-format-support.md) — parsers may need i18n for format names
- [MCP Tool Development](./mcp-tool-development.md) — tool descriptions must be bilingual
- [Developer Guide: i18n](../docs/DeveloperGuide.md#internationalization-i18n)
