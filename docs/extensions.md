# Editor Extensions

## VS Code ✓ working

### Install

Build and package the VSIX:

```bash
cd extensions/vscode
npm install
npm run build
npx vsce package --no-dependencies
```

Install in VS Code:
```bash
code --install-extension glance-0.2.1.vsix
# or: Cmd+Shift+P → Extensions: Install from VSIX
```

### Usage

Right-click any `.jsonl`, `.csv`, `.tsv`, or `.log` file in the Explorer → **Glance: Open Large File**

### Features

| Feature | How |
|---|---|
| Pagination | `← Prev` / `Next →` — 200 lines per page (10 in pretty mode) |
| Search | Live substring search, debounced 300ms |
| Regex search | Toggle `.*` or `Ctrl+R` — ReDoS-safe |
| Pretty-print JSON | Toggle `{}` — expands JSONL records into cards |
| Match navigation | `▲` / `▼` — jump to prev/next match in file context |
| Malformed lines | Collapsed with 150-char preview — click `▶ Show` to expand |
| Jump to line | Type in `Line #` field and press `Enter` |
| CSV column view | CSV/TSV renders as a table automatically |
| Copy line | Click any line number |
| Theme support | Follows active VS Code theme automatically |

### Keyboard shortcuts

| Key | Action |
|---|---|
| `Ctrl+F` | Focus search |
| `Enter` | Next match |
| `Shift+Enter` | Previous match |
| `Ctrl+G` | Focus go-to-line |
| `Ctrl+R` | Toggle regex |
| `Escape` | Clear search |
| `Alt+←` / `Alt+→` | Previous / next page |

---

## Zed ⚠ experimental

The Zed extension compiles to WebAssembly and runs inside the AI Assistant panel as a slash command. It reads files directly via WASI — no daemon process needed.

**Limitation:** Zed's extension API does not support custom panels. A direct file viewer is not possible with the current Zed API.

### Requirements

Rust must be installed via **rustup** (not Homebrew):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Install

1. Open Zed
2. `Cmd+Shift+P` → `zed: install dev extension`
3. Select folder: `extensions/zed`

Zed compiles the extension to WASM automatically.

### Usage

In the Zed AI Assistant panel:

```
/glance /path/to/file.jsonl
/glance /path/to/file.jsonl --line 4500
/glance /path/to/file.jsonl --search "error"
/glance /path/to/file.jsonl --count "timeout"
/glance /path/to/file.jsonl --info
```
