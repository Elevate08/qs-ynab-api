#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$ROOT_DIR/bin"
BACKEND_DIR="$ROOT_DIR/backend"

echo "==> Building YNAB Overview Rust Backend..."
mkdir -p "$BIN_DIR"

cd "$BACKEND_DIR"
cargo test --release
cargo build --release

cp "$BACKEND_DIR/target/release/ynab-cli" "$BIN_DIR/ynab-cli"
chmod +x "$BIN_DIR/ynab-cli"

echo "==> Build complete: $BIN_DIR/ynab-cli"
