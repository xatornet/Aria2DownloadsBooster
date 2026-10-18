# ⚡ Aria2 Downloads Booster — edición Windows

**🇪🇸 Español** · [🇬🇧 English](README.en.md)

> **Este proyecto está creado CON IA** (código generado por IA y curado/verificado por el autor).

Acelerador de descargas multi-hilo con interfaz **nativa** basado en **aria2**.

<img width="1001" height="702" alt="imagen" src="https://github.com/user-attachments/assets/1ecd6954-aeac-43ce-941d-fe323b19bbf1" />


## Qué hace

- Pide un **enlace** (uno o varios como espejos) y una **carpeta de destino**, y descarga a la
  **velocidad máxima** con hasta **16 conexiones simultáneas** por descarga.
- **Pausa y reanuda incluso tras cerrar la aplicación**: la lista de descargas inconclusas
  sobrevive al cierre y cada reanudación **continúa donde estaba** (usa los archivos de control
  `.aria2`; nunca re-descarga desde cero).
- **Recuperación automática**: si el motor aria2 cae o se reinicia, las descargas activas se
  re-añaden solas y siguen.
- Cada tarjeta muestra en vivo: progreso, tamaño, **velocidad**, **hilos en uso** (8/16),
  **tiempo restante** y **qué está haciendo aria2 en cada momento** (resolviendo DNS,
  preparando, recibiendo datos, en cola esperando hueco, reintentando…).
- Descarga el binario aria2c correcto automáticamente (con espejos) si falta.
- Interfaz en español o inglés según el idioma del sistema. Tema oscuro.

## Tecnología

| Capa | Detalle |
|---|---|
| Lenguaje / UI | **C# (.NET 8) + Avalonia 11** (interfaz nativa, no webview) |
| Motor | **aria2c** en modo demonio JSON-RPC (puertos 6800-6804, token aleatorio por sesión) |
| Persistencia | `registry.json` en `%APPDATA%\Aria2DownloadsBooster\` (fuente de verdad del resume) |
| Tramas | `%APPDATA%\Aria2DownloadsBooster\ui.log` y `aria2c.log` |

## Usar (sin compilar)

Descarga el `.zip` de la última release, descomprime y ejecuta `Aria2DownloadsBooster.exe`
(self-contained: no necesita .NET instalado).

> **Renombrado**: la app se llamaba *Aria-XTRDownloader*; si existían tus datos en
> `%APPDATA%\Aria-XTRDownloader\`, se mueven solos al primer arranque al nuevo
> `%APPDATA%\Aria2DownloadsBooster\` (descargas, ajustes e historial intactos).

## Compilar

```powershell
dotnet publish AriaXtrDownloader.csproj -c Release -r win-x64 --self-contained true `
  /p:PublishSingleFile=true /p:IncludeNativeLibrariesForSelfExtract=true -o dist\portable-win-x64
```

## Builds a petición (GitHub Actions)

En la pestaña **Actions → «Build Windows» → Run workflow** se compila el EXE portable;
si la compilación tiene éxito, el `.zip` se publica automáticamente en **Releases**.

## Notas de diseño (bugs de aria2 ya resueltos)

- `--file-allocation=none` en Windows (con `falloc`/`prealloc`, sin privilegios de admin aria2
  rellena el archivo de ceros y **bloquea el demonio entero**).
- `system.multicall` en JSON-RPC: token como primer parámetro de **cada sub-llamada** y resultados
  en forma `[valor]`; `aria2.tellWaiting` está roto con rpc-secret y **nunca se usa**.
- Un GID desaparecido (`"GID … is not found"`) se detecta y la descarga se re-añade con
  `continue=true`.

## Licencia

MIT

---

*Rama `arch`: versión para Arch Linux con UI nativa **GTK4 (Rust + gtk4-rs)** en desarrollo.*
