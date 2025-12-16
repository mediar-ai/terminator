# Terminator - AI-Native GUI Automation

Open-source desktop automation (MIT). Gives AI hands to control any app on Windows.
**Mediar AI** | [$2.8M seed](https://x.com/louis030195/status/1948745185178914929) | [mediar.ai](https://mediar.ai)

## Installation

**Windows (Primary Platform):**
```bash
npx @mediar-ai/cli --help  # Run without install
npm i -g @mediar-ai/cli    # Or install globally
```

## Release

**CRITICAL**: Use `terminator` CLI only (syncs versions across workspace).

```bash
npx @mediar-ai/cli release           # Bump patch → tag → push (triggers CI/CD)
# Or with globally installed CLI:
terminator release
```

**Never manually edit versions in package.json or Cargo.toml.**

Git tags trigger: `publish-npm.yml` (npm), `publish-mcp.yml` (npm), `ci-wheels.yml` (PyPI).

## Development

```bash
git clone https://github.com/mediar-ai/terminator && cd terminator
cargo build && cargo test
cargo fmt && cargo clippy
```

## Critical Patterns

**TypeScript workflows:**
- Use `context.data = {...}` in final step for MCP integration
- See `examples/simple_notepad_workflow/` for best practices
- If CLI shows "(no parser)" → check `workflow_typescript.rs:379-398`

**Security:**
- ❌ **NEVER use `#id` selectors** (non-deterministic across machines)
- ❌ Don't commit credentials
- ✅ Use `role:Type && name:Name` or `nativeid` selectors

## MCP Debugging

**Logs:** `%LOCALAPPDATA%\claude-cli-nodejs\Cache\*\mcp-logs-terminator-mcp-agent\*.txt`

**Enable:** Set `LOG_LEVEL=debug` and `RUST_BACKTRACE=1` in MCP config.

## Packages

- npm: `@mediar-ai/terminator` (2 Windows platform packages), `terminator-mcp-agent`, `@mediar-ai/workflow`
- PyPI: `terminator`
- crates.io: `terminator-rs`

Repo: `crates/terminator*`, `packages/terminator-*/workflow`
