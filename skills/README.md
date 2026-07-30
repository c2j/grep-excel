# Skills

Practical guides for getting the most out of grep-excel, organized by role.

## For Users

Step-by-step tutorials for common real-world tasks.

| Skill | Description |
|-------|-------------|
| [Analyzing Database Exports](./analyzing-database-exports.md) | Import multi-table DB dumps, JOIN across files, aggregate, and export results |
| [Debugging Corrupted Files](./debugging-corrupted-files.md) | Recover data from damaged Excel files with `--repair` |
| [Using with AI Assistants](./using-with-ai-assistants.md) | Integrate grep-excel MCP with Claude Desktop or Cursor |
| [Batch Processing Pipelines](./batch-processing-pipelines.md) | Automate workflows with `--exec` pipelines and `--run` shell commands |
| [Working with Archives](./working-with-archives.md) | Search inside ZIP, TAR, and split-volume archives without decompression |

## For Developers

Implementation guides for extending grep-excel.

| Skill | Description |
|-------|-------------|
| [Adding File Format Support](./adding-file-format-support.md) | Add a new file format parser (step-by-step with PDF as example) |
| [MCP Tool Development](./mcp-tool-development.md) | Build a new MCP tool — from params struct to `--exec` dispatch |
| [i18n & Localization](./i18n-and-localization.md) | Add Chinese/English strings following the mandatory i18n convention |

## See Also

- [User Guide](../docs/UserGuide.md) — comprehensive end-user documentation
- [Developer Guide](../docs/DeveloperGuide.md) — architecture, `SearchEngine` trait, and extension points
- [Contributing Guide](../CONTRIBUTING.md) — development setup, conventions, and PR checklist
- [README](../README.md) — project overview and feature summary
