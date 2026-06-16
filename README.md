# hostab

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/dyrnq/hostab/actions/workflows/ci.yml/badge.svg)](https://github.com/dyrnq/hostab/actions/workflows/ci.yml)

**Your dev tool to manage `/etc/hosts` like a pro — written in Rust.**

A fast, safe hosts file manager distributed as a single static binary, with a REST API server and Swagger UI.

## Why hostab?

- **Atomic writes** — `NamedTempFile` → `sync_all` → `persist` (no partial writes, no TOCTOU)
- **File locking** — `flock` prevents concurrent modification
- **DNS resolution** — forward and reverse lookups via hickory-resolver
- **Filter & search** — wildcard / regex filter, auto-detect pattern type
- **Merge** — combine hosts files from local or remote sources (SSRF-safe)
- **Multi-format output** — table, raw TSV, markdown, JSON
- **REST API** — serve with OpenAPI docs, full CRUD + disable/enable/toggle
- **Canonical / Alias awareness** — first hostname is canonical, per POSIX /etc/hosts semantics

## Installation

```bash
cargo install hostab
```

Or download from [releases](https://github.com/dyrnq/hostab/releases) (Linux x86_64/ARM64/ARMv7, macOS x86_64/Apple Silicon, Windows x86_64).

## Quick Start

```bash
# List
hostab e list

# Filter with auto-detect (*/? → glob, else substring)
hostab e list -f "prod*"
hostab e list -f localhost -i

# IPv4 / IPv6 only
hostab e list --ipv4

# Add
hostab e add 10.0.0.1 app.local api.local
hostab e add 10.0.0.2 db.local --comment "database"

# Add with explicit canonical and aliases
hostab e add 10.0.0.1 --canonical app.local --alias api.local

# Remove
hostab e rm app.local
hostab e rm --ip 10.0.0.1

# Enable / disable / toggle (auto-split from shared IP, alias promotes)
hostab e disable api.local
hostab e enable api.local
hostab e toggle api.local
hostab e disable --ip 10.0.0.1

# Move hostname to new IP
hostab e edit api.local --ip 10.0.0.99

# DNS resolution
hostab resolve github.com
hostab resolve 8.8.8.8
hostab resolve -l localhost       # local hosts file only

# Merge from multiple sources
hostab merge -s ./dev-hosts -s https://example.com/hosts

# Validate (table output with LINE/IP/HOST/ISSUE)
hostab verify

# Start REST API server (default: 127.0.0.1:3456)
hostab serve
hostab serve --port 8080 --bind 0.0.0.0
```

## Commands

### Entry (`e`)

| Command | Description |
|---|---|
| `e list [--ipv4\|--ipv6] [-f FILTER] [-i]` | List entries, auto-detect filter |
| `e add <ip> <hosts...> [--canonical X] [--alias X...] [--comment X]` | Add entry, first hostname is canonical |
| `e rm <hosts...> [--ip X]` | Remove by hostname or IP |
| `e disable <hosts...> [--ip X]` | Comment out (alias promotes to canonical) |
| `e enable <hosts...> [--ip X]` | Uncomment (merge back) |
| `e toggle <host> [--ip X]` | Flip enabled/disabled |
| `e edit <host> --ip X` | Move hostname to new IP |

### Top-level

| Command | Description |
|---|---|
| `serve [--port P] [--bind ADDR] [--no-docs]` | Start REST API server with Swagger UI |
| `resolve <hosts...> [-l]` | DNS forward/reverse lookup, `-l` for local only |
| `merge -s <src...> [-t <target>]` | Merge from files/URLs (SSRF-protected) |
| `verify [--strict]` | Validate, table output (LINE/IP/HOST/ISSUE) |
| `cat` | Print raw hosts file |
| `completion <bash\|zsh\|fish>` | Shell completion |
| `version` | Version + commit + build date |

### REST API

| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/api/entries` | List entries (`?ip=`, `?hostname=`, `?filter=`) |
| `POST` | `/api/entries` | Add entry `{"ip":"...","hosts":[...],"comment":"..."}` |
| `DELETE` | `/api/entries/{hostname}` | Remove by hostname |
| `DELETE` | `/api/entries?ip=X` | Remove by IP |
| `PUT` | `/api/entries/{hostname}` | Edit: move to new IP `{"ip":"..."}` |
| `PUT` | `/api/entries/{hostname}/disable` | Disable hostname |
| `PUT` | `/api/entries/{hostname}/enable` | Enable hostname |
| `PUT` | `/api/entries/{hostname}/toggle` | Toggle hostname |

OpenAPI 3.1 spec at `/api/openapi.json`, Swagger UI at `/docs` (omit with `--no-docs`).

## Filter (auto-detect)

```
hostab e list -f local      # substring match
hostab e list -f "prod*"    # glob (*/? wildcards)
hostab e list -f "db.loca?" # glob single-char
hostab e list -f local -i   # case insensitive
```

## Output Formats

`--out table` (default, expanded):

```
│ ip            │ host                 │ comment │ canonical    │
├───────────────┼──────────────────────┼─────────┼──────────────┤
│ 127.0.0.1     │ localhost            │         │ localhost    │
│ 10.0.0.1      │ app.local api.local  │         │ app.local    │
```

`--out json`:  `[{"ip":"127.0.0.1","host":"localhost","comment":null,"canonical":"localhost","aliases":[]}]`

`--out raw`: tab-separated with CANONICAL column; `--out markdown`: GitHub-flavored table.

## Verify output

```
╭──────┬───────────┬───────────┬───────────╮
│ LINE │ ip        │ host      │ issue     │
├──────┼───────────┼───────────┼───────────┤
│ 1    │ 127.0.0.1 │ localhost │ duplicate │
│ 2    │ ::1       │ localhost │ duplicate │
╰──────┴───────────┴───────────┴───────────╯
```

## Global Options

```
--hosts-file <PATH>   [env: HOSTS_FILE] [default: /etc/hosts]
-q, --quiet
-o, --out <FORMAT>    table, raw, markdown, json
```

## Safety

- **Atomic writes** — `NamedTempFile` → `sync_all` → `persist` (no TOCTOU)
- **File locking** — `flock` prevents concurrent access
- **Path validation** — path traversal and null byte checks on all reads/writes
- **Input validation** — IP, hostname, and comment validation on API endpoints
- **SSRF protection** — merge blocks requests to private/reserved IP ranges
- **Rate limited API** — server defaults to 127.0.0.1 with concurrent connection limit
- **Duplicate detection** — warns on re-add, verify finds duplicates
- **Canonical/Alias semantics** — first hostname is canonical per POSIX, alias promotes on removal

## License

MIT
