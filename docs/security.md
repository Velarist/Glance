# Security

Glance is a local desktop tool. The daemon communicates only over stdio with the spawning process and is never exposed over the network.

## Path validation

All `open` requests are validated via `std::fs::canonicalize` in `src/security.rs` before any file is accessed. This resolves `..`, `.`, and symlinks in a single step and rejects paths that do not exist or are inaccessible.

```
{"method":"open","params":{"path":"../../etc/passwd"}}
→ Error: cannot access '../../etc/passwd': No such file or directory
```

## Webview content security

All file content rendered in the VS Code panel is HTML-escaped before insertion into the DOM. The webview uses a strict Content Security Policy (`default-src 'none'`) with nonce-based inline script loading — no external scripts or `unsafe-inline`.

## Known limitations

**TOCTOU (cache):** A window exists between reading file metadata and loading the index cache. An attacker with local write access could replace the file in that window. Fixing this requires file locking — disproportionate complexity for a local tool.

**Stdin trust:** The daemon trusts all input from its stdin. In normal operation only the spawning process (VS Code extension) writes to it. If another local process gains access to the daemon's stdin, it can open files readable by the current user.
