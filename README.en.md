# Aria-XTRDownloader — Arch Linux edition (GTK4)

[🇪🇸 Español](README.md) · **🇬🇧 English**

> **⚡ AI-built project.** Some of the code is AI-generated, curated/verified by the author.

 A download accelerator built on **aria2** with a **native GTK4** interface
 
<img width="1308" height="983" alt="image" src="https://github.com/user-attachments/assets/2addd66a-b25b-4e33-9d38-890c244e4d0b" />

## What it does
- **16 threads per download**
- Pause/resume that **survives app exit** (picks up where it left off using `.aria2` control files)
- Automatic recovery if the engine dies.

## Stack (`arch` branch)

| Layer | Technology |
|---|---|
| UI | **Rust + gtk4-rs** (GTK ≥ 4.10, custom dark CSS) |
| Engine | **aria2c** as a JSON-RPC daemon (ports 6800-6804) |
| HTTP-RPC | Custom TCP over localhost (HTTP/1.1, no dependencies) |
| Persistence | `registry.json` in `~/.local/share/Aria-XTRDownloader/` |

## Installation on Arch (pick one)

**1) As an Arch package — downloads, builds and installs in one go** (recommended):

```bash
git clone -b arch https://github.com/xatornet/XTRDownloader.git && cd XTRDownloader/packaging && makepkg -si
```

**2) Without cloning the repo — a single line** (the PKGBUILD fetches the code itself, `arch` branch):

```bash
wget https://raw.githubusercontent.com/xatornet/XTRDownloader/arch/packaging/PKGBUILD -O PKGBUILD && makepkg -si
```

> In both cases `makepkg -si` resolves and installs the dependencies
> (on a fresh system it may first ask for `sudo pacman -S --needed base-devel`).

Installs `/usr/bin/aria-xtrdownloader`, an applications-menu entry and the icon
(hicolor theme). Runtime dependencies: `aria2`, `gtk4`, `libnotify`.

## Background mode, tray and notifications

- **Closing the window does not quit the application**: it hides and keeps downloading in the
  background (toggle in Settings). The first time, a desktop notification lets you know.
- **System tray icon** (StatusNotifierItem protocol over D-Bus, via the `ksni` library — the
  reference Rust implementation, proven on Plasma):
  - **Left-click** (or menu → Open): restores the window.
  - **Menu → Quit**: definitive shutdown (stops the aria2 engine; the icon disappears).
  - KDE/XFCE/Cinnamon/LXQt, etc. On **GNOME** you need the
    *«AppIndicator and KStatusNotifierItem Support»* extension.
  - The icon (`aria-xtrdownloader`) shows properly when the app is **installed** as a package
    (hicolor theme); running it straight from `target/` may show a generic icon.
- When a download finishes while the window is hidden, it notifies *«Download completed»*
  (uses `notify-send` — `libnotify`; silently ignored if missing).
- **Relaunching the app restores the window** (single instance via GtkApplication).
- **Ctrl+Q** with the window visible = definitive quit (stops the aria2 engine).

## Environment variables (optional)

| Variable | Effect |
|---|---|
| `ARIA_XTR_DATA` | Redirects the data directory (registry, logs, session) |
| `ARIA_XTR_ENGINE` | Path to an alternative `aria2c` binary |
| `ARIA_XTR_NO_TRAY=1` | Disables the SNI tray entirely (diagnostics) |

## Data files

- `~/.local/share/Aria-XTRDownloader/registry.json` — download list (source of truth for resume)
- `~/.local/share/Aria-XTRDownloader/aria2c.log` — engine log
- `~/.local/share/Aria-XTRDownloader/ui.log` — app traces (diagnostics)

## Status

Work in progress (GTK4 UI + engine). The `windows` branch (Avalonia/.NET) is **finished and
stable**: this branch carries the same engine logic, including all known aria2 bugs already
resolved (per-subcall token in `system.multicall`, `tellWaiting` unusable with rpc-secret,
`file-allocation=prealloc` on Linux, automatic re-add with `continue=true` when the daemon
loses its GIDs).

## License

MIT
