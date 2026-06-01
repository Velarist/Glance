# Changelog

---

## [0.3.4]

### Fixed
- VS Code extension now renders CSV/TSV search results as a table, matching normal read rendering.
- Explicit `Glance: Open Large File` commands now open files in Glance even when they are below the automatic 25 MB threshold.

---

## [0.3.3]

### Fixed
- Daemon `open` now deduplicates active file handles for the same canonical path while keeping each `file_id` lifecycle independent.
- Reopening a file after it changes while an older lease is still active now gets a fresh handle and line index instead of reusing stale metadata.

### Added
- Black-box daemon coverage for duplicate open/close lifecycle behavior.
- Internal dedup tests that verify shared handles are actually reused.
- Tracing fields for daemon open/close handle lifecycle: dedup hit/miss, discarded builds, active leases, and active paths.

### Known limitations
- File handle deduplication is keyed by canonical path, so hardlinks to the same inode are not deduplicated.
- Existing `file_id` leases keep their original line index if the file changes on disk; callers should `open` again to get a fresh handle.

---

## [0.3.2]

### Fixed
- Cross-platform test assertions now use path-aware absolute path checks instead of Unix-only `/` prefix checks.
- Removed unused test variables that triggered warnings in CLI and reader edge case coverage.

---

## [0.3.1]

### Fixed
- `ContextLine.line_number` in CLI `--json` output now 1-indexed, consistent with `SearchMatch.line`

### Added
- `GLANCE_CACHE_DIR` environment variable — store index caches in a custom directory instead of alongside source files. Useful for read-only directories, network drives, or shared cache locations.

```bash
export GLANCE_CACHE_DIR="$HOME/.cache/glance"
```

---

## [0.3.0]

### Added
- Context lines for search — `--before N` and `--after N` in CLI, `"before"`/`"after"` in JSON-RPC
- `FileHandle.search_with_context` — uses line index for O(1) seeks when fetching context
- Match line marked with `▶` in CLI output, separator `─────` between match groups
- `context_before` and `context_after` fields in `SearchResult` (omitted from JSON when empty)

---

## [0.2.1]

### Fixed
- `glance read` on CSV/TSV files now renders as an aligned table — `fields[]` were being parsed by the daemon but silently dropped during CLI mapping to `ReadLine`. JSON output also now includes `fields` per line.

---

## [0.2.0]

### Added
- `glance info` — show file metadata from terminal
- `glance read` — read lines with `--offset`, `--limit`, `--pretty`
- `glance search` — search with match highlight `>>x<<`, `--regex`, `--max`
- `glance count` — count matching lines, `--regex`
- `glance validate` — scan JSONL, report invalid lines, exits `1` if found (CI-friendly)
- `glance serve` — explicit daemon subcommand (default without subcommand)
- `--json` flag on all subcommands — machine-readable output for scripting/jq
- `src/cli/output.rs` — single Format enum, no output logic duplication
- Tests: 96 → 140 across 11 suites

---

## [0.1.0]

### Fixed
- Empty file incorrectly reported as 1 line — `LineIndex::build` now returns 0
- `Mutex` held during disk I/O replaced with `RwLock` — concurrent reads no longer block each other
- `validate_path` moved to `src/security.rs` — single security gate for all file access
- LFI via path traversal — `open` requests validated with `canonicalize`
- Cache OOM — `total_lines` capped at 500M before `Vec::with_capacity`
- Unicode search — byte offsets from `line_lower` no longer used on `line` directly
- Blocking I/O in async — `cmd_read`, `cmd_search`, `cmd_count` use `spawn_blocking`
- Empty query now returns a clear error instead of matching every line
- Request size capped at 4MB to prevent unbounded memory use

### Changed
- `reader/` split into `format.rs`, `search.rs`, `stream.rs`, `pretty.rs` — one responsibility per file

### Added
- `--version` flag to CLI
- 96 integration tests across 10 suites
- CI with multi-platform builds and release workflow
