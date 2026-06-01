# Glance

Open large files (JSONL, CSV, logs 3GB+) without crashing or lag — lightweight Rust daemon with a developer CLI and IDE extension support.

> **Status:** Early development. Core daemon and VS Code extension work. Zed extension is experimental.

## Quick start

```bash
cargo build --release

glance info /data/events.jsonl
glance read /data/events.jsonl --limit 50
glance search /data/events.jsonl "error"
glance validate /data/events.jsonl
```

## Supported formats

| Extension | Format |
|---|---|
| `.jsonl`, `.ndjson` | JSONL / Newline-delimited JSON |
| `.csv`, `.tsv` | Delimited text |
| `.log`, anything else | Raw text |

Format is detected by extension + first-line sniff.

## Documentation

| Doc | Description |
|---|---|
| [docs/cli.md](docs/cli.md) | All CLI subcommands and flags |
| [docs/daemon.md](docs/daemon.md) | JSON-RPC protocol for IDE extensions |
| [docs/extensions.md](docs/extensions.md) | VS Code and Zed extension setup |
| [docs/security.md](docs/security.md) | Security model and known limitations |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |
| [CHANGELOG.md](CHANGELOG.md) | Version history |

## Project structure

```
glance/
├── src/
│   ├── cli/        developer CLI subcommands
│   ├── reader/     file reading, search, CSV, pretty-print
│   ├── index/      line index + disk cache
│   ├── server/     JSON-RPC daemon
│   ├── protocol/   request/response types
│   └── security.rs path validation
├── docs/           detailed documentation
├── tests/          140 integration tests
└── extensions/
    ├── vscode/     VS Code extension
    └── zed/        Zed extension (WASM)
```

## Testing

```bash
cargo test   # 140 tests across 11 suites
```

## Roadmap

- [ ] JetBrains plugin (Kotlin)
- [ ] Deduplicate file handles
- [ ] Configurable cache directory
- [x] Context lines for search results (`--before`/`--after` params) — v0.3.0
