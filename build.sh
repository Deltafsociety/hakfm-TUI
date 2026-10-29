
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

echo "Building $BIN_NAME..."
cargo build --release --manifest-path "$MANIFEST"

BIN="$PROJECT_DIR/target/release/$BIN_NAME"

if [[ ! -x "$BIN" ]]; then
    echo "Error: Executable not found: $BIN"
    exit 1
fi

mkdir -p "$BIN_DIR"

for COMMAND in hakfm radio; do
    cat > "$BIN_DIR/$COMMAND" <<EOF
#!/usr/bin/env bash
exec "$BIN" "\$@"
EOF
    chmod +x "$BIN_DIR/$COMMAND"
done

for RC in "$HOME/.bashrc" "$HOME/.zshrc"; do
    if [[ -f "$RC" ]] && ! grep -Fq '.local/bin' "$RC"; then
        printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$RC"
    fi
done

export PATH="$BIN_DIR:$PATH"

echo
echo "Installed successfully!"
echo "You can now run: hakfm"
echo "                 radio"
echo
echo "Restart your terminal or run:"
echo '  export PATH="$HOME/.local/bin:$PATH"'
