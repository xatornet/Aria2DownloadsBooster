# ⚡ Aria2 Downloads Booster — Windows edition

[🇪🇸 Español](README.md) · **🇬🇧 English**

> **This project was built WITH AI** (AI-generated code, curated/verified by the author).

Multi-threaded download accelerator with a **native** interface, powered by **aria2**.

<img width="1001" height="702" alt="imagen" src="https://github.com/user-attachments/assets/1ecd6954-aeac-43ce-941d-fe323b19bbf1" />


## What it does

- Asks for a **link** (one or several as mirrors) and a **destination folder**, then downloads at
  **maximum speed** with up to **16 simultaneous connections** per download.
- **Pause and resume even after closing the application**: the list of unfinished downloads
  survives app exit and every resume **picks up where it left off** (uses `.aria2` control
  files; never re-downloads from scratch).
- **Automatic recovery**: if the aria2 engine dies or restarts, active downloads are re-added
  on their own and keep going.
- Each card shows live: progress, size, **speed**, **threads in use** (8/16), **time
  remaining** and **what aria2 is doing at every moment** (resolving DNS, preparing,
  receiving data, queued waiting for a slot, retrying…).
- Automatically downloads the correct aria2c binary (with mirrors) if missing.
- Interface in Spanish or English depending on the system language. Dark theme.

## Technology

| Layer | Detail |
|---|---|
| Language / UI | **C# (.NET 8) + Avalonia 11** (native UI, not a webview) |
| Engine | **aria2c** as a JSON-RPC daemon (ports 6800-6804, random per-session token) |
| Persistence | `registry.json` in `%APPDATA%\Aria2DownloadsBooster\` (source of truth for resume) |
| Traces | `%APPDATA%\Aria2DownloadsBooster\ui.log` and `aria2c.log` |

## Using it (no compiling)

Download the `.zip` from the latest release, unpack and run `Aria2DownloadsBooster.exe`
(self-contained: no .NET installation required).

> **Renamed**: the app used to be called *Aria-XTRDownloader*; if your data still lives in
> `%APPDATA%\Aria-XTRDownloader\`, it moves itself to the new
> `%APPDATA%\Aria2DownloadsBooster\` on first launch (downloads, settings and history intact).

## Building

```powershell
dotnet publish AriaXtrDownloader.csproj -c Release -r win-x64 --self-contained true `
  /p:PublishSingleFile=true /p:IncludeNativeLibrariesForSelfExtract=true -o dist\portable-win-x64
```

## On-demand builds (GitHub Actions)

In the **Actions → “Build Windows” → Run workflow** tab the portable EXE is compiled;
if the build succeeds, the `.zip` is automatically published to **Releases**.

## Design notes (aria2 bugs already resolved)

- `--file-allocation=none` on Windows (with `falloc`/`prealloc`, without admin privileges aria2
  fills the file with zeros and **blocks the whole daemon**).
- `system.multicall` in JSON-RPC: token as the first parameter of **each sub-call** and results
  shaped as `[value]`; `aria2.tellWaiting` is broken with rpc-secret and is **never used**.
- A missing GID (`"GID … is not found"`) is detected and the download is re-added with
  `continue=true`.

## License

MIT

---

*`arch` branch: Arch Linux version with a native **GTK4 (Rust + gtk4-rs)** UI, in development.*
