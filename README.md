# hostab

[![CI](https://github.com/dyrnq/hostab/actions/workflows/ci.yml/badge.svg)](https://github.com/dyrnq/hostab/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Your dev tool to manage `/etc/hosts` like a pro — written in Rust.**

A fast, safe hosts file manager distributed as a single static binary.

## Why hostab?

- **Atomic writes** — write to `.tmp`, `fsync`, `rename` (no partial writes)
- **File locking** — `flock` prevents concurrent modification
- **DNS resolution** — forward and reverse lookups via hickory-resolver
- **Filter & search** — wildcard / regex filter, auto-detect pattern type
- **Merge** — combine hosts files from local or remote sources
- **Multi-format output** — table, raw TSV, markdown, JSON

## Installation

```bash
cargo install hostab
```

Or download from [releases](https://github.com/dyrnq/hostab/releases) (Linux x86_64/ARM64/ARMv7, macOS x86_64/Apple Silicon, Windows x86_64).

## Quick Start

```bash
# List (compact by default)
hostab e list

# Filter with auto-detect (*/? → glob, else substring)
hostab e list -f "prod*"
hostab e list -f localhost -i

# IPv4 / IPv6 only
hostab e list --ipv4

# Add
hostab e add 10.0.0.1 app.local api.local
hostab e add 10.0.0.2 db.local --comment "database"

# Remove
hostab e rm app.local
hostab e rm --ip 10.0.0.1

# Enable / disable / toggle (auto-split from shared IP)
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
```

## Commands

### Entry (`e`)

| Command | Description |
|---|---|
| `e list [--ipv4\|--ipv6] [--expand] [-f FILTER] [-i]` | List entries, auto-detect filter |
| `e add <ip> <hosts...> [--comment X]` | Add or merge entry |
| `e rm <hosts...> [--ip X]` | Remove by hostname or IP |
| `e disable <hosts...> [--ip X]` | Comment out (split from shared IP) |
| `e enable <hosts...> [--ip X]` | Uncomment (merge back) |
| `e toggle <host> [--ip X]` | Flip enabled/disabled |
| `e edit <host> --ip X` | Move hostname to new IP |

### Top-level

| Command | Description |
|---|---|
| `resolve <hosts...> [-l]` | DNS forward/reverse lookup, `-l` for local only |
| `merge -s <src...> [-t <target>]` | Merge from files/URLs |
| `verify [--strict]` | Validate, table output (LINE/IP/HOST/ISSUE) |
| `cat` | Print raw hosts file |
| `completion <bash\|zsh\|fish>` | Shell completion |
| `version` | Version + commit + build date |

## Filter (auto-detect)

```
hostab e list -f local      # substring match
hostab e list -f "prod*"    # glob (*/? wildcards)
hostab e list -f "db.loca?" # glob single-char
hostab e list -f local -i   # case insensitive
```

## Output Formats

`--out table` (default, compact):

```
│ ip            │ host                │ comment  │
├───────────────┼─────────────────────┼──────────┤
│ 127.0.0.1     │ localhost           │          │
│ 10.0.0.1      │ app.local api.local │          │
```

`--out json`:  `[{"ip":"127.0.0.1","host":"localhost","comment":null}]`

`--out raw`: tab-separated; `--out markdown`: GitHub-flavored table.

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

- **Atomic writes** — `.tmp` → `fsync` → `rename`
- **File locking** — `flock` prevents concurrent access
- **Duplicate detection** — warns on re-add, verify finds duplicates
- **Split/merge** — disable/enable preserves co-hosted hostnames

## License

MIT
