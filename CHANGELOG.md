# Changelog

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
