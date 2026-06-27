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
- **Process classification** — system vs user listeners, with filters to hide system services
- **Port lookup** — search by port, PID, process name, path, or command; history timeline and one-click **Free port**
- **Row actions** — stop process, open in browser, file manager, terminal, editor (Cursor / VS Code), copy path/URL, pin project, move to trash, delete permanently
- **Compact tray popover** for quick access without opening the full window
- **Watched-port notifications** — in-app toasts and system alerts when specific ports change
- **Export snapshot** — copy filtered results as JSON or Markdown
- **CLI** — `port-watch check <port> [--udp]` for scripting and CI
- **Safety guards** — blocks destructive actions on protected system paths

## Platform support

| Platform | Status |
| --- | --- |
| macOS | Available |
| Windows | Planned |
| Linux | Planned |

## Requirements

- [Node.js](https://nodejs.org/) (npm or [bun](https://bun.sh/))
- [Rust toolchain](https://www.rust-lang.org/tools/install) (for Tauri)
- Platform-specific build dependencies — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

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

Release bundles are written to `src-tauri/target/release/bundle/`.

## Usage

### Main window

The full window shows all listening ports in a sortable, resizable table. Use the toolbar to search, filter user vs system listeners, export results, and open settings.

### Tray popover

Enable tray mode from the tray icon context menu. The compact popover lists user listeners with quick stop, browser, and file manager actions.

### Port lookup

Search for a specific port to see whether it is free, who is using it, and its recent history. Use **Free port** to stop all processes bound to that port.

### CLI

Install the command-line tool from **Settings → Command line** (one click). Port Watch symlinks `/usr/local/bin/port-watch` to the app binary so you can run:

```bash
port-watch check 3000
port-watch check 53 --udp
```

On first launch, Port Watch may offer to install the CLI automatically. macOS may ask for your password to write to `/usr/local/bin`.

**Exit codes:** `0` = port free, `1` = port in use (JSON on stdout), `2` = error.

**Without PATH install**, use the bundled binary directly:

```bash
"/Applications/Port Watch.app/Contents/MacOS/port-watch" check 3000
"/Applications/Port Watch.app/Contents/MacOS/port-watch" install-cli
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
    Scanner[PortScanner]
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

Destructive actions (move to trash, delete permanently) are blocked for processes running from protected system paths such as `/System`, `/usr`, `/bin`, `/sbin`, and `/Library`.

System process stop/delete requires an explicit opt-in in Settings.

## Tech stack

Tauri 2 · Rust · React 19 · shadcn/ui · Tailwind 4 · TanStack Table

## License

MIT — see [LICENSE](LICENSE).
