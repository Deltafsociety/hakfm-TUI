#!/usr/bin/env bash
set -e

# ==============================================================================
# HAK.FM TUI - One-line Installer for Linux & macOS
# Repository: https://github.com/Deltafsociety/hakfm-TUI
# ==============================================================================

echo ""
echo "  _   _   ___   _  __  _____ __  __ "
echo " | | | | / _ \ | |/ / |  ___|  \/  |"
echo " | |_| |/ /_\ \| ' /  | |_  | |\/| |"
echo " |  _  ||  _  || . \  |  _| | |  | |"
echo " |_| |_||_| |_||_|\_\ |_|   |_|  |_|"
echo "  Terminal Radio for Hacker Focus   "
echo ""

OS="$(uname -s)"
ARCH="$(uname -m)"

# 1. Check audio engine (mpv)
if ! command -v mpv >/dev/null 2>&1; then
    echo "[-] Notice: 'mpv' is required for stream playback."
    if [ "$OS" = "Darwin" ]; then
        echo "    Install with: brew install mpv"
    elif [ -f /etc/arch-release ]; then
        echo "    Install with: sudo pacman -S mpv"
    elif [ -f /etc/debian_version ]; then
        echo "    Install with: sudo apt install mpv"
    elif [ -f /etc/fedora-release ]; then
        echo "    Install with: sudo dnf install mpv"
    fi
    echo ""
fi

BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

# 2. Prefer building with Cargo if installed, otherwise download release binary
if command -v cargo >/dev/null 2>&1; then
    echo "[+] Found Rust / Cargo. Building and installing latest release from source..."
    cargo install --git https://github.com/Deltafsociety/hakfm-TUI.git --bin hakfm --force
    CARGO_BIN="$(command -v hakfm || echo "$HOME/.cargo/bin/hakfm")"
    if [ -x "$CARGO_BIN" ]; then
        ln -sf "$CARGO_BIN" "$BIN_DIR/hakfm" 2>/dev/null || true
        ln -sf "$CARGO_BIN" "$BIN_DIR/radio" 2>/dev/null || true
    fi
else
    echo "[+] Cargo not found. Fetching pre-built binary..."
    TARGET_BIN="$BIN_DIR/hakfm"
    if curl -fSL "https://github.com/Deltafsociety/hakfm-TUI/releases/latest/download/hakfm" -o "$TARGET_BIN" 2>/dev/null || \
       curl -fSL "https://github.com/Deltafsociety/hakfm-TUI/releases/download/v1.0.0.0/hakfm" -o "$TARGET_BIN"; then
        chmod +x "$TARGET_BIN"
        ln -sf "$TARGET_BIN" "$BIN_DIR/radio"
    else
        echo "[!] Pre-built binary download failed. Please install Rust and compile with:"
        echo "    cargo install --git https://github.com/Deltafsociety/hakfm-TUI.git"
        exit 1
    fi
fi

# 3. Ensure ~/.local/bin is in PATH
for RC in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
    if [ -f "$RC" ] && ! grep -Fq '.local/bin' "$RC"; then
        printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$RC"
    fi
done

echo ""
echo "[✓] HAK.FM installed successfully!"
echo "    Launch anytime with: hakfm"
echo "                      or: radio"
echo ""
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]] && [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    echo "[i] Restart your terminal session, or run:"
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
