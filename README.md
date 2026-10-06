# Port Watch

<p align="left">
  <img src="docs/icon.png" alt="Port Watch icon" width="64" height="64" />
</p>

Cross-platform desktop port monitor built with **Tauri 2**, **React**, and **shadcn/ui**. Scan listening TCP/UDP ports, classify processes, and manage dev servers from the system tray or full window.

![Main window — dark mode](docs/screenshots/main-window-dark.png)

## Screenshots

| Main window (dark) | Tray popover |
| --- | --- |
| ![Main window dark](docs/screenshots/main-window-dark.png) | ![Tray popover](docs/screenshots/tray-popover-light.png) |

| Row actions | Settings |
| --- | --- |
| ![Row actions menu](docs/screenshots/row-actions-menu.png) | ![Settings dialog](docs/screenshots/settings.png) |

| Port search | Main window (light) |
| --- | --- |
| ![Port search empty state](docs/screenshots/port-search-empty.png) | ![Main window light](docs/screenshots/main-window-light.png) |

## Features

- **Live port scan** with configurable auto-refresh (3s / 10s / off), optional UDP
- **Process classification** — vendor/system/user listeners (Apple, Microsoft, distro packages), with filters to hide system services
- **Port lookup** — search by port, PID, process name, path, or command; history timeline and one-click **Free port**
- **Row actions** — stop process, open in browser, file manager, terminal, editor (Cursor / VS Code), copy path/URL, pin project, move to trash, delete permanently
- **Compact tray popover** for quick access without opening the full window
- **Watched-port notifications** — in-app toasts and desktop alerts when specific ports change
- **Notification controls** — clear all toasts, mute port change toasts for 15 minutes / 1 hour / 8 hours, or turn them off, from the toast stack or the toolbar bell (mute and on/off are also in Settings)
- **Export snapshot** — copy filtered results as JSON or Markdown
- **CLI** — `port-watch check <port> [--udp]` for scripting and CI
- **Safety guards** — blocks destructive actions on protected system paths

## Platform support

| Platform | Scanner backend | CLI PATH install |
| --- | --- | --- |
| macOS | `lsof` + `ps` | `/usr/local/bin/port-watch` (symlink; may prompt for password) |
| Linux | `ss` + `/proc` | `~/.local/bin/port-watch` (symlink; ensure `~/.local/bin` is on PATH) |
| Windows | PowerShell (`Get-NetTCPConnection`) | `%LOCALAPPDATA%\Programs\Port Watch\port-watch.exe` (user PATH) |

macOS-only UI: **menu bar mode** (accessory app / dockless tray).

## Requirements

- [Node.js](https://nodejs.org/) (npm or [bun](https://bun.sh/))
- [Rust toolchain](https://www.rust-lang.org/tools/install) (for Tauri)
- Platform build dependencies — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

**Linux (Debian/Ubuntu)** additionally needs:

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

## Install from source

```bash
git clone https://github.com/SaiBarathR/port-watch.git
cd port-watch
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Release bundles are written to `src-tauri/target/release/bundle/` (`.app` on macOS, `.deb`/AppImage on Linux, `.msi`/`.exe` on Windows).

## Releases

Pre-built installers are published on [GitHub Releases](https://github.com/SaiBarathR/port-watch/releases):

| Platform | Download |
| --- | --- |
| macOS | `.dmg` (universal: Apple Silicon + Intel) |
| Windows | `.msi` or `.exe` setup |
| Linux | `.deb`, `.rpm`, or `.AppImage` |

Builds are unsigned. macOS may show Gatekeeper warnings (right-click → Open, or allow in System Settings → Privacy & Security). Windows SmartScreen may prompt for “More info” → “Run anyway”.

### Publishing a new release

1. Bump `version` in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` (keep them in sync).
2. Commit and push to `main`.
3. Tag and push:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
4. GitHub Actions builds on macOS, Linux, and Windows and uploads assets to a draft release. Review and publish it on the Releases page.

## Usage

### Main window

The full window shows all listening ports in a sortable, resizable table. Use the toolbar to search, filter user vs system listeners, export results, and open settings.

### Tray popover

Click the tray icon for a compact popover with quick stop, browser, and file manager actions. On macOS, enable **menu bar mode** from the tray context menu to hide the dock icon.

### Port lookup

Search for a specific port to see whether it is free, who is using it, and its recent history. Use **Free port** to stop all processes bound to that port.

### CLI

Install from **Settings → Command line** (one click) or run `install-cli` on the bundled binary:

```bash
port-watch check 3000
port-watch check 53 --udp
```

**Exit codes:** `0` = port free, `1` = port in use (JSON on stdout), `2` = error.

**Direct binary examples:**

```bash
# macOS
"/Applications/Port Watch.app/Contents/MacOS/port-watch" check 3000

# Linux / Windows (path varies by install location)
port-watch install-cli
```

## How it works

```mermaid
flowchart LR
  subgraph frontend [React Frontend]
    MainWindow[MainWindow]
    Popover[TrayPopover]
    Settings[SettingsDialog]
  end
  subgraph backend [Tauri Rust Backend]
    Poller[BackgroundPoller]
    Scanner[PlatformScanner]
    Tray[SystemTray]
    Commands[ProcessFilesystemWorkflow]
  end
  Poller --> Scanner
  Poller -->|ports-updated event| MainWindow
  Poller -->|ports-updated event| Popover
  Tray --> Popover
  MainWindow --> Commands
  Popover --> Commands
```

## Safety

Deleting a project folder (move to trash, delete permanently) is one backend step that checks the folder before it stops the process. A folder can be deleted only if all of these hold:

- It is the project folder the latest scan recorded for that process. The window cannot name any other path.
- It is a real folder (not a symlink or a file) inside your home folder.
- It is not your home folder itself, one of its standard folders (`Desktop`, `Documents`, `Downloads`, …), or anywhere inside app data and settings (`Library`, `Applications`, `AppData`, and hidden folders such as `.config` or `.ssh`).
- It is not under a protected system path:
  - **macOS:** `/System`, `/usr`, `/bin`, `/sbin`, `/Library`
  - **Linux:** `/usr`, `/bin`, `/sbin`, `/lib`, `/lib64`, `/opt` (not `/usr/local`)
  - **Windows:** `C:\Windows`, `Program Files`, `Program Files (x86)`, `ProgramData`

On Windows the folder is inferred rather than read from the process, so delete is offered only when it comes from a script path, never from where the program is installed.

Stop refuses a PID that is not in the latest scan or that the scan knows under a different name. System process stop/delete requires an explicit opt-in in Settings, and "Stop all visible user processes" never includes system services.

## Tech stack

Tauri 2 · Rust · React 19 · shadcn/ui · Tailwind 4 · TanStack Table

## License

MIT — see [LICENSE](LICENSE).
