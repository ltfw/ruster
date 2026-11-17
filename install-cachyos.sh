#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
user_bin="$HOME/.local/bin"
user_apps="$HOME/.local/share/applications"
user_icon="$HOME/.local/share/icons/hicolor/scalable/apps/ruster.svg"

if [[ "${1:-install}" == "uninstall" ]]; then
  rm -f "$user_bin/ruster" "$user_apps/ruster.desktop" "$user_icon"
  rmdir --ignore-fail-on-non-empty "$(dirname -- "$user_icon")" 2>/dev/null || true
  echo "Ruster has been removed from your user account."
  exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Cargo is missing. Install Rust with: sudo pacman -S rust" >&2
  exit 1
fi

cargo build --release --locked --manifest-path "$project_dir/Cargo.toml"
install -Dm755 "$project_dir/target/release/ruster" "$user_bin/ruster"
install -Dm644 "$project_dir/assets/ruster.svg" "$user_icon"
install -d "$user_apps"

cat > "$user_apps/ruster.desktop" <<DESKTOP
[Desktop Entry]
Version=1.0
Type=Application
Name=Ruster
Comment=Browse the web
Exec="$user_bin/ruster"
Icon=$user_icon
Terminal=false
Categories=Network;WebBrowser;
StartupWMClass=io.ruster.Browser
DESKTOP

chmod 644 "$user_apps/ruster.desktop"
echo "Ruster is installed. Find it in your app menu, or run: $user_bin/ruster"
