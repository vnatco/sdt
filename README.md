# Shut Down Timer

A small timer that shuts down, restarts or puts your computer to sleep when time is up. Set the time on a dial, press Start, and shrink it to a pill that floats on top of the screen while it counts down.

No .NET, no runtime to install: a native app built with [Tauri](https://tauri.app) (Rust) and Svelte.

## Features

- **Dial.** Drag around the ring (one lap is one hour), click the ring to jump, scroll over the hours or minutes, or just type: `5` is 5 minutes, `130` is 1 hour 30 minutes. Quick +5m, +15m, +30m and +1h buttons.
- **In or At.** Count down a duration, or pick a time of day ("At 23:30").
- **Shut Down, Restart or Sleep.** The whole app takes the colour of the chosen action.
- **The pill.** Shrink the timer to a pill that stays on top of other windows. Hover it for Pause, +5, Full View, Hide to Tray and Cancel. Drag it anywhere: it snaps to the top centre when dropped near it, sits flush in corners, and always grows away from the screen edge.
- **The last minute.** The pill turns red and grows with a big countdown, +5 Min, +15 Min and Cancel, with a soft chime and ticks for the last ten seconds. If the window was hidden, it comes back on top.
- **Tray.** The tray icon is a live progress ring of the time left. Left click shows or hides the timer; right click opens the app's menu. The taskbar button fills up as time passes.
- **Forced, like `shutdown /s /f`.** Shut down and restart close open apps without asking to save.
- **Safe around sleep.** The countdown runs on the wall clock. If the time runs out while the computer is asleep, you get one minute of warning after it wakes instead of an instant shut down. While a timer runs, the computer is kept from going to sleep on its own (can be turned off).

## Platforms

Windows 10 and 11. The code has Linux (`systemctl`) and macOS power commands, but only Windows is built and tested; the floating-window behaviour (click-through, gliding, dragging) is Windows-only.

On computers with Modern Standby (no S3 sleep), Sleep turns the display off, which is how Windows enters standby on those machines.

## Building

Requirements: Windows 10/11, [Rust](https://rustup.rs) (stable, MSVC), [Node.js](https://nodejs.org) 20+, and the WebView2 runtime (included with Windows 11).

```bash
npm install
npm run tauri dev      # run in development
npm run dist           # build the installer: src-tauri/target/release/bundle/nsis/ShutDownTimer_<version>_x64-setup.exe
```

Tests:

```bash
npm test
cd src-tauri
cargo test
```

`cargo test -- --ignored scheduled_shutdown` asks Windows to shut down and restart in 10 minutes and cancels each straight away, proving the real calls are accepted without turning the computer off.

### Trying It Without Turning Off

Start the app with `--dry-run` (or the environment variable `SDT_DRY_RUN=1`). The timer runs normally, but when it ends the action is only logged, and the app shows a Dry Run badge.

## Project Layout

| Path | What |
| --- | --- |
| `src-tauri/src/timer.rs` | The countdown: deadline, pause, add time, the last minute, sleep handling |
| `src-tauri/src/power.rs` | Shut down, restart and sleep |
| `src-tauri/src/window.rs` | Click-through, gliding and dragging the window |
| `src-tauri/src/menu.rs` | The pop-up menu window and its placement on screen |
| `src-tauri/src/tray.rs` | The tray icon's progress ring |
| `src/` | Interface (SvelteKit + TypeScript) |
| `design/icon.svg` | Source of the app icon |

Settings are stored in `%APPDATA%\co.vnat.sdt`.

## License

[Apache 2.0](LICENSE)
