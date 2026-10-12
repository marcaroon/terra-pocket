# Terra companion app (M10)

Tauri 2 + React + TypeScript. The window and tray are a thin client: all
Bluetooth, media and volume logic lives in `companion-core` (spec §5, §27).

| Part | Role |
| --- | --- |
| `src-tauri/src/main.rs` | Commands, `status` event, tray, close-to-tray, startup entry, single instance |
| `src-tauri/src/status.rs` | Folds `companion-core` session updates into what the window and tray show |
| `src/` | React view; calls `get_status`, `get_settings`, `set_auto_connect`, `set_start_with_windows`, `reconnect` |

`src-tauri` is its own Cargo workspace (with its own `Cargo.lock`) because
`tauri::generate_context!` embeds `dist/`, which `npm run build` creates.

## Build

From `companion/app`, with Node 20.19+ and the Rust toolchain active
(`. ..\..\scripts\activate-local.ps1` on the development laptop):

```powershell
npm ci
npx tauri build --debug --no-bundle
.\src-tauri\target\debug\terra-companion.exe
```

`npx tauri build --no-bundle` builds the release binary (no console window).
Dropping `--no-bundle` also builds the NSIS installer; the Tauri bundler then
downloads NSIS on first use. `npx tauri dev` runs against the Vite dev server.

The npm `@tauri-apps/*` packages must match the `tauri` crate's major/minor
version (currently 2.11, chosen by Cargo for `rust-version = 1.85`).

## Behavior

- **Auto Connect** (default on, stored in `%APPDATA%\dev.terra.companion\settings.json`):
  connect at start and keep reconnecting with the M9 backoff. Off disconnects
  and stays disconnected until **Reconnect**.
- **Start with Windows** writes the per-user startup entry through
  `tauri-plugin-autostart`; Windows starts the app with `--minimized` (tray only).
- Closing the window hides it to the tray. Tray: status line, Open, Reconnect, Exit.
- A second launch focuses the running instance instead of competing for Terra.
  Do not run `terra-cli connect` at the same time: Terra accepts one connection.
