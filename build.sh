#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="$PROJECT_DIR/Cargo.toml"
BIN_DIR="$HOME/.local/bin"

if [[ ! -f "$MANIFEST" ]]; then
    echo "Error: Cargo.toml not found in $PROJECT_DIR"
    exit 1
fi

BIN_NAME="$(awk '
    /^\[package\]/ { in_package=1; next }
    /^\[/ { in_package=0 }
    in_package && /^[[:space:]]*name[[:space:]]*=/ {
        sub(/^[^=]*=[[:space:]]*/, "")
        gsub(/["\047]/, "")
        print
        exit
    }
' "$MANIFEST")"

if [[ -z "$BIN_NAME" ]]; then
    echo "Error: Could not determine package name from Cargo.toml"
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
    echo "[-] Rust and Cargo are required to build $BIN_NAME."
    echo "[+] Install the official modern Rust compiler via:"
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

RUST_VER="$(rustc --version 2>/dev/null | awk '{print $2}')"
RUST_MINOR="$(echo "$RUST_VER" | cut -d. -f2)"
if [[ -n "$RUST_MINOR" && "$RUST_MINOR" -lt 85 ]]; then
    echo "[-] Detected Rust $RUST_VER (apt/system package manager version is too old for Rust 2024 dependencies)."
    if command -v rustup >/dev/null 2>&1; then
        echo "[+] Updating Rust via rustup..."
        rustup update stable
    else
        echo "[!] Modern dependencies require Rust 1.85+."
        echo "    Install or update via official rustup: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
fi

echo "Building $BIN_NAME..."
cargo build --release --manifest-path "$MANIFEST"

BIN="$PROJECT_DIR/target/release/$BIN_NAME"

if [[ ! -x "$BIN" ]]; then
    echo "Error: Executable not found: $BIN"
    exit 1
fi

mkdir -p "$BIN_DIR"
cp -f "$BIN" "$BIN_DIR/$BIN_NAME"
chmod +x "$BIN_DIR/$BIN_NAME"
ln -sf "$BIN_DIR/$BIN_NAME" "$BIN_DIR/radio"

for RC in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
    if [[ -f "$RC" ]] && ! grep -Fq '.local/bin' "$RC"; then
        printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$RC"
    fi
done

if [[ -d "$HOME/.config/fish" && -f "$HOME/.config/fish/config.fish" ]]; then
    if ! grep -Fq '.local/bin' "$HOME/.config/fish/config.fish"; then
        printf '\nfish_add_path -a $HOME/.local/bin\n' >> "$HOME/.config/fish/config.fish"
    fi
fi

export PATH="$BIN_DIR:$PATH"

echo ""
echo "[✓] Installed successfully into $BIN_DIR/$BIN_NAME!"
echo "    You can now run: hakfm"
echo "                 or: radio"
echo ""
echo "Restart your terminal or run:"
echo '  export PATH="$HOME/.local/bin:$PATH"'
