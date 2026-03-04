# SSTEQ25 Discover

Desktop app that finds SSTEQ25 telescope fork mounts on the local network and opens their web interface in a browser.

## Prerequisites

### All Platforms

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [pnpm](https://pnpm.io/) — install via `corepack enable && corepack prepare pnpm@latest --activate` or `npm install -g pnpm`

### Linux

Install system dependencies for Tauri and mDNS:

**Debian / Ubuntu:**

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

**Arch Linux:**

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \
  appmenu-gtk-module libappindicator-gtk3 librsvg xdotool
```

**Fedora:**

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install "c-development"
```

`avahi-daemon` is also required for mDNS discovery (usually pre-installed).

### macOS

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

### Windows

1. Install [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) — select **"Desktop development with C++"** during installation.
2. [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) — already included on Windows 10 (1803+) and later. Otherwise install the Evergreen Bootstrapper.
3. Install Rust with the MSVC toolchain (default on Windows via [rustup](https://www.rust-lang.org/tools/install)).

For more details see the [Tauri v2 Prerequisites guide](https://v2.tauri.app/start/prerequisites/).

## Development

```bash
pnpm install
pnpm tauri dev
```

## Build

```bash
pnpm tauri build
```

Outputs platform-specific installers in `src-tauri/target/release/bundle/`.

## How It Works

The app discovers mounts using three methods simultaneously:

1. **mDNS** — listens for `_sstmount._tcp.local.` service advertisements
2. **Network scan** — checks port 5000 on all IPs in local subnets (repeats every 3 min)
3. **Known IPs** — tries `192.168.45.1` and `192.168.46.2` directly

Discovered IPs are probed via HTTP (`/api/hostname`) to identify SST mounts vs AllSky cameras. Clicking "Connect" opens `http://<ip>:5000` in your default browser. Previously connected IPs are remembered across sessions.

## Stack

- **Tauri v2** — desktop shell
- **React + TypeScript** — frontend (Vite)
- **Material UI** — dark theme UI components
- **Rust** — backend (mDNS via `mdns-sd`, network scanning via `tokio`, HTTP probing via `reqwest`)
