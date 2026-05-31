# Glance

> **Status:** Early development. The core daemon and VS Code extension work. The Zed extension compiles but is experimental.

Open large files (JSONL, CSV, logs 3GB+) in your IDE without crashing — a lightweight daemon + editor extensions.

## How it works

```
VS Code / JetBrains              Zed
        │                              │
        │  JSON-RPC 2.0 over stdio     │  slash command (/glance)
        ▼                              ▼
  glance daemon (Rust)      WASM extension (self-contained)
  ├── Line index                ├── Line index (built in WASM via WASI fs)
  ├── Reader                    ├── Reader (std::fs, no daemon needed)
  └── Search                    └── Search (streaming in WASM)
```

**VS Code:** implemented and tested — the daemon is spawned automatically when you open a file.

**Zed:** implemented as a slash command (`/glance`) in the AI Assistant panel. Works when compiled, but Zed's extension API does not support custom panels, so the experience is more limited than VS Code.

**JetBrains:** not yet implemented (roadmap).

## Supported formats

| Extension | Format |
|---|---|
| `.jsonl`, `.ndjson` | JSONL / Newline-delimited JSON |
| `.csv`, `.tsv` | Delimited text |
| `.log`, anything else | Raw text |

Format is detected by extension first, then confirmed by sniffing the first line of the file. A `.log` file whose first line is a JSON object will be treated as JSONL automatically.

## Build

Requires Rust 1.70+.

```bash
# debug
cargo build

# release (optimized)
cargo build --release
```

Binary output: `target/release/glance`

```bash
# check version
./target/release/glance --version
```

## Usage

The daemon speaks JSON-RPC 2.0 over stdio — one request per line, one response per line.

```bash
# start daemon manually (for testing)
./target/release/glance
```

All requests accept an optional `"version": 1` field. If the version does not match the daemon, a warning is logged but the request is still processed.

Multiple files can be open simultaneously — each `open` call returns an independent `file_id`.

### Methods

**open** — index a file, returns a `file_id`
```json
{"jsonrpc":"2.0","id":1,"method":"open","params":{"path":"/data/events.jsonl"}}
```
```json
{"jsonrpc":"2.0","id":1,"result":{"file_id":1,"total_lines":5000000,"file_size":3221225472,"format":"jsonl"}}
```

**read** — fetch lines by offset (0-indexed)
```json
{"jsonrpc":"2.0","id":2,"method":"read","params":{"file_id":1,"offset":0,"limit":200}}
```

Optional: `"pretty": true` pretty-prints each line's JSON content (JSONL files only).
```json
{"jsonrpc":"2.0","id":2,"method":"read","params":{"file_id":1,"offset":0,"limit":50,"pretty":true}}
```

**read** response includes `fields: string[]` per line for CSV/TSV files, ready for column rendering.

**search** — substring search (case-insensitive by default)
```json
{"jsonrpc":"2.0","id":3,"method":"search","params":{"file_id":1,"query":"error","max_results":100}}
```

Optional: `"regex": true` switches to regex mode (case-sensitive). Returns an error immediately if the pattern is invalid.
```json
{"jsonrpc":"2.0","id":3,"method":"search","params":{"file_id":1,"query":"error\\d+","regex":true,"max_results":100}}
```

**count** — count matching lines without loading results into memory (O(1) RAM regardless of file size)
```json
{"jsonrpc":"2.0","id":4,"method":"count","params":{"file_id":1,"query":"error"}}
```
```json
{"jsonrpc":"2.0","id":4,"result":{"count":18432}}
```

Also supports `"regex": true`.

**info** — file metadata
```json
{"jsonrpc":"2.0","id":4,"method":"info","params":{"file_id":1}}
```

**close** — release file handle
```json
{"jsonrpc":"2.0","id":5,"method":"close","params":{"file_id":1}}
```

## VS Code extension ✓ working

```
extensions/vscode/
├── package.json
├── tsconfig.json
├── bin/
│   └── glance          daemon binary (bundled)
├── media/
│   ├── panel.css          webview styles
│   └── panel.js           webview JavaScript (event-delegation, no inline handlers)
└── src/
    ├── extension.ts       entry point (activate/deactivate)
    ├── daemon.ts          GlanceDaemon class + RPC types
    └── panel.ts           panel creation + message handling
```

**Install (VSIX):**

```bash
cd extensions/vscode
npm install
npm run build
npx vsce package --no-dependencies
```

Then in VS Code: `Cmd+Shift+P` → `Extensions: Install from VSIX` → select `glance-0.1.0.vsix`.

Or install from terminal once `code` CLI is on PATH:
```bash
code --install-extension glance-0.1.0.vsix
```

**Usage:**

- Right-click any `.jsonl`, `.csv`, `.tsv`, or `.log` file in the Explorer → **Glance: Open Large File**
- Or `Cmd+Shift+P` → `Glance: Open Large File`

**Features in the panel:**

| Feature | How |
|---|---|
| Pagination | `← Prev` / `Next →` buttons — 200 lines per page (10 per page in pretty mode) |
| Search | Live substring search (case-insensitive), debounced 300ms |
| Regex search | Toggle `.*` button or `Ctrl+R` — case-sensitive, ReDoS-safe |
| Pretty-print JSON | Toggle `{}` button — expands each JSONL record into a readable card (JSONL only) |
| Pretty + search | When `{}` is active, search results also show as pretty JSON cards with match highlighted |
| Match navigation | `▲` / `▼` buttons next to search — jump to prev/next match in file context |
| Malformed lines | Collapsed by default with 150-char preview — click `▶ Show` to expand full content |
| Jump to line | Type line number in `Line #` field and press `Enter` |
| CSV column view | CSV/TSV renders as a scrollable table with column headers automatically |
| Copy line | Click any line number to copy that line's content to clipboard |
| Total match count | Shown separately alongside results, even when result list is truncated |
| Theme support | Panel colors follow the active VS Code theme automatically (light, dark, high contrast) |

**Keyboard shortcuts:**

| Key | Action |
|---|---|
| `Ctrl+F` | Focus search input |
| `Enter` | Next match (when search input is focused) |
| `Shift+Enter` | Previous match (when search input is focused) |
| `Ctrl+G` | Focus go-to-line input |
| `Ctrl+R` | Toggle regex mode |
| `Escape` | Clear search, return to file view |
| `Alt+←` | Previous page |
| `Alt+→` | Next page |

**Note on large files:** The daemon binary is bundled inside the VSIX. The first time a file is opened, a line index is built (one full pass) and cached to `<file>.glance_idx`. Subsequent opens load the index instantly.

## Project structure

```
glance/
├── Cargo.toml
├── src/
│   ├── main.rs              entry point, CLI (--version, --help)
│   ├── lib.rs               module exports
│   ├── security.rs          path validation (validate_path)
│   ├── protocol/
│   │   ├── request.rs       RPC request param types
│   │   └── response.rs      RPC response data types
│   ├── index/
│   │   ├── line_index.rs    byte-offset index for O(1) line seek
│   │   └── cache.rs         persist index to disk, reload on restart
│   ├── reader/
│   │   ├── mod.rs           file open, read_lines, search, search_regex, count, pretty-print
│   │   └── csv.rs           RFC 4180 CSV/TSV line parser (quoted fields, escaped quotes)
│   └── server/
│       └── rpc.rs           JSON-RPC 2.0 server (RwLock for concurrent reads)
├── tests/
│   ├── line_index_test.rs   line index — empty file, offsets, CRLF, round-trip
│   ├── cache_test.rs        cache — save/load, invalidation, corrupt magic
│   ├── reader_test.rs       reader — read_lines, search, regex, count, edge cases
│   ├── csv_test.rs          CSV parser — quoting, escaping, delimiter detection
│   └── security_test.rs     security — path traversal, symlinks, empty path
└── extensions/
    ├── vscode/              VS Code extension
    │   ├── src/             TypeScript source (extension.ts, daemon.ts, panel.ts)
    │   ├── media/           Webview assets (panel.css, panel.js)
    │   └── bin/             Bundled daemon binary
    └── zed/                 Zed extension (Rust → WASM, self-contained)
```

## Testing

```bash
cargo test
```

42 integration tests across 5 suites. All tests use real temporary files — no mocks.

| Suite | Tests | Coverage |
|---|---|---|
| `line_index_test` | 6 | empty file, byte offsets, CRLF, round-trip via `from_parts` |
| `cache_test` | 4 | save/load, cache invalidation on file change, corrupt magic bytes |
| `reader_test` | 15 | `read_lines`, `search`, `search_regex`, `count`, edge cases |
| `csv_test` | 11 | quoting, escaped quotes, empty fields, comma/tab delimiter |
| `security_test` | 6 | path traversal rejection, symlink resolution, empty path |

## Index cache

On first open, the daemon builds a line index (one full pass over the file) then saves it to `<file>.glance_idx` alongside the original file. On subsequent opens the cache is loaded instantly — no re-scan needed.

The cache is invalidated automatically when the file changes (checked via file size + modification time).

## Zed extension ⚠ experimental

```
extensions/zed/
├── extension.toml
├── Cargo.toml
└── src/lib.rs
```

**Requirements:** Rust must be installed via [rustup](https://rustup.rs) — not Homebrew. Zed uses rustup to compile extensions to WebAssembly.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Install as dev extension:**

1. Open Zed
2. Command Palette → `zed: install dev extension`
3. Select folder: `extensions/zed`

Zed compiles the extension to WASM automatically. No manual build step needed.

**Usage in Zed AI Assistant panel:**

```
/glance /path/to/file.jsonl                  show first 50 lines
/glance /path/to/file.jsonl --line 4500      jump to line 4500
/glance /path/to/file.jsonl --search "error" search (case-insensitive)
/glance /path/to/file.jsonl --count "timeout" count matches
/glance /path/to/file.jsonl --info           file metadata
```

**How it works:** The Zed extension reads files directly via WASI filesystem access inside the WebAssembly sandbox — no daemon process is spawned. The line index is built on demand within WASM on each invocation.

**Limitation:** Zed's extension API does not support custom webview panels. This extension only works inside the AI Assistant panel via `/glance`. A direct file viewer (like the VS Code panel) is not possible with the current Zed API.

## Security

Glance is a local desktop tool — the daemon communicates only over stdio with the spawning process (VS Code extension). It is not exposed over the network.

### Path validation

`open` requests are validated via `std::fs::canonicalize` in `src/security.rs` before any file is accessed. This resolves `..`, `.`, and symlinks in a single step and rejects paths that do not exist or are inaccessible:

```
{"method":"open","params":{"path":"../../etc/passwd"}}
→ Error: cannot access '../../etc/passwd': No such file or directory
```

### Webview content security

All file content rendered in the VS Code panel is HTML-escaped before insertion into the DOM. The webview runs with a strict Content Security Policy (`default-src 'none'`) and uses nonce-based inline script loading — no external scripts or `unsafe-inline` are permitted.

### Known limitations

- **TOCTOU (cache):** A window exists between reading file metadata and loading the index cache. An attacker with local write access could replace the file in that window. Fixing this would require file locking and adds disproportionate complexity for a local tool.
- **Stdin trust:** The daemon trusts all input from its stdin. In normal operation only the VS Code extension process writes to it. If another local process gains access to the daemon's stdin, it can open files readable by the current user.

## Changelog

### v0.1.0
- **Fix:** empty file incorrectly reported as 1 line — `LineIndex::build` now returns 0 lines for empty files
- **Fix:** `Mutex` held during disk I/O replaced with `RwLock` — concurrent reads across multiple open files no longer block each other
- **Fix:** `validate_path` moved to `src/security.rs` — all file access paths go through a single security gate
- **Fix:** LFI via path traversal — `open` requests validated with `canonicalize` before any file access
- Added `--version` flag to CLI
- Removed unused `--transport` argument and dead `is_csv()` method

## Roadmap

- [ ] JetBrains plugin (Kotlin)
- [ ] Deduplicate file handles — opening the same file twice reuses the existing index
- [ ] Configurable cache directory (default: alongside source file)
- [ ] Context lines for search results (`before`/`after` params)
