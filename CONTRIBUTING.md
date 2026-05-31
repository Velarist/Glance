# Contributing to Glance

## Setup

```bash
git clone https://github.com/<your-username>/Glance
cd glance
cargo build
```

For the VS Code extension:
```bash
cd extensions/vscode && npm install && npm run build
```

## Before submitting

```bash
cargo test        # 96 tests must pass
cargo clippy -- -D warnings  # zero warnings
```

## Branches

| Branch | Use |
|---|---|
| `main` | stable — merge via PR only |
| `dev` | active development |
| `feature/*` | created when needed for large isolated features |
| `hotfix/*` | created when needed for critical fixes from main |

## Rules

- No `unsafe` Rust
- New file access must go through `validate_path()`
- Webview content must go through `esc()` before rendering
- One responsibility per file

## License

Contributions are licensed under MIT.
