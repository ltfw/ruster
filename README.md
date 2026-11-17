<p align="center">
  <img src="assets/ruster.svg" width="96" alt="Ruster icon">
</p>

# Ruster

Ruster is a small desktop web browser written in Rust. It uses GTK 3 for the window and WebKitGTK 4.1 to render pages. It does not bundle Chromium or use Blink.

## Features

- Tabs in the title bar; each tab keeps its own page and history
- Address bar that accepts a URL or a search term (searches go to Google)
- Back, forward, and reload/stop buttons
- Bookmarks, saved to `~/.local/share/ruster/bookmarks.json`
- A local new-tab page with a search box
- Load errors shown in the status bar and printed to the terminal

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close current tab |
| `Ctrl+L` | Focus the address bar |
| `Ctrl+R` | Reload |
| `Alt+Left` / `Alt+Right` | Back / forward |

## Requirements

- Rust (stable, 2021 edition)
- GTK 3 and WebKitGTK 4.1 development libraries
- `pkg-config`

On CachyOS or Arch Linux:

```sh
sudo pacman -S --needed base-devel rust pkgconf gtk3 webkit2gtk-4.1
```

On Debian or Ubuntu:

```sh
sudo apt install build-essential cargo pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev
```

## Build and run

```sh
git clone https://github.com/ltfw/ruster.git
cd ruster
cargo run --release
```

## Install for your user (CachyOS / Arch)

```sh
./install-cachyos.sh
```

This builds a release binary and installs:

- the app to `~/.local/bin/ruster`
- a launcher to `~/.local/share/applications/ruster.desktop`
- the icon to `~/.local/share/icons/hicolor/scalable/apps/ruster.svg`

Ruster then appears in your application menu. Run the script again to update an existing install. The first build downloads crates from crates.io, so it needs an internet connection.

To uninstall:

```sh
./install-cachyos.sh uninstall
```

## Project layout

```
src/main.rs          the whole application
assets/ruster.svg    app icon
install-cachyos.sh   user install / uninstall script
```
