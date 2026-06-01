# Security

Glance is a local desktop tool. The daemon communicates only over stdio with the spawning process and is never exposed over the network.

## Path validation

All core file access goes through `validate_path()` in `src/security.rs` before a file is opened. This includes the CLI commands (`info`, `read`, `search`, `count`, `validate`) and daemon `open` requests.

`validate_path()` uses `std::fs::canonicalize`, which resolves `..`, `.`, and symlinks in one step and rejects paths that do not exist or are inaccessible to the current user.

```
{"method":"open","params":{"path":"./missing.jsonl"}}
-> Error: cannot access './missing.jsonl': No such file or directory
```

This is a validation and normalization boundary, not a filesystem sandbox. Glance may open any existing file that the current OS user is allowed to read.

## Cache validation

Line index caches are loaded only when the cache header is valid and the cached file size and modification time match the current file metadata. Corrupt, truncated, stale, or absurdly large cache indexes are rejected and rebuilt from the source file.

## Known limitations

**TOCTOU (cache):** A window exists between reading file metadata and loading the index cache. An attacker with local write access could replace the file in that window. Fixing this requires file locking, which is disproportionate complexity for a local tool.

**Stdin trust:** The daemon trusts all input from its stdin. In normal operation only the spawning process (VS Code extension) writes to it. If another local process gains access to the daemon's stdin, it can open files readable by the current user.
