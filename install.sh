#!/usr/bin/env bash
set -e

REPO="Deltafsociety/hakfm-TUI"
LATEST_RELEASE_URL="https://github.com/$REPO/releases/latest"

echo ""
echo "  _   _   ___   _  __  _____ __  __ "
echo " | | | | / _ \ | |/ / |  ___|  \/  |"
echo " | |_| |/ /_\ \| ' /  | |_  | |\/| |"
echo " |  _  ||  _  || . \  |  _| | |  | |"
echo " |_| |_||_| |_||_|\_\ |_|   |_|  |_|"
echo "  Terminal Radio for Hacker Focus   "
echo ""

OS_RAW="$(uname -s)"
ARCH_RAW="$(uname -m)"

case "$OS_RAW" in
    Linux)  OS="linux" ;;
    Darwin) OS="darwin" ;;
    *)      OS="unknown" ;;
esac

case "$ARCH_RAW" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)
        if [ "$OS" = "darwin" ]; then
            ARCH="arm64"
        else
            ARCH="aarch64"
        fi
        ;;
    *)              ARCH="$ARCH_RAW" ;;
esac

BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"
TARGET_BIN="$BIN_DIR/hakfm"

FORCE_BUILD=0
for arg in "$@"; do
    if [ "$arg" = "--build" ] || [ "$arg" = "-b" ]; then
        FORCE_BUILD=1
    fi
done

INSTALLED=0

if [ "$FORCE_BUILD" -eq 0 ] && [ "$OS" != "unknown" ]; then
    echo "[+] Looking for pre-built binary for $OS ($ARCH)..."

    CANDIDATES=()
    if [ "$OS" = "darwin" ]; then
        CANDIDATES+=(
            "hakfm-darwin-universal"
            "hakfm-darwin-${ARCH}"
            "hakfm-darwin"
        )
    else
        CANDIDATES+=(
            "hakfm-linux-${ARCH}"
        )
        if [ "$ARCH" = "x86_64" ]; then
            CANDIDATES+=("hakfm")
        fi
    fi

    TMP_BIN="$(mktemp "${TMPDIR:-/tmp}/hakfm.XXXXXX")"
    for ASSET in "${CANDIDATES[@]}"; do
        DOWNLOAD_URL="$LATEST_RELEASE_URL/download/$ASSET"
        if curl -fSL --progress-bar "$DOWNLOAD_URL" -o "$TMP_BIN" 2>/dev/null; then
            chmod +x "$TMP_BIN"
            if "$TMP_BIN" --version >/dev/null 2>&1; then
                mv -f "$TMP_BIN" "$TARGET_BIN"
                chmod +x "$TARGET_BIN"
                ln -sf "$TARGET_BIN" "$BIN_DIR/radio"
                INSTALLED=1
                echo "[✓] Downloaded and verified pre-built binary ($ASSET)."
                break
            else
                rm -f "$TMP_BIN"
            fi
        fi
    done
    rm -f "$TMP_BIN" 2>/dev/null || true
fi

if [ "$INSTALLED" -eq 0 ]; then
    if [ "$FORCE_BUILD" -eq 1 ]; then
        echo "[+] Building from source requested..."
    else
        echo "[-] Pre-built binary not available or incompatible for this device."
        echo "[+] Falling back to compiling from source..."
    fi

    NEED_RUSTUP=0
    if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
        NEED_RUSTUP=1
    else
        RUST_VER="$(rustc --version 2>/dev/null | awk '{print $2}')"
        RUST_MINOR="$(echo "$RUST_VER" | cut -d. -f2)"
        if [ -n "$RUST_MINOR" ] && [ "$RUST_MINOR" -lt 85 ]; then
            echo "[-] Detected Rust $RUST_VER (system toolchain too old for edition 2024 crates)."
            NEED_RUSTUP=1
        fi
    fi

    if [ "$NEED_RUSTUP" -eq 1 ]; then
        if command -v rustup >/dev/null 2>&1; then
            echo "[+] Updating Rust toolchain via rustup..."
            rustup update stable
        else
            echo "[+] Installing official modern Rust toolchain via rustup..."
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        fi
        if [ -f "$HOME/.cargo/env" ]; then
            # shellcheck disable=SC1091
            source "$HOME/.cargo/env"
        fi
        export PATH="$HOME/.cargo/bin:$PATH"
    fi

    echo "[+] Compiling and installing latest hakfm from source..."
    cargo install --git "https://github.com/$REPO.git" --bin hakfm --force --locked

    CARGO_BIN="$(command -v hakfm || echo "$HOME/.cargo/bin/hakfm")"
    if [ -x "$CARGO_BIN" ]; then
        cp -f "$CARGO_BIN" "$TARGET_BIN" 2>/dev/null || ln -sf "$CARGO_BIN" "$TARGET_BIN"
        chmod +x "$TARGET_BIN"
        ln -sf "$TARGET_BIN" "$BIN_DIR/radio"
        INSTALLED=1
    fi
fi

if [ "$INSTALLED" -eq 0 ]; then
    echo "[!] Installation failed. Please check error output above or report an issue at:"
    echo "    https://github.com/$REPO/issues"
    exit 1
fi

if ! command -v mpv >/dev/null 2>&1; then
    echo ""
    echo "[!] Notice: 'mpv' is required for streaming audio."
    if [ "$OS" = "darwin" ]; then
        echo "    Install with: brew install mpv"
    elif command -v apt-get >/dev/null 2>&1; then
        echo "    Install with: sudo apt update && sudo apt install -y mpv"
    elif command -v pacman >/dev/null 2>&1; then
        echo "    Install with: sudo pacman -S mpv"
    elif command -v dnf >/dev/null 2>&1; then
        echo "    Install with: sudo dnf install -y mpv"
    elif command -v zypper >/dev/null 2>&1; then
        echo "    Install with: sudo zypper install mpv"
    elif command -v apk >/dev/null 2>&1; then
        echo "    Install with: sudo apk add mpv"
    else
        echo "    Please install 'mpv' using your system package manager."
    fi
fi

for RC in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
    if [ -f "$RC" ] && ! grep -Fq '.local/bin' "$RC"; then
        printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$RC"
    fi
done
if [ -d "$HOME/.config/fish" ] && [ -f "$HOME/.config/fish/config.fish" ]; then
    if ! grep -Fq '.local/bin' "$HOME/.config/fish/config.fish"; then
        printf '\nfish_add_path -a $HOME/.local/bin\n' >> "$HOME/.config/fish/config.fish"
    fi
fi

echo ""
echo "[✓] HAK.FM installed successfully!"
echo "    Launch anytime with: hakfm"
echo "                      or: radio"
echo ""
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]] && [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    echo "[i] Restart your terminal session, or run:"
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
