# Skill: MCP Tool Development

Add a new MCP (Model Context Protocol) tool to grep-excel. This is a 6-step process that touches the type system, engine trait, all engine implementations, the MCP server, and the CLI exec dispatcher.

## Prerequisites

- Rust knowledge and familiarity with the grep-excel codebase
- Understanding of the `SearchEngine` trait architecture
- Build: `cargo build -p grep-excel --features mcp-server`

## Architecture

```
AI Assistant (Claude/Cursor)
    │ stdio (JSON-RPC)
    ▼
rmcp Server (mcp.rs)
    │ #[tool] methods
    ▼
GrepExcelServer (Arc<RwLock<SyncDb>>)
    │ spawn_blocking
    ▼
SearchEngine trait → engine impls (memory / duckdb / sqlite)
```

The `--exec` CLI mode shares the same tool dispatch path:
```
CLI --exec JSON → exec_dispatch() → SearchEngine
```

## Step-by-Step: Adding `my_new_tool`

### Step 1: Define Parameters Struct

In `crates/core/src/types.rs`:

```rust
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "mcp-server", derive(schemars::JsonSchema))]
pub struct MyNewToolParams {
    #[cfg_attr(feature = "mcp-server", schemars(description = "Description for MCP schema"))]
    pub param1: String,
    pub param2: Option<usize>,
}
```

- `Deserialize` is always needed (for CLI JSON parsing)
- `schemars::JsonSchema` is conditional on `mcp-server` feature (generates MCP tool schema)
- `#[cfg_attr(...)]` patterns keep the struct clean when `mcp-server` is not enabled

### Step 2: Add Method to SearchEngine Trait

In `crates/core/src/engine/mod.rs`:

```rust
pub trait SearchEngine: Send {
    // ... existing methods ...

    fn my_new_operation(&mut self, param1: &str, param2: Option<usize>) -> Result<String>;
}
```

If the method has a sensible default implementation (e.g., "not supported" error),
provide it as a default `fn` body so existing engine implementations don't break.

### Step 3: Implement in All Engine Backends

**`crates/core/src/engine/memory.rs`, `duckdb.rs`, `sqlite.rs`:**

```rust
fn my_new_operation(&mut self, param1: &str, param2: Option<usize>) -> Result<String> {
    // Implementation specific to this engine
    // Memory engine: operate on in-memory data
    // DuckDB engine: use DuckDB SQL functions
    // SQLite engine: use SQLite SQL functions
    Ok(format!("Processed: {}", param1))
}
```

If a feature cannot be supported by a particular engine (e.g. session temp tables in the memory engine), return a clear `anyhow::bail!("... is not supported with the memory engine. Rebuild with --features engine-duckdb or engine-sqlite.")` error. This is an acceptable pattern — not every engine needs a full implementation.
```

### Step 4: Add #[tool] Handler

In `crates/cli/src/mcp.rs`, inside `impl GrepExcelServer`:

```rust
#[tool(description = "Short description of what this tool does.")]
pub async fn my_new_tool(
    &self,
    Parameters(params): Parameters<MyNewToolParams>,
) -> Result<String, String> {
    let db = Arc::clone(&self.db);
    tokio::task::spawn_blocking(move || {
        let mut guard = db.write();
        guard.0.my_new_operation(&params.param1, params.param2)
            .map(|result| serde_json::to_string_pretty(&result).unwrap())
            .map_err(|e| format!("Operation failed: {}", e))
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
```

**Thread safety notes:**
- Use `db.write()` for mutating operations, `db.read()` for read-only
- Wrap blocking engine calls in `tokio::task::spawn_blocking` to avoid blocking the async runtime
- Return JSON strings — the MCP protocol handles serialization

### Step 5: Add Exec Dispatch

In `crates/cli/src/main.rs`, in the `exec_dispatch()` function:

```rust
"my_new_tool" => {
    let p: MyNewToolParams = serde_json::from_value(params.clone())?;
    let result = db.my_new_operation(&p.param1, p.param2)?;
    Ok(serde_json::to_string_pretty(&result)?)
}
```

### Step 6: Update `--exec help`

In `print_exec_help()`, add the new tool to the output:

```rust
println!("  my_new_tool        - Short description");
println!("    param1 (string)   - Description of param1");
println!("    param2 (number)   - Description of param2 (optional)");
```

## Real Example: materialize_query

The `materialize_query` tool demonstrates this end-to-end pattern:

| Step | File | Key Detail |
|---|---|---|
| Params struct | `types.rs` → `MaterializeQueryParams` | `schemars(description)` for `name`, `sql`, `replace`, `max_rows` |
| Trait method | `engine/mod.rs` → `materialize_query()` | Signature: `fn materialize_query(&mut self, name: &str, sql: &str, replace: bool, max_rows: Option<usize>) -> Result<TempTableInfo>` |
| Engine impls | `duckdb.rs`, `sqlite.rs` (full); `memory.rs` (stub) | DuckDB/SQLite create temp tables; memory engine returns "not supported" error — acceptable for engine-limited features |
| MCP handler | `mcp.rs` → `#[tool] materialize_query` | `db.write()` with `spawn_blocking` |
| Exec dispatch | `main.rs` → `exec_dispatch()` match arm | Deserializes `MaterializeQueryParams` |
| Help | `print_exec_help()` | Listed with parameter descriptions |

## Verification

```bash
# Build with MCP support
cargo build -p grep-excel --features mcp-server

# Test the tool via --exec (no MCP server needed)
cargo run -p grep-excel --bin grep_excel -- data.xlsx \
  --exec '{"tool":"my_new_tool","params":{"param1":"test","param2":5}}'

# Test via MCP server (start server, then test with MCP inspector)
grep_excel --mcp

# Full test suite
cargo test -p grep-excel --features full
```

## Checklist

- [ ] Params struct in `types.rs` with `#[derive(Deserialize)]`
- [ ] Trait method in `engine/mod.rs`
- [ ] Implementation in all 3 engines (`memory.rs`, `duckdb.rs`, `sqlite.rs`) — stub with "not supported" error is acceptable for engine-limited features
- [ ] `#[tool]` handler in `mcp.rs`
- [ ] `exec_dispatch` match arm in `main.rs`
- [ ] `print_exec_help` entry
- [ ] Built and tested with `--exec`
- [ ] MCP tool table in `README.md` updated

## See Also

- [Adding File Format Support](./adding-file-format-support.md) — extend import capabilities
- [Developer Guide: MCP Tools](../docs/DeveloperGuide.md#mcp-tool-development) — detailed MCP architecture
- [Developer Guide: Type System](../docs/DeveloperGuide.md#type-system) — params struct conventions
