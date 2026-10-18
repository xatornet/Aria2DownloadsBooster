# ⚡ Aria-XTRDownloader

Acelerador de descargas multiplataforma (Windows y Linux) basado en **aria2**,
con interfaz nativa en **Avalonia (C#/.NET)**.

![plataformas](https://img.shields.io/badge/Windows-x64-0078D6)
![plataformas](https://img.shields.io/badge/Linux-x64-FCC624)
![stack](https://img.shields.io/badge/Avalonia-11.3-8B5CF6)

## Características

- 🚀 **Máxima velocidad**: hasta **16 hilos/conexiones paralelas por descarga**
  (`-x16 -s16 -k1M`), varios espejos (una URL por línea = mismo archivo).
- ⏸️ **Pausar y reanudar**, incluso **después de cerrar la aplicación**:
  el registro local (`AppData/Aria-XTRDownloader/registry.json`) guarda cada
  descarga y aria2 continúa usando sus archivos de control `.aria2`.
- 📋 Lista de descargas previas sin terminar, con estado, tamaño, velocidad y ETA.
- 🖥️ **Motor aria2 por sistema operativo**: la app detecta su SO y descarga
  automáticamente el binario que corresponde en su primera ejecución:
  - Windows → build oficial 1.37.0 (SourceForge, con mirrors de respaldo)
  - Linux → build estático amd64 (P3TERX Aria2-Pro-Core)
- 🌐 Interfaz oscura moderna (FluentTheme), en **español** con detección de
  inglés del sistema.

## Uso

1. Lanzar la app. Si falta el motor, el banner ofrece **«Descargar motor aria2»**.
2. «+ Nueva descarga»: pegar URL(es), elegir carpeta, ajustar hilos (1–16).
3. Pausar/reanudar con los botones de cada tarjeta; borrar con confirmación
   (opción de borrar también el archivo parcial).
4. Cerrar la app cuando quieras: las descargas quedan guardadas y podrán
   reanudarse al volver.

## Compilar

```bash
dotnet build -c Release                          # binario local
dotnet publish -c Release -r win-x64  --self-contained true /p:PublishSingleFile=true -o dist/portable-win-x64
dotnet publish -c Release -r linux-x64 --self-contained true -p:PublishSingleFile=true -o dist/portable-linux-x64
```

> El publish de Linux debe ejecutarse en Linux (o WSL). En distros con
> `aria2c` del sistema también puedes copiarlo a
> `~/.config/Aria-XTRDownloader/engine/aria2c` (o en Windows a
> `%APPDATA%/Aria-XTRDownloader/engine/aria2c.exe`).

## Arquitectura

- **Motor**: demonio `aria2c` con JSON-RPC local (puerto 6800 + token aleatorio).
- **Registro propio**: fuente de verdad de las descargas; al reanudar una
  huérfana se re-añade con `continue=true` y aria2 retoma por su control `.aria2`.
- **Parada limpia**: `aria2.shutdown` + `--save-session` de respaldo.

## Estructura

```
Program.cs / App.axaml         Punto de entrada
Services/EngineService.cs      Descarga SO→binario, arranque/parada del demonio
Services/AriaRpc.cs            Cliente JSON-RPC (multicall)
Services/RegistryService.cs    Persistencia JSON (registro + ajustes)
Services/DownloadVm.cs         Estado por descarga para las tarjetas
ViewModels/MainWindowViewModel polling 1 s, pausar/reanudar/reañadir
Views/                         MainWindow + diálogos (añadir/ajustes/borrar)
```
