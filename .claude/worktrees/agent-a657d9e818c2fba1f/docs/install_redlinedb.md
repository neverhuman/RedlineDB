# Installing RedlineDB

RedlineDB provides a drop-in replacement for the standard `sqlite3` CLI tool. This allows AI agents, automated scripts, and interactive users to seamlessly use RedlineDB without knowing the difference!

## Global Installation

We provide an `install.sh` script to build the release binary and install it globally.

```bash
./install.sh
```

During installation, the script will prompt you:
> `Do you want to create a symlink to alias 'sqlite3' to 'redlinedb'?`

If you say **Yes**, it will create `/usr/local/bin/sqlite3` which simply symlinks to RedlineDB. All system calls to `sqlite3` will be intercepted!

## CLI Parity

RedlineDB perfectly mimics the `sqlite3` binary signature:

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
