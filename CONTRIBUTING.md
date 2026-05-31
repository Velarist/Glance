# Contributing to Glance

Thanks for your interest in contributing. This document covers how to get started, what areas need help, and how to submit changes.

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.70+ | Install via [rustup](https://rustup.rs) |
| Node.js | 18+ | For VS Code extension |
| npm | 9+ | Comes with Node.js |

For the Zed extension, Rust must be installed via **rustup** specifically (not Homebrew) — Zed uses rustup internally to compile WASM.

---

## Project layout

```
glance/
├── src/                  Rust daemon (core)
│   ├── server/rpc.rs     JSON-RPC handler
│   ├── reader/           File reading, search, CSV parsing
│   └── index/            Line index + disk cache
└── extensions/
    ├── vscode/           VS Code extension (TypeScript)
    │   ├── src/          Extension host code
    │   └── media/        Webview (panel.js, panel.css)
    └── zed/              Zed extension (Rust → WASM)
```

---

## Setup

```bash
# Clone the repo
git clone https://github.com/<your-username>/glance
cd glance

# Build the daemon
cargo build

# Build the VS Code extension
cd extensions/vscode
npm install
npm run build
```

---

## Running locally

**Daemon (test via stdin):**
```bash
cargo build
echo '{"jsonrpc":"2.0","id":1,"method":"open","params":{"path":"/path/to/file.jsonl"}}' \
  | ./target/debug/glance
```

**VS Code extension (dev mode):**
```bash
cd extensions/vscode
npm run build
```
Then open `extensions/vscode` in VS Code and press `F5` to launch an Extension Development Host.

---

## Areas to contribute

### Good first issues
- Add more file format detection heuristics in `src/reader/mod.rs`
- Improve CSV column header detection (first row as header)
- Add line count to the index cache file for faster `info` queries

### Medium complexity
- JetBrains plugin (Kotlin) — see `extensions/` for VS Code reference
- Regex search with case-insensitive flag option
- Export search results to a new editor tab (VS Code)

### Hard / exploratory
- Persistent index across daemon restarts without re-scan on file change
- Columnar Parquet file support via Arrow
- Zed extension using language server instead of slash commands

---

## Code conventions

**Rust:**
- No `unsafe` blocks
- Errors propagated with `anyhow::Result` — no `unwrap()` in library code
- New file operations must go through `validate_path()` before accessing disk

**TypeScript:**
- No inline `onclick` HTML attributes — use event delegation on `#lines`
- All file content rendered in the webview must pass through `esc()` before insertion
- Keep `panel.js` free of ES modules — it runs as a plain inline script

**General:**
- No comments explaining *what* the code does — only *why* when non-obvious
- One concern per file: `daemon.ts` for RPC, `panel.ts` for panel lifecycle, `media/panel.js` for webview UI

---

## Submitting a pull request

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Run `cargo build` and verify the daemon compiles
4. Run `npm run build` in `extensions/vscode` and verify TypeScript compiles
5. Test manually against a real large file if the change touches file reading or the webview
6. Open a PR with a clear description of what changed and why

### PR checklist

- [ ] `cargo build` passes
- [ ] `npm run build` passes in `extensions/vscode`
- [ ] No new `unsafe` Rust code
- [ ] New file access paths use `validate_path()`
- [ ] Webview content goes through `esc()` before rendering

---

## Reporting bugs

Open an issue with:
- OS and VS Code version
- File format and approximate size
- Steps to reproduce
- Relevant output from `zed: open log` or VS Code Developer Tools console

---

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
