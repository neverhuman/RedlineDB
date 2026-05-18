#!/usr/bin/env bash
set -e

echo "Building RedlineDB CLI..."
cargo build --release -p redlinedb-cli

BIN_PATH="target/release/redlinedb"

if [ ! -f "$BIN_PATH" ]; then
    echo "Build failed! Binary not found at $BIN_PATH"
    exit 1
fi

echo "RedlineDB built successfully."

# Try to install to /usr/local/bin if possible, else use ~/.cargo/bin
INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ]; then
    echo "Warning: No write access to $INSTALL_DIR. Using ~/.cargo/bin instead"
    INSTALL_DIR="$HOME/.cargo/bin"
    mkdir -p "$INSTALL_DIR"
fi

echo "Installing redlinedb to $INSTALL_DIR..."
cp "$BIN_PATH" "$INSTALL_DIR/redlinedb"
chmod +x "$INSTALL_DIR/redlinedb"

# Ask about creating the sqlite3 alias
echo ""
echo "Do you want to create a symlink to alias 'sqlite3' to 'redlinedb'?"
echo "This will intercept all SQLite calls system-wide so agents use RedlineDB."
read -p "Create sqlite3 alias? [y/N]: " create_alias

if [[ "$create_alias" =~ ^[Yy]$ ]]; then
    if [ -w "$INSTALL_DIR" ]; then
        ln -sf "$INSTALL_DIR/redlinedb" "$INSTALL_DIR/sqlite3"
        echo "Created alias: $INSTALL_DIR/sqlite3 -> $INSTALL_DIR/redlinedb"
    else
        echo "Failed to create alias due to lack of write permissions."
    fi
else
    echo "Skipped alias creation."
fi

echo ""
echo "Installation complete!"
if [[ "$INSTALL_DIR" == "$HOME/.cargo/bin" ]]; then
    echo "Make sure $INSTALL_DIR is in your PATH."
fi
