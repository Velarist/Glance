# Daemon — JSON-RPC Protocol

The daemon communicates over stdio using JSON-RPC 2.0 — one request per line, one response per line. Used by IDE extensions; for interactive use prefer the [CLI](cli.md).

```bash
glance serve   # explicit
glance         # default (no subcommand)
```

All requests accept an optional `"version": 1` field. Version mismatch logs a warning but still processes the request.

Multiple files can be open simultaneously — each `open` call returns an independent `file_id`.
When the same canonical path is already open, the daemon reuses the underlying file handle
while keeping each `file_id` lifecycle independent.

Known limitations:
- Deduplication is keyed by canonical path, not file identity. Hardlinks to the same inode
  through different canonical paths are treated as separate files.
- Existing `file_id` leases are not automatically refreshed if the file changes on disk.
  A later `open` of the same path checks file metadata and gets a fresh handle when needed.

---

## Methods

### open

Index a file, returns a `file_id`.

```json
{"jsonrpc":"2.0","id":1,"method":"open","params":{"path":"/data/events.jsonl"}}
```
```json
{"jsonrpc":"2.0","id":1,"result":{"file_id":1,"total_lines":5000000,"file_size":3221225472,"format":"jsonl"}}
```

### read

Fetch lines by offset (0-indexed). Response includes `fields: string[]` per line for CSV/TSV.

```json
{"jsonrpc":"2.0","id":2,"method":"read","params":{"file_id":1,"offset":0,"limit":200}}
{"jsonrpc":"2.0","id":2,"method":"read","params":{"file_id":1,"offset":0,"limit":50,"pretty":true}}
```

`"pretty": true` — pretty-prints JSON content (JSONL only).

### search

Substring search (case-insensitive). Supports `"regex": true` for regex mode.

```json
{"jsonrpc":"2.0","id":3,"method":"search","params":{"file_id":1,"query":"error","max_results":100}}
{"jsonrpc":"2.0","id":3,"method":"search","params":{"file_id":1,"query":"error\\d+","regex":true,"max_results":100}}
```

### count

Count matching lines — O(1) RAM regardless of file size.

```json
{"jsonrpc":"2.0","id":4,"method":"count","params":{"file_id":1,"query":"error"}}
```
```json
{"jsonrpc":"2.0","id":4,"result":{"count":18432}}
```

Also supports `"regex": true`.

### info

File metadata.

```json
{"jsonrpc":"2.0","id":5,"method":"info","params":{"file_id":1}}
```

### close

Release a file handle. Idempotent — closing a non-existent `file_id` is not an error.

```json
{"jsonrpc":"2.0","id":6,"method":"close","params":{"file_id":1}}
```

---

## Index cache

On first open, a line index is built (one full pass) and saved to `<file>.glance_idx` alongside the source file. Subsequent opens load the index instantly.

The cache is invalidated automatically when the file changes (checked via file size + modification time).
