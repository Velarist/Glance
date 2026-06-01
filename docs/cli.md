# CLI Reference

Every subcommand supports `--json` for machine-readable output.

## glance info

Show file metadata.

```bash
glance info <path> [--json]
```

```bash
glance info /data/events.jsonl
# File:   /data/events.jsonl
# Format: jsonl
# Lines:  1,122,000
# Size:   4.49 MB (...)

glance info /data/events.jsonl --json | jq .lines
```

---

## glance read

Read lines from a file.

```bash
glance read <path> [--offset N] [--limit N] [--pretty] [--json]
```

| Flag | Default | Description |
|---|---|---|
| `--offset` | `0` | Start from line N (0-indexed) |
| `--limit` | `20` | Number of lines to return |
| `--pretty` | off | Expand JSON content (JSONL only) |
| `--json` | off | Machine-readable output |

```bash
glance read /data/events.jsonl
glance read /data/events.jsonl --offset 1000 --limit 50
glance read /data/events.jsonl --pretty
glance read /data/events.jsonl --json | jq '.lines[0].content'
```

CSV/TSV files render as an aligned table automatically.

---

## glance search

Search for a query string in a file. Case-insensitive by default.
Match is highlighted with `>>match<<` markers.

```bash
glance search <path> <query> [--regex] [--max N] [--json]
```

| Flag | Default | Description |
|---|---|---|
| `--regex` | off | Treat query as regex (case-sensitive) |
| `--max` | `50` | Maximum results to show |
| `--before` | `0` | Lines of context before each match |
| `--after` | `0` | Lines of context after each match |
| `--json` | off | Machine-readable output |

```bash
glance search /data/events.jsonl "error"
glance search /data/events.jsonl "\d{3}" --regex
glance search /data/events.jsonl "timeout" --max 100
glance search /data/events.jsonl "error" --before 2 --after 2
glance search /data/events.jsonl "error" --json | jq '.results[].line'
```

Match line is marked with `▶`. Context lines are shown above/below, separated by `─────` between match groups.

---

## glance count

Count lines matching a query. O(1) memory — streams the file.

```bash
glance count <path> <query> [--regex] [--json]
```

```bash
glance count /data/events.jsonl "error"
glance count /data/events.jsonl "\d+" --regex
glance count /data/events.jsonl "error" --json | jq .count
```

---

## glance validate

Scan a JSONL file and report lines that are not valid JSON.
Exits `1` if any invalid lines are found — useful in CI pipelines.

```bash
glance validate <path> [--json]
```

```bash
glance validate /data/events.jsonl
glance validate /data/events.jsonl --json

# CI usage
glance validate data.jsonl && echo "clean" || echo "has invalid lines"
```

---

## glance serve

Run the JSON-RPC daemon over stdio. This is the default when no subcommand is given.

```bash
glance serve
# or just:
glance
```
