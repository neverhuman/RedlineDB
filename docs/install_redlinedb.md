# Installing RedlineDB

RedlineDB provides a CLI with a SQLite-oriented interface. It is intended for compatibility testing and incremental adoption, not as a blanket claim of perfect `sqlite3` parity.

## Release Binary Installation

For Linux and macOS, use the release installer. Pin `VERSION` for CI or any
reproducible environment:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v1.0.1 bash
```

For a fully locked install, also pin the tarball digest from the matching
release `.sha256` file:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.1 REDLINEDB_SHA256=<sha256> bash
```

The installer refuses to install if the checksum is missing or mismatched.

## Source Installation

We provide an `install.sh` script to build the release binary and install it globally.

```bash
./install.sh
```

During installation, the script will prompt you:
> `Do you want to create a symlink to alias 'sqlite3' to 'redlinedb'?`

If you say **Yes**, it will create `/usr/local/bin/sqlite3` which simply symlinks to RedlineDB. All system calls to `sqlite3` will be intercepted!

## CLI Parity

The command-line wrapper supports the documented flags and invocation patterns listed below:

```bash
# Execute a single query via arguments
sqlite3 my_database.db "SELECT * FROM users;" -json

# Pipe queries via stdin
echo "SELECT * FROM users" | sqlite3 my_database.db -csv

# Open an interactive REPL
sqlite3 my_database.db
```

Supported SQLite flags:
- `-json`, `-csv`, `-list`, `-line` (Output formatting)
- `-header`, `-noheader` (Headers toggle)
- `-bail` (Exit on error)
- `-echo` (Echo inputs)
- `-separator SEP` (Custom separators)

## Uninstalling the Alias

To revert the alias back to the system SQLite:
```bash
rm /usr/local/bin/sqlite3
```
*(Your system will gracefully fall back to `/usr/bin/sqlite3` or brew's installation).*
