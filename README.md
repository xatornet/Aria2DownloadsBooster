# Aria-XTRDownloader — edición Arch Linux (GTK4)

> **⚡ Proyecto creado con IA.** Parte del código es generado por IA y curado/verificado por el autor.

Acelerador de descargas sobre **aria2** con interfaz **nativa GTK4**: varias descargas a la vez
con hasta **16 hilos por descarga**, pausa/reanudación que **sobrevive al cierre de la app**
(continúa donde estaba usando los archivos de control `.aria2`) y recuperación automática si el
motor cae.

<img width="1308" height="983" alt="image" src="https://github.com/user-attachments/assets/2addd66a-b25b-4e33-9d38-890c244e4d0b" />


## Stack (rama `arch`)

| Capa | Tecnología |
|---|---|
| UI | **Rust + gtk4-rs** (GTK ≥ 4.10, CSS oscuro propio) |
| Motor | **aria2c** en modo demonio JSON-RPC (puertos 6800-6804) |
| HTTP-RPC | TCP propio sobre localhost (HTTP/1.1, sin dependencias) |
| Persistencia | `registry.json` en `~/.local/share/Aria-XTRDownloader/` |

## Compilar y ejecutar en Arch

```bash
sudo pacman -S --needed base-devel rust aria2 gtk4 libnotify
git clone -b arch https://github.com/xatornet/XTRDownloader.git
cd XTRDownloader
cargo build --release
./target/release/aria-xtrdownloader
```

## Instalar como paquete Arch (recomendado)

```bash
cd XTRDownloader/packaging
makepkg -si
```

O sin clonar el repo (el PKGBUILD se descarga el código solo, rama `arch`):

```bash
wget https://raw.githubusercontent.com/xatornet/XTRDownloader/arch/packaging/PKGBUILD
makepkg -si
```

Instala `/usr/bin/aria-xtrdownloader`, entrada de menú de aplicaciones e icono
(tema hicolor). Dependencias en tiempo de ejecución: `aria2`, `gtk4`, `libnotify`.

## Segundo plano y notificaciones

- **Cerrar la ventana no cierra la aplicación**: se oculta y sigue descargando en segundo plano
  (ajuste activable/desactivable). La primera vez avisa con una notificación del escritorio.
- Al terminar una descarga mientras la ventana está oculta, notifica *«Descarga completada»*
  (usa `notify-send` — paquete `libnotify`; si falta, se ignora en silencio).
- **Relanzar la app restaura la ventana** (instancia única vía GtkApplication).
- **Ctrl+Q** con la ventana visible = salida definitiva (apaga el motor aria2).

## Variables de entorno (opcionales)

| Variable | Efecto |
|---|---|
| `ARIA_XTR_DATA` | Redirige el directorio de datos (registry, logs, sesión) |
| `ARIA_XTR_ENGINE` | Ruta a un binario `aria2c` alternativo |

## Ficheros de datos

- `~/.local/share/Aria-XTRDownloader/registry.json` — lista de descargas (fuente de verdad del resume)
- `~/.local/share/Aria-XTRDownloader/aria2c.log` — log del motor
- `~/.local/share/Aria-XTRDownloader/ui.log` — trazas de la app (diagnóstico)

## Estado

Desarrollo en curso (UI GTK4 + motor). La rama `windows` (Avalonia/.NET) está **terminada y estable**:
esta rama porta la misma lógica de motor, incluidos todos los bugs conocidos de aria2 ya resueltos
(token por sub-llamada en `system.multicall`, `tellWaiting` inutilizable con rpc-secret,
`file-allocation=prealloc` en Linux, re-añadido automático con `continue=true` cuando el demonio
pierde los GIDs).

## Licencia

MIT
