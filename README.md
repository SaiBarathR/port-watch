# Port Watch

Native macOS port monitor built with Tauri 2, React, and shadcn/ui. Scans listening TCP ports via `lsof`, classifies processes as Apple/system/user, and provides stop, Finder, and delete actions with safety guards.

## Run

```bash
cd ~/Dev/port-watch
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The `.app` bundle is written to `src-tauri/target/release/bundle/macos/Port Watch.app`.

## Features

- Live TCP listener scan with 3s / 10s / off auto-refresh
- Hide system services by default (user listeners only)
- Search by port, PID, process name, or path
- Stop (SIGTERM → SIGKILL), Open in Finder, Copy Path, Move to Trash, Delete Permanently
- Protected path guards for `/System`, `/usr`, `/bin`, `/sbin`, `/Library`
