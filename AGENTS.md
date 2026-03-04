# AGENTS.md

## Project Overview

Tauri v2 desktop app (Rust backend + React/TypeScript frontend) that discovers SSTEQ25 telescope mounts on the local network.

## Architecture

```
src/              React + TypeScript + Material UI frontend (Vite)
src-tauri/src/    Rust backend
  lib.rs          Tauri setup, shared state (AppState), IPC command handlers
  discovery.rs    mDNS browsing for _sstmount._tcp.local. (mdns-sd crate, background thread)
  scanner.rs      TCP port 5000 scanning across local subnets (tokio, concurrent)
  prober.rs       HTTP probing /api/hostname and /api/config (reqwest)
```

## Key Patterns

- **IPC**: Frontend calls Rust via `invoke()` commands. Backend pushes updates via `emit()` events (`devices-updated`, `scan-progress`, `mdns-discovered`, `ips-updated`).
- **State**: `AppState` holds `discovered_ips`, `devices`, and `storage_path` behind `Arc<Mutex<>>`. Managed via `app.manage()`.
- **Async runtime**: Use `tauri::async_runtime::handle()` to obtain the Tokio runtime handle — never `tokio::runtime::Handle::current()`. Event listener callbacks and `setup()` run on non-Tokio threads; `tauri::async_runtime::handle()` is safe from any thread. Pass the handle explicitly to background threads (e.g. `scanner::start_scan_loop`).
- **Mutex locking**: All `.lock()` calls use `.unwrap_or_else(|e| e.into_inner())` to recover from poisoned mutexes and prevent cascading panics across threads.
- **Persistence**: Previously connected IPs are stored in `connected.json` in the Tauri app data directory (not localStorage).
- **Connection**: Opens mount web UI (`http://<ip>:5000`) in system browser via `tauri-plugin-opener`.

## Commands

```bash
pnpm install          # install JS deps
pnpm tauri dev        # run in dev mode (hot reload)
pnpm tauri build      # production build
cargo check           # check Rust only (run from src-tauri/)
npx tsc --noEmit      # check TypeScript only
```

## Device Detection Logic

1. `GET /api/hostname` — success with JSON `{hostname}` → SST mount
2. `GET /api/hostname` — 401 → AllSky camera
3. `GET /api/config` — success (fallback) → AllSky camera
4. Both fail → device not recognized, excluded from list

## Dependencies of Note

- `mdns-sd` v0.12 — pure Rust mDNS (spawns its own daemon thread)
- `ipnetwork` — subnet enumeration (replaces old CoffeeScript `netmask` lib)
- `if-addrs` — local network interface enumeration
- `reqwest` — async HTTP client with JSON support
- `@mui/material` v7 — UI component library (dark theme)
